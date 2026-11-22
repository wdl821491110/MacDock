//! Now-playing reader. Polls the Windows media session on a background MTA thread
//! (WinRT calls must not run on the UI thread). The menu bar reads `now()` from its
//! tick and redraws only when the snapshot changes. Playback commands are queued
//! from the UI thread and executed on the same MTA thread.

use std::collections::VecDeque;
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

#[derive(Clone, Copy)]
pub enum Cmd {
    Prev,
    Toggle,
    Next,
}

static STATE: OnceLock<Mutex<Option<NowPlaying>>> = OnceLock::new();
static CMDQ: OnceLock<Mutex<VecDeque<Cmd>>> = OnceLock::new();

thread_local! {
    static MANAGER: std::cell::RefCell<Option<Manager>> = const { std::cell::RefCell::new(None) };
}

/// Latest snapshot from the media worker. Cheap: one lock + clone of a few strings.
pub fn now() -> Option<NowPlaying> {
    STATE.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|g| g.clone())
}

/// Queue a playback command from the UI thread; the MTA worker executes it.
pub fn command(c: Cmd) {
    if let Ok(mut q) = CMDQ.get_or_init(|| Mutex::new(VecDeque::new())).lock() {
        q.push_back(c);
    }
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

fn drain_cmds() {
    let batch: Vec<Cmd> = {
        let Ok(mut q) = CMDQ.get_or_init(|| Mutex::new(VecDeque::new())).lock() else { return };
        q.drain(..).collect()
    };
    if batch.is_empty() {
        return;
    }
    if let Some(s) = session() {
        for c in batch {
            let r = match c {
                Cmd::Prev => s.TrySkipPreviousAsync(),
                Cmd::Next => s.TrySkipNextAsync(),
                Cmd::Toggle => s.TryTogglePlayPauseAsync(),
            };
            let _ = r.and_then(|o| o.get());
        }
    }
}

/// Spawn the reader. Safe to call once at startup.
pub fn start() {
    std::thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        loop {
            drain_cmds();
            let snap = read_once();
            if let Ok(mut st) = STATE.get_or_init(|| Mutex::new(None)).lock() {
                *st = snap;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}
