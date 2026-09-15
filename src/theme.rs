use gtk::gdk::Display;

use crate::config::ThemePref;

const STYLE_CSS: &str = include_str!("style.css");

/// 加载全局 CSS 主题
pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(STYLE_CSS);
    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// 应用主题偏好:通过 adw::StyleManager 切换(跟随系统/浅色/深色)。
pub fn apply_theme(theme: ThemePref, _window: &adw::ApplicationWindow) {
    let manager = adw::StyleManager::default();
    match theme {
        ThemePref::System => {
            manager.set_color_scheme(adw::ColorScheme::Default);
        }
        ThemePref::Light => {
            manager.set_color_scheme(adw::ColorScheme::ForceLight);
        }
        ThemePref::Dark => {
            manager.set_color_scheme(adw::ColorScheme::ForceDark);
        }
    }
}
