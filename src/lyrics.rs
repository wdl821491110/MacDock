//! Lyrics for the now-playing item. Fetches plain LRC from the Netease public API
//! on a worker thread (title + artist from the media session -> search -> lyric),
//! caches by song key, and exposes the current line for the menu bar to draw.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone)]
pub struct Line {
    pub t: f64,
    pub text: String,
}

struct Cache {
    map: HashMap<String, Option<Arc<Vec<Line>>>>,
    active: Option<Arc<Vec<Line>>>,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(Cache { map: HashMap::new(), active: None }))
}

fn key(title: &str, artist: &str) -> String {
    if artist.is_empty() { title.to_string() } else { format!("{}|{}", title, artist) }
}

/// Current lyric line at playback position `pos` (seconds).
pub fn current(pos: f64) -> Option<String> {
    let c = cache().lock().ok()?;
    let lines = c.active.as_ref()?;
    // last line whose timestamp <= pos
    let mut lo = 0usize;
    let mut hi = lines.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if lines[mid].t <= pos { lo = mid + 1 } else { hi = mid }
    }
    if lo == 0 {
        return None;
    }
    let text = lines[lo - 1].text.clone();
    if text.is_empty() { None } else { Some(text) }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

const HDR: &str = "User-Agent: Mozilla/5.0\r\nReferer: https://music.163.com/\r\n";

fn fetch(title: &str, artist: &str) -> Option<Vec<Line>> {
    let kw = if artist.is_empty() { title.to_string() } else { format!("{} {}", title, artist) };
    let path = format!("/api/search/get?s={}&type=1&limit=3&offset=0", urlencode(&kw));
    let (st, body) = crate::claude_oauth::get("music.163.com", &path, HDR).ok()?;
    if st != 200 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let songs = v["result"]["songs"].as_array()?;
    let song = songs.first()?;
    let id = song["id"].as_u64()?;

    let (st2, lbody) = crate::claude_oauth::get("music.163.com", &format!("/api/song/lyric?id={}&lv=1", id), HDR).ok()?;
    if st2 != 200 {
        return None;
    }
    let lv: serde_json::Value = serde_json::from_str(&lbody).ok()?;
    let lrc = lv["lrc"]["lyric"].as_str()?;
    let lines = parse_lrc(lrc);
    if lines.is_empty() { None } else { Some(lines) }
}

fn parse_time(tag: &str) -> Option<f64> {
    let (m, s) = tag.split_once(':')?;
    let m: f64 = m.parse().ok()?;
    let s: f64 = s.parse().ok()?;
    Some(m * 60.0 + s)
}

fn parse_lrc(s: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for row in s.lines() {
        let mut times = Vec::new();
        let mut rest = row;
        loop {
            let t = rest.trim_start();
            if t.starts_with('[') {
                if let Some(end) = t.find(']') {
                    if let Some(sec) = parse_time(&t[1..end]) {
                        times.push(sec);
                        rest = &t[end + 1..];
                        continue;
                    }
                }
            }
            break;
        }
        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        for t in times {
            out.push(Line { t, text: text.to_string() });
        }
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Spawn the fetch worker. Watches the media snapshot; fetches lyrics once per new song.
pub fn start() {
    std::thread::spawn(|| {
        let mut last = String::new();
        loop {
            if let Some(np) = crate::media::now() {
                let k = key(&np.title, &np.artist);
                if k != last {
                    last = k.clone();
                    let cached = cache().lock().ok().and_then(|c| c.map.get(&k).cloned());
                    let entry = match cached {
                        Some(v) => v,
                        None => {
                            let got = fetch(&np.title, &np.artist).map(Arc::new);
                            if let Ok(mut c) = cache().lock() {
                                if c.map.len() > 40 {
                                    c.map.clear();
                                }
                                c.map.insert(k.clone(), got.clone());
                            }
                            got
                        }
                    };
                    if let Ok(mut c) = cache().lock() {
                        c.active = entry;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_lrc() {
        let lrc = "[ar:测试]\n[00:01.50]第一行\n[00:03.00][00:06.00]第二行\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 3);
        assert!((lines[0].t - 1.5).abs() < 0.01);
        assert_eq!(lines[0].text, "第一行");
        assert_eq!(lines[2].text, "第二行");
    }
}
