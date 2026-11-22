//! Lyrics for the now-playing item. Uses the same Kugou source as MoeKoe desktop:
//! search by title (+duration to pin the right version) -> candidate id/accesskey ->
//! download fmt=lrc -> base64 decode -> plain LRC. Cached by song key.

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
    active_key: String,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(Cache { map: HashMap::new(), active: None, active_key: String::new() }))
}

fn key(title: &str, artist: &str) -> String {
    if artist.is_empty() { title.to_string() } else { format!("{}|{}", title, artist) }
}

/// Current lyric line at playback position `pos` (seconds).
pub fn current(pos: f64) -> Option<String> {
    let c = cache().lock().ok()?;
    let lines = c.active.as_ref()?;
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

const HDR: &str = "User-Agent: Mozilla/5.0\r\nReferer: https://www.kugou.com/\r\n";

/// Minimal standard base64 decoder (ASCII), no external crate.
fn b64(s: &str) -> Option<Vec<u8>> {
    fn v(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => None,
            _ => None,
        }
    }
    let b = s.trim().as_bytes();
    let mut out = Vec::with_capacity(b.len() * 3 / 4);
    let mut i = 0;
    while i + 4 <= b.len() {
        let c0 = v(b[i])?;
        let c1 = v(b[i + 1])?;
        let pad2 = b[i + 2] == b'=';
        let pad3 = b[i + 3] == b'=';
        let c2 = if pad2 { 0 } else { v(b[i + 2])? };
        let c3 = if pad3 { 0 } else { v(b[i + 3])? };
        out.push((c0 << 2) | (c1 >> 4));
        if !pad2 { out.push((c1 << 4) | (c2 >> 2)); }
        if !pad3 { out.push((c2 << 6) | c3); }
        i += 4;
    }
    Some(out)
}

/// Pick the candidate whose duration is closest to the playing track (ms).
fn pick<'a>(cands: &'a [serde_json::Value], dur_ms: u64) -> Option<&'a serde_json::Value> {
    if cands.is_empty() { return None; }
    if dur_ms == 0 { return cands.first(); }
    cands.iter().min_by_key(|c| {
        let d = c["duration"].as_u64().unwrap_or(0);
        d.abs_diff(dur_ms)
    })
}

fn fetch(title: &str, duration: f64) -> Option<Vec<Line>> {
    let dur_ms = (duration * 1000.0).round() as u64;
    let path = format!(
        "/search?ver=1&man=no&client=pc&keyword={}&duration={}&hash=",
        urlencode(title),
        dur_ms
    );
    let (st, body) = crate::claude_oauth::get("lyrics.kugou.com", &path, HDR).ok()?;
    if st != 200 { return None; }
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let cands = v["candidates"].as_array()?;
    // If duration-based search came back empty, retry with duration=0 (title only).
    let chosen = match pick(cands, dur_ms) {
        Some(c) => c.clone(),
        None => {
            let path0 = format!("/search?ver=1&man=no&client=pc&keyword={}&duration=0&hash=", urlencode(title));
            let (st0, body0) = crate::claude_oauth::get("lyrics.kugou.com", &path0, HDR).ok()?;
            if st0 != 200 { return None; }
            let v0: serde_json::Value = serde_json::from_str(&body0).ok()?;
            pick(v0["candidates"].as_array()?, 0)?.clone()
        }
    };
    let id = chosen["id"].as_u64()?;
    let ak = chosen["accesskey"].as_str()?;
    let dpath = format!(
        "/download?ver=1&client=pc&id={}&accesskey={}&fmt=lrc&charset=utf8",
        id,
        urlencode(ak)
    );
    let (st2, dbody) = crate::claude_oauth::get("lyrics.kugou.com", &dpath, HDR).ok()?;
    if st2 != 200 { return None; }
    let dv: serde_json::Value = serde_json::from_str(&dbody).ok()?;
    let content = dv["content"].as_str()?;
    let raw = b64(content)?;
    let lrc = String::from_utf8_lossy(&raw);
    let lines = parse_lrc(&lrc);
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
        let row = row.trim_start_matches('\u{feff}');
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
                            let got = fetch(&np.title, np.duration).map(Arc::new);
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
                        c.active_key = k;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
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
    #[test]
    fn decodes_base64() {
        assert_eq!(String::from_utf8(b64("SGVsbG8=").unwrap()).unwrap(), "Hello");
    }
}
