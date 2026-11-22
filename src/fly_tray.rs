//! The ^ extra (port of TrayFlyout.cs): the notification area icons in a grid. Hosting starts when the flyout
//! opens (apps answer TaskbarCreated within a moment and the grid fills live) and stops when it closes.

use std::time::Instant;

use crate::flyout::{white, Cmd, Content, Spot, Ui};
use crate::shell::Shell;
use crate::tray;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

const CELL: f32 = 44.0;
const ID_ICON: u32 = 100;
/// Small "pin to menu bar" buttons live in a separate id range so they never collide with icons.
const ID_PIN: u32 = 5000;

const WM_MOUSEMOVE: u32 = 0x200;
const WM_LBUTTONDOWN: u32 = 0x201;
const WM_LBUTTONUP: u32 = 0x202;
const WM_RBUTTONDOWN: u32 = 0x204;
const WM_RBUTTONUP: u32 = 0x205;
const WM_CONTEXTMENU: u32 = 0x7B;
const NIN_SELECT: u32 = 0x400;
const NIN_POPUPOPEN: u32 = 0x406;
const NIN_POPUPCLOSE: u32 = 0x407;

pub struct TrayFly {
    opened: Instant,
    hosted: bool,
    /// (hwnd, uid) per drawn cell, so a click finds its icon even if the list changed since
    shown: Vec<(u32, u32)>,
    /// "exe|name" key per drawn cell, for pinning to the menu bar.
    keys: Vec<String>,
    hot: Option<u32>,
}

impl TrayFly {
    pub fn new() -> TrayFly {
        TrayFly { opened: Instant::now(), hosted: false, shown: Vec::new(), keys: Vec::new(), hot: None }
    }

    /// Resolve an icon's stable key: "exe path|display name". Falls back to the tip/name.
    fn key_of(ic: &tray::Icon) -> String {
        let mut shell = Shell::default();
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(HWND(ic.hwnd as usize as *mut _), Some(&mut pid)) };
        let exe = if pid != 0 { shell.process_info(pid).0.unwrap_or_default() } else { String::new() };
        let name = if !ic.name.is_empty() {
            ic.name.clone()
        } else {
            ic.tip.lines().next().unwrap_or("").trim().to_string()
        };
        format!("{}|{}", exe, name)
    }

    /// Persist an icon snapshot as PNG. tiny-skia Pixmap is premultiplied RGBA;
    /// gfx::save_png expects BGRA, so swap R and B.
    fn save_snapshot(key: &str, img: &tiny_skia::Pixmap) {
        let (w, h) = (img.width(), img.height());
        let src = img.data();
        let mut bgra = Vec::with_capacity(src.len());
        for px in src.chunks_exact(4) {
            bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let _ = std::fs::create_dir_all(crate::config::tray_icon_dir());
        crate::gfx::save_png(&crate::config::tray_icon_file(key), w, h, &bgra);
    }

    fn with_icon(&self, id: u32, f: impl FnOnce(&tray::Icon)) {
        let Some(&(h, u)) = self.shown.get(id.wrapping_sub(ID_ICON) as usize) else { return };
        tray::with_icons(|icons| {
            if let Some(ic) = icons.and_then(|v| v.iter().find(|i| i.hwnd == h && i.uid == u)) {
                f(ic);
            }
        });
    }
}

impl Content for TrayFly {
    fn width(&self) -> f32 {
        5.0 * CELL + 2.0 * 14.0 + 2.0 * 13.0 + 4.0
    }

    fn opened(&mut self, hwnd: isize) {
        self.hosted = tray::start(hwnd);
    }

    fn closing(&mut self) {
        if let Some(id) = self.hot.take() {
            self.with_icon(id, |ic| tray::send_mouse(ic, NIN_POPUPCLOSE));
        }
        tray::stop();
    }

    fn period_ms(&self) -> u32 {
        // repaint while "Loading…" can still turn into "Nothing running"
        500
    }

    fn draw(&mut self, ui: &mut Ui) -> f32 {
        let w = ui.w;
        let mut shown = Vec::new();
        let mut keys = Vec::new();
        let pinned = crate::config::Config::load().menu_pinned;
        let h = tray::with_icons(|icons| {
            let visible: Vec<&tray::Icon> = icons.map(|v| v.iter().filter(|i| i.image.is_some() && !i.hidden).collect()).unwrap_or_default();
            let n = visible.len();
            // header: the hovered icon's tooltip, else the title; count on the right
            let hot_tip = self.hot.and_then(|id| visible.get(id.wrapping_sub(ID_ICON) as usize)).map(|i| match i.tip.lines().next().map(str::trim).filter(|t| !t.is_empty()) { Some(t) => t.to_string(), None => i.name.clone() }).filter(|t| !t.is_empty());
            if n > 0 {
                let cw = ui.text_w(&n.to_string(), 11.5, 400);
                ui.text_mid(w - 2.0 - cw, 9.0, &n.to_string(), 11.5, 400, white(0x80), cw + 2.0);
            }
            let title = hot_tip.unwrap_or_else(|| "Background apps".into());
            ui.text_mid(2.0, 9.0, &title, 13.0, 600, white(0xF2), w - 34.0);
            let mut y = 22.0;
            if n == 0 {
                let msg = if !self.hosted {
                    "The notification area could not be hosted."
                } else if self.opened.elapsed().as_secs_f32() < 1.4 {
                    "Loading…"
                } else {
                    "Nothing running in the background."
                };
                y += 6.0;
                let lh = ui.text_wrap(4.0, y, msg, 12.0, 400, white(0x80), w - 8.0);
                return y + lh + 2.0;
            }
            let ch = CELL - 6.0;
            for (i, ic) in visible.iter().enumerate() {
                let (cx, cy) = ((i % 5) as f32 * CELL, y + (i / 5) as f32 * ch);
                let id = ID_ICON + i as u32;
                if ui.hit(id, cx, cy, CELL, ch) {
                    ui.fill(cx, cy, CELL, ch, 9.0, white(0x24));
                }
                if let Some(img) = &ic.image {
                    ui.image(img, cx + (CELL - 20.0) / 2.0, cy + (ch - 20.0) / 2.0, 20.0, 20.0);
                }
                let key = TrayFly::key_of(ic);
                let is_pinned = pinned.iter().any(|p| p == &key);
                // pin button in the top-right corner of the cell
                let (px, py) = (cx + CELL - 13.0, cy + 1.0);
                if ui.hit(ID_PIN + i as u32, px, py, 12.0, 12.0) {
                    ui.fill(px, py, 12.0, 12.0, 4.0, white(0x40));
                }
                // a filled dot marks already-pinned icons
                ui.fill(px + 3.0, py + 3.0, 6.0, 6.0, 3.0,
                    if is_pinned { (0xFF, 0x2D, 0x2D, 0xFF) } else { white(0x66) });
                shown.push((ic.hwnd, ic.uid));
                keys.push(key);
            }
            y += n.div_ceil(5) as f32 * ch;
            y
        });
        self.shown = shown;
        self.keys = keys;
        h
    }

    fn placed(&mut self, spots: &[Spot], origin: (i32, i32), dpi: f32) {
        let shown = &self.shown;
        tray::with_icons(|icons| {
            let Some(icons) = icons else { return };
            for s in spots.iter().filter(|s| (ID_ICON..ID_PIN).contains(&s.id)) {
                let Some(&(h, u)) = shown.get((s.id - ID_ICON) as usize) else { continue };
                if let Some(ic) = icons.iter_mut().find(|i| i.hwnd == h && i.uid == u) {
                    let (l, t) = (origin.0 + (s.x * dpi) as i32, origin.1 + (s.y * dpi) as i32);
                    ic.place = windows::Win32::Foundation::RECT { left: l, top: t, right: l + (s.w * dpi) as i32, bottom: t + (s.h * dpi) as i32 };
                }
            }
        });
    }

    fn hover(&mut self, id: Option<u32>) {
        let id = id.filter(|&i| (ID_ICON..ID_PIN).contains(&i));
        if id == self.hot {
            return;
        }
        if let Some(old) = self.hot {
            self.with_icon(old, |ic| {
                if ic.version >= 4 {
                    tray::send_mouse(ic, NIN_POPUPCLOSE);
                }
            });
        }
        self.hot = id;
        if let Some(new) = id {
            self.with_icon(new, |ic| {
                tray::send_mouse(ic, WM_MOUSEMOVE);
                if ic.version >= 4 {
                    tray::send_mouse(ic, NIN_POPUPOPEN);
                }
            });
        }
    }

    fn press(&mut self, id: u32, _fx: f32) {
        self.with_icon(id, |ic| tray::send_mouse(ic, WM_LBUTTONDOWN));
    }

    fn click(&mut self, id: u32, _fx: f32) -> Cmd {
        // pin / unpin button
        if (ID_PIN..ID_PIN + self.keys.len() as u32).contains(&id) {
            let key = self.keys[(id - ID_PIN) as usize].clone();
            let mut cfg = crate::config::Config::load();
            if let Some(pos) = cfg.menu_pinned.iter().position(|p| p == &key) {
                cfg.menu_pinned.remove(pos);
                cfg.save();
                return Cmd::UnpinMenu(key);
            }
            // save an icon snapshot next to the key so the bar can draw it later
            let idx = (id - ID_PIN) as usize;
            if let Some(&(h, u)) = self.shown.get(idx) {
                tray::with_icons(|icons| {
                    if let Some(ic) = icons.and_then(|v| v.iter().find(|i| i.hwnd == h && i.uid == u)) {
                        if let Some(img) = &ic.image {
                            Self::save_snapshot(&key, img);
                        }
                    }
                });
            }
            cfg.menu_pinned.push(key.clone());
            cfg.save();
            return Cmd::PinMenu(key);
        }
        if id < ID_ICON {
            return Cmd::None;
        }
        self.with_icon(id, |ic| {
            tray::send_mouse(ic, WM_LBUTTONUP);
            if ic.version >= 3 {
                tray::send_mouse(ic, NIN_SELECT);
            }
        });
        // a left click usually opens the app's window: get out of its way
        Cmd::Close
    }

    fn right_click(&mut self, id: u32) -> Cmd {
        // the app's menu takes the foreground, which closes us anyway
        self.with_icon(id, |ic| {
            tray::send_mouse(ic, WM_RBUTTONDOWN);
            tray::send_mouse(ic, WM_RBUTTONUP);
            if ic.version >= 3 {
                tray::send_mouse(ic, WM_CONTEXTMENU);
            }
        });
        Cmd::None
    }
}
