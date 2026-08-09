use gtk::gdk::Display;
use gtk::prelude::*;

use crate::config::ThemePref;

const STYLE_CSS: &str = include_str!("style.css");

/// 加载全局 CSS 主题
pub fn load_css() {
    let provider = gtk::CssProvider::new();
    // load_from_string 是 GTK 4.18 新增,用 load_from_data 兼容旧版
    provider.load_from_data(STYLE_CSS);
    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// 应用主题偏好:通过窗口 CSS 类切换(跟随系统/浅色/深色)。
/// style.css 用 window.theme-light/dark 定义两套 CSS 变量配色。
/// 注意:Deepin 的 GTK 4.18 不支持 @media (prefers-color-scheme),
/// 因此"跟随系统"由这里主动检测系统深浅色并映射到对应类。
pub fn apply_theme(theme: ThemePref, window: &gtk::ApplicationWindow) {
    let dark = match theme {
        ThemePref::System => system_prefers_dark(),
        ThemePref::Light => false,
        ThemePref::Dark => true,
    };
    if dark {
        window.add_css_class("theme-dark");
        window.remove_css_class("theme-light");
    } else {
        window.add_css_class("theme-light");
        window.remove_css_class("theme-dark");
    }
}

/// 检测系统是否偏好深色:gsettings color-scheme → gtk-theme 名称兜底
fn system_prefers_dark() -> bool {
    if let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if s.contains("prefer-dark") {
            return true;
        }
        if s.contains("prefer-light") {
            return false;
        }
    }
    // 兜底:主题名含 dark 视为深色
    if let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if s.contains("dark") {
            return true;
        }
    }
    false
}
