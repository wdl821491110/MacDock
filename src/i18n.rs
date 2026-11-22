//! Interface language: English (the source strings), Indonesian or Simplified Chinese.
//! Strings stay English in the code and go through `t` where they are shown;
//! anything missing from a translation table shows in English.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Id,
    Zh,
}

static LANG: AtomicU8 = AtomicU8::new(0);

/// Config value ("auto", "en", "id", "zh") → the language to show. Auto follows the Windows display language.
pub fn resolve(setting: &str) -> Lang {
    match setting {
        "id" => Lang::Id,
        "en" => Lang::En,
        "zh" => Lang::Zh,
        _ => {
            let ui = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
            let primary = ui & 0x3FF;
            if primary == 0x04 {
                Lang::Zh // LANG_CHINESE (simplified or traditional UI)
            } else if primary == 0x21 {
                Lang::Id // LANG_INDONESIAN
            } else {
                Lang::En
            }
        }
    }
}

pub fn set(setting: &str) {
    LANG.store(resolve(setting) as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        x if x == Lang::Id as u8 => Lang::Id,
        x if x == Lang::Zh as u8 => Lang::Zh,
        _ => Lang::En,
    }
}

/// `s` in the interface language.
pub fn t(s: &str) -> &str {
    match lang() {
        Lang::En => s,
        Lang::Id => id(s).unwrap_or(s),
        Lang::Zh => zh(s).unwrap_or(s),
    }
}

/// A template with one `{}` (e.g. "Hide {}"), translated, then filled in.
pub fn fill(template: &str, v: &str) -> String {
    t(template).replacen("{}", v, 1)
}

fn id(s: &str) -> Option<&'static str> {
    Some(match s {
        // menu bar
        "File" => "File",
        "Edit" => "Edit",
        "View" => "Tampilan",
        "Window" => "Jendela",
        "Help" => "Bantuan",
        "Hide {}" => "Sembunyikan {}",
        "Quit {}" => "Keluar dari {}",
        "{} Help" => "Bantuan {}",
        "Update Available: MacDock {}…" => "Pembaruan Tersedia: MacDock {}…",
        "About This PC" => "Tentang PC Ini",
        "Check for MacDock Updates…" => "Periksa Pembaruan MacDock…",
        "System Settings…" => "Pengaturan Sistem…",
        "MacDock Settings…" => "Pengaturan MacDock…",
        "Force Quit…" => "Paksa Keluar…",
        "Sleep" => "Tidur",
        "Restart…" => "Mulai Ulang…",
        "Shut Down…" => "Matikan…",
        "Lock Screen" => "Kunci Layar",
        "Quit MacDock" => "Keluar dari MacDock",
        "Open MacDock at Login" => "Buka MacDock saat Login",
        "Automatically Hide Dock" => "Sembunyikan Dock Otomatis",
        "Show AI Usage" => "Tampilkan Pemakaian AI",
        "Show Battery Percentage" => "Tampilkan Persentase Baterai",
        "Show Tray Icons" => "Tampilkan Ikon Tray",
        "Hide Others" => "Sembunyikan Lainnya",
        "New Window" => "Jendela Baru",
        "New Tab" => "Tab Baru",
        "Open…" => "Buka…",
        "Open" => "Buka",
        "Save" => "Simpan",
        "Save As…" => "Simpan Sebagai…",
        "Print…" => "Cetak…",
        "Close Window" => "Tutup Jendela",
        "Undo" => "Urungkan",
        "Redo" => "Ulangi",
        "Cut" => "Potong",
        "Copy" => "Salin",
        "Paste" => "Tempel",
        "Select All" => "Pilih Semua",
        "Find…" => "Cari…",
        "Emoji & Symbols" => "Emoji & Simbol",
        "Zoom In" => "Perbesar",
        "Zoom Out" => "Perkecil",
        "Actual Size" => "Ukuran Asli",
        "Enter Full Screen" => "Masuk Layar Penuh",
        "Minimize" => "Minimalkan",
        "Zoom" => "Zoom",
        "Move to Left Half" => "Pindah ke Separuh Kiri",
        "Move to Right Half" => "Pindah ke Separuh Kanan",
        "Windows Tips" => "Tips Windows",
        // dock
        "Turn Hiding On" => "Nyalakan Sembunyi Otomatis",
        "Turn Hiding Off" => "Matikan Sembunyi Otomatis",
        "Show Recent Apps" => "Tampilkan App Terbaru",
        "Show Downloads Stack" => "Tampilkan Tumpukan Unduhan",
        "Dock Settings…" => "Pengaturan Dock…",
        "Keep in Dock" => "Simpan di Dock",
        "Remove from Dock" => "Hapus dari Dock",
        "Remove from Recents" => "Hapus dari Terbaru",
        "Change Icon…" => "Ubah Ikon…",
        "Reset Icon" => "Kembalikan Ikon",
        "Quit" => "Keluar",
        "Open in Explorer" => "Buka di Explorer",
        "Show in Explorer" => "Tampilkan di Explorer",
        "Open Downloads" => "Buka Unduhan",
        "No files" => "Tidak ada file",
        "Empty Recycle Bin" => "Kosongkan Recycle Bin",
        // launchpad
        "Search" => "Cari",
        "Loading apps…" => "Memuat app…",
        "No Results" => "Tidak Ada Hasil",
        // the Apps window and its categories
        "Applications" => "Aplikasi",
        "Apps" => "Apps",
        "Spotlight Search" => "Cari dengan Spotlight",
        "Application" => "Aplikasi",
        "Search the Web for" => "Cari di web",
        "Utilities" => "Utilitas",
        "Productivity & Finance" => "Produktivitas & Keuangan",
        "Entertainment" => "Hiburan",
        "Information & Reading" => "Informasi & Bacaan",
        "Social" => "Sosial",
        "Creativity" => "Kreativitas",
        "Developer Tools" => "Alat Developer",
        "Games" => "Game",
        "Other" => "Lainnya",
        "All apps" => "Semua app",
        "Apps window" => "Jendela Apps",
        "Full screen" => "Layar penuh",
        // settings
        "MacDock Settings" => "Pengaturan MacDock",
        "Dock" => "Dock",
        "Size" => "Ukuran",
        "Magnification" => "Pembesaran",
        "Off" => "Mati",
        "Automatically hide the dock" => "Sembunyikan dock otomatis",
        "Show recent apps" => "Tampilkan app terbaru",
        "Show Downloads stack" => "Tampilkan tumpukan Unduhan",
        "Icons on rounded tiles" => "Ikon di atas kotak bulat",
        "Notification badges" => "Lencana notifikasi",
        "Menu Bar & Desktop" => "Menu Bar & Desktop",
        "Hide the Windows taskbar" => "Sembunyikan taskbar Windows",
        "Menu bar" => "Menu bar",
        "Battery percentage" => "Persentase baterai",
        "AI usage in the menu bar" => "Pemakaian AI di menu bar",
        "Claude limits from your Claude login" => "Batas Claude dari login Claude",
        "Tray icons in the menu bar" => "Ikon tray di menu bar",
        "Desktop widgets" => "Widget desktop",
        "Mac-style Alt+Tab" => "Alt+Tab ala Mac",
        "restart" => "restart",
        "Animation" => "Animasi",
        "Genie effect when minimizing" => "Efek Genie saat minimize",
        "Minimize duration" => "Durasi minimize",
        "Magnification speed" => "Kecepatan pembesaran",
        "Dock show / hide speed" => "Kecepatan muncul dock",
        "Hide delay" => "Jeda sembunyi",
        "Reduce motion" => "Kurangi gerakan",
        "Reduce motion snaps the dock instead of animating it. Turn the Genie effect off to minimize with the Windows animation." => "Kurangi gerakan membuat dock langsung berubah tanpa animasi. Matikan efek Genie untuk memakai animasi minimize bawaan Windows.",
        "Language" => "Bahasa",
        "Auto" => "Otomatis",
        "General" => "Umum",
        "Restart MacDock to apply the menu bar, widgets and Alt+Tab changes." => "Restart MacDock untuk menerapkan perubahan menu bar, widget dan Alt+Tab.",
        "Changes apply right away and are saved to config.json." => "Perubahan langsung berlaku dan tersimpan di config.json.",
        "Restart MacDock" => "Restart MacDock",
        "Reset Animations" => "Reset Animasi",
        "Open config.json" => "Buka config.json",
        _ => return None,
    })
}

fn zh(s: &str) -> Option<&'static str> {
    Some(match s {
        // menu bar
        "File" => "文件",
        "Edit" => "编辑",
        "View" => "显示",
        "Window" => "窗口",
        "Help" => "帮助",
        "Hide {}" => "隐藏 {}",
        "Quit {}" => "退出 {}",
        "{} Help" => "{} 帮助",
        "Update Available: MacDock {}…" => "有可用更新：MacDock {}…",
        "About This PC" => "关于此电脑",
        "Check for MacDock Updates…" => "检查 MacDock 更新…",
        "System Settings…" => "系统设置…",
        "MacDock Settings…" => "MacDock 设置…",
        "Force Quit…" => "强制退出…",
        "Sleep" => "睡眠",
        "Restart…" => "重新启动…",
        "Shut Down…" => "关机…",
        "Lock Screen" => "锁定屏幕",
        "Quit MacDock" => "退出 MacDock",
        "Open MacDock at Login" => "开机时启动 MacDock",
        "Automatically Hide Dock" => "自动隐藏程序坞",
        "Show AI Usage" => "显示 AI 用量",
        "Show Battery Percentage" => "显示电池百分比",
        "Show Tray Icons" => "显示托盘图标",
        "Hide Others" => "隐藏其他窗口",
        "New Window" => "新建窗口",
        "New Tab" => "新建标签页",
        "Open…" => "打开…",
        "Open" => "打开",
        "Save" => "存储",
        "Save As…" => "另存为…",
        "Print…" => "打印…",
        "Close Window" => "关闭窗口",
        "Undo" => "撤销",
        "Redo" => "重做",
        "Cut" => "剪切",
        "Copy" => "复制",
        "Paste" => "粘贴",
        "Select All" => "全选",
        "Find…" => "查找…",
        "Emoji & Symbols" => "表情与符号",
        "Zoom In" => "放大",
        "Zoom Out" => "缩小",
        "Actual Size" => "实际大小",
        "Enter Full Screen" => "进入全屏",
        "Minimize" => "最小化",
        "Zoom" => "缩放",
        "Move to Left Half" => "移到左半屏",
        "Move to Right Half" => "移到右半屏",
        "Windows Tips" => "Windows 提示",
        // dock
        "Turn Hiding On" => "开启自动隐藏",
        "Turn Hiding Off" => "关闭自动隐藏",
        "Show Recent Apps" => "显示最近使用的应用",
        "Show Downloads Stack" => "显示下载堆栈",
        "Dock Settings…" => "程序坞设置…",
        "Keep in Dock" => "保留在程序坞中",
        "Remove from Dock" => "从程序坞中移除",
        "Remove from Recents" => "从最近使用中移除",
        "Change Icon…" => "更改图标…",
        "Reset Icon" => "重置图标",
        "Quit" => "退出",
        "Open in Explorer" => "在资源管理器中打开",
        "Show in Explorer" => "在资源管理器中显示",
        "Open Downloads" => "打开下载文件夹",
        "No files" => "没有文件",
        "Empty Recycle Bin" => "清空回收站",
        // launchpad
        "Search" => "搜索",
        "Loading apps…" => "正在加载应用…",
        "No Results" => "没有结果",
        // apps window
        "Applications" => "应用程序",
        "Apps" => "应用",
        "Spotlight Search" => "聚焦搜索",
        "Application" => "应用程序",
        "Search the Web for" => "在网页中搜索",
        "Utilities" => "实用工具",
        "Productivity & Finance" => "效率与财务",
        "Entertainment" => "娱乐",
        "Information & Reading" => "资讯与阅读",
        "Social" => "社交",
        "Creativity" => "创意",
        "Developer Tools" => "开发工具",
        "Games" => "游戏",
        "Other" => "其他",
        "All apps" => "所有应用",
        "Apps window" => "应用窗口",
        "Full screen" => "全屏",
        // settings
        "MacDock Settings" => "MacDock 设置",
        "Dock" => "程序坞",
        "Size" => "图标大小",
        "Magnification" => "放大倍数",
        "Off" => "关闭",
        "Automatically hide the dock" => "自动隐藏程序坞",
        "Show recent apps" => "显示最近使用的应用",
        "Show Downloads stack" => "显示下载堆栈",
        "Icons on rounded tiles" => "图标使用圆角底衬",
        "Notification badges" => "通知角标",
        "Menu Bar & Desktop" => "菜单栏与桌面",
        "Hide the Windows taskbar" => "隐藏 Windows 任务栏",
        "Menu bar" => "菜单栏",
        "Battery percentage" => "电池百分比",
        "AI usage in the menu bar" => "菜单栏显示 AI 用量",
        "Claude limits from your Claude login" => "用 Claude 登录读取用量额度",
        "Tray icons in the menu bar" => "菜单栏显示托盘图标",
        "Desktop widgets" => "桌面小组件",
        "Mac-style Alt+Tab" => "Mac 风格 Alt+Tab",
        "restart" => "重启",
        "Animation" => "动画",
        "Genie effect when minimizing" => "最小化时的神灯特效",
        "Minimize duration" => "最小化时长",
        "Magnification speed" => "放大动画速度",
        "Dock show / hide speed" => "程序坞显隐速度",
        "Hide delay" => "隐藏延迟",
        "Reduce motion" => "减少动态效果",
        "Reduce motion snaps the dock instead of animating it. Turn the Genie effect off to minimize with the Windows animation." => "减少动态效果会让程序坞直接显示而不播放动画。关闭神灯特效后最小化将使用 Windows 自带动画。",
        "Language" => "语言",
        "Auto" => "自动",
        "General" => "通用",
        "Restart MacDock to apply the menu bar, widgets and Alt+Tab changes." => "需要重启 MacDock 才能应用菜单栏、小组件和 Alt+Tab 的更改。",
        "Changes apply right away and are saved to config.json." => "更改立即生效并保存到 config.json。",
        "Restart MacDock" => "重启 MacDock",
        "Reset Animations" => "重置动画",
        "Open config.json" => "打开 config.json",
        // language names shown in the settings choice row
        "English" => "英语",
        "Indonesia" => "印尼语",
        "中文" => "中文",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english() {
        set("id");
        assert_eq!(t("Copy"), "Salin");
        assert_eq!(t("Something new"), "Something new");
        assert_eq!(fill("Hide {}", "Chrome"), "Sembunyikan Chrome");
        set("zh");
        assert_eq!(t("Copy"), "复制");
        assert_eq!(t("Something new"), "Something new");
        assert_eq!(fill("Hide {}", "Chrome"), "隐藏 Chrome");
        set("en");
        assert_eq!(t("Copy"), "Copy");
        assert_eq!(fill("Quit {}", "Chrome"), "Quit Chrome");
    }
}
