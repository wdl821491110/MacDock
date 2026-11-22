//! Now-playing reader. Polls the Windows media session on a background MTA thread
//! (WinRT calls must not run on the UI thread). The menu bar reads `now()` from its
//! 1-second tick and redraws only when the snapshot changes.

use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use windows::Foundation::TimeSpan;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Playback,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[derive(Clone, PartialEq, Debug)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub position: f64, // seconds
    pub duration: f64, // seconds
    pub playing: bool,
}

static STATE: OnceLock<Mutex<Option<NowPlaying>>> = OnceLock::new();

thread_local! {
    static MANAGER: RefCell<Option<Manager>> = const { RefCell::new(None) };
}

/// Latest snapshot from the media worker. Cheap: one lock + clone of a few strings.
pub fn now() -> Option<NowPlaying> {
    STATE.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|g| g.clone())
}

fn session() -> Option<Session> {
    MANAGER.with(|m| {
        let mut m = m.borrow_mut();
        if m.is_none() {
            *m = Manager::RequestAsync().and_then(|o| o.get()).ok();
        }
        m.as_ref()?.GetCurrentSession().ok()
    })
}

fn secs(ts: TimeSpan) -> f64 {
    ts.Duration as f64 / 10_000_000.0
}

fn read_once() -> Option<NowPlaying> {
    let s = session()?;
    let p = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = p.Title().ok()?.to_string();
    if title.is_empty() {
        return None;
    }
    let artist = p.Artist().map(|a| a.to_string()).unwrap_or_default();
    let playing = s
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .is_ok_and(|x| x == Playback::Playing);
    let (position, duration) = s
        .GetTimelineProperties()
        .map(|tl| {
            (
                tl.Position().map(secs).unwrap_or(0.0),
                tl.EndTime().map(secs).unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0));
    Some(NowPlaying { title, artist, position, duration, playing })
}

/// Spawn the reader. Safe to call once at startup.
pub fn start() {
    std::thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        loop {
            let snap = read_once();
            if let Ok(mut st) = STATE.get_or_init(|| Mutex::new(None)).lock() {
                *st = snap;
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}
