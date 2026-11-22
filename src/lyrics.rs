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

/// Clean a media-session title into a searchable song name.
/// Handles "歌名 - 歌手", "歌名 (Live)", "歌名 [现场]", "歌名 ft.xxx" etc.
fn clean_title(t: &str) -> String {
    let mut s = t.trim().to_string();
    // "歌名 - 歌手" / "歌名 – 歌手" / "歌名_歌手"
    for sep in [" - ", " – ", " — ", " _ "] {
        if let Some(i) = s.find(sep) {
            s = s[..i].to_string();
            break;
        }
    }
    // drop parenthesised/bracketed suffixes (Live, remix, 现场, ...)
    for (open, close) in [('(', ')'), ('（', '）'), ('[', ']'), ('【', '】')] {
        if let Some(i) = s.find(open) {
            if s[i..].contains(close) {
                s = s[..i].to_string();
            }
        }
    }
    // drop "ft." / "feat." tails
    for tag in [" ft.", " ft ", " feat.", " feat ", " ft.", " & "] {
        if let Some(i) = s.to_lowercase().find(&tag.to_lowercase()) {
            s = s[..i].to_string();
        }
    }
    s.trim().to_string()
}

fn search(kw: &str) -> Option<Vec<serde_json::Value>> {
    let path = format!("/search?ver=1&man=no&client=pc&keyword={}&duration=0&hash=", urlencode(kw));
    let (st, body) = crate::claude_oauth::get("lyrics.kugou.com", &path, HDR).ok()?;
    if st != 200 { return None; }
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    Some(v["candidates"].as_array()?.clone())
}

/// Pick a candidate whose song/singer matches the track; closest duration wins.
fn pick_match(cands: &[serde_json::Value], title: &str, artist: &str, dur_ms: u64) -> Option<serde_json::Value> {
    let norm = |s: &str| s.to_lowercase().replace([' ', '　', '-', '_'], "");
    let t = norm(title);
    let a = norm(artist);
    let mut ranked: Vec<(u8, u64, serde_json::Value)> = Vec::new();
    for c in cands {
        let song = norm(c["song"].as_str().unwrap_or(""));
        let singer = norm(c["singer"].as_str().unwrap_or(""));
        let d = c["duration"].as_u64().unwrap_or(0);
        // score: exact song match, then song-contained, then artist match
        let mut score = 0u8;
        if !t.is_empty() && song == t { score = 3; }
        else if !t.is_empty() && (song.contains(&t) || t.contains(&song)) { score = 2; }
        if !a.is_empty() && !singer.is_empty() && (singer == a || singer.contains(&a) || a.contains(&singer)) {
            score += 2;
        }
        if score > 0 {
            ranked.push((score, d.abs_diff(dur_ms), c.clone()));
        }
    }
    if ranked.is_empty() {
        // no match: fall back to the single candidate, or the duration-closest one
        return if cands.len() == 1 {
            Some(cands[0].clone())
        } else if dur_ms > 0 {
            cands.iter().min_by_key(|c| c["duration"].as_u64().unwrap_or(0).abs_diff(dur_ms)).cloned()
        } else {
            cands.first().cloned()
        };
    }
    ranked.sort_by_key(|x| (std::cmp::Reverse(x.0), x.1));
    ranked.into_iter().next().map(|x| x.2)
}

fn fetch(title: &str, artist: &str, duration: f64) -> Option<Vec<Line>> {
    let dur_ms = (duration * 1000.0).round() as u64;
    let clean = clean_title(title);
    // Try several keyword forms until one yields a matching candidate.
    let mut keywords: Vec<String> = Vec::new();
    if !clean.is_empty() { keywords.push(clean.clone()); }
    if !artist.is_empty() && artist != "未知歌手" {
        keywords.push(format!("{} {}", artist, clean));
    }
    if clean != title && !title.is_empty() { keywords.push(title.to_string()); }

    let mut chosen: Option<serde_json::Value> = None;
    for kw in &keywords {
        if let Some(cands) = search(kw) {
            if let Some(c) = pick_match(&cands, &clean, artist, dur_ms) {
                chosen = Some(c);
                break;
            }
        }
    }
    let chosen = chosen?;
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
        use std::time::Instant;
        let mut last = String::new();
        let mut last_try: Option<(String, Instant)> = None;
        loop {
            if let Some(np) = crate::media::now() {
                let k = key(&np.title, &np.artist);
                let fresh = k != last;
                if fresh {
                    last = k.clone();
                }
                // fetch on a new song, or retry a failed one every 15 s
                let due = last_try.as_ref().is_none_or(|(tk, t)| tk != &k || t.elapsed() >= Duration::from_secs(15));
                if fresh || due {
                    last_try = Some((k.clone(), Instant::now()));
                    let cached = cache().lock().ok().and_then(|c| c.map.get(&k).cloned()).flatten();
                    let entry = match cached {
                        Some(v) => Some(v),
                        None => {
                            let got = fetch(&np.title, &np.artist, np.duration).map(Arc::new);
                            if got.is_some() {
                                // only cache successes, so failures get retried
                                if let Ok(mut c) = cache().lock() {
                                    if c.map.len() > 40 {
                                        c.map.clear();
                                    }
                                    c.map.insert(k.clone(), got.clone());
                                }
                            }
                            got
                        }
                    };
                    if let Ok(mut c) = cache().lock() {
                        c.active = entry;
                        c.active_key = k.clone();
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
