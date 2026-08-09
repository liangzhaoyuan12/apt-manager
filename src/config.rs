use std::fs;
use std::path::PathBuf;

/// 持久化配置(主题/语言记忆)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePref {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangPref {
    System,
    Zh,
    En,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub theme: ThemePref,
    pub language: LangPref,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: ThemePref::System,
            language: LangPref::System,
        }
    }
}

fn config_path() -> PathBuf {
    let dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config"));
    dir.join("apt-manager").join("config")
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = Config::default();
        if let Ok(content) = fs::read_to_string(config_path()) {
            for line in content.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                match (k.trim(), v.trim()) {
                    ("theme", "light") => cfg.theme = ThemePref::Light,
                    ("theme", "dark") => cfg.theme = ThemePref::Dark,
                    ("theme", "system") => cfg.theme = ThemePref::System,
                    ("language", "zh") => cfg.language = LangPref::Zh,
                    ("language", "en") => cfg.language = LangPref::En,
                    ("language", "system") => cfg.language = LangPref::System,
                    _ => {}
                }
            }
        }
        cfg
    }

    pub fn save(&self) {
        let theme = match self.theme {
            ThemePref::System => "system",
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        };
        let lang = match self.language {
            LangPref::System => "system",
            LangPref::Zh => "zh",
            LangPref::En => "en",
        };
        let content = format!("theme={theme}\nlanguage={lang}\n");
        if let Some(dir) = config_path().parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(config_path(), content);
    }
}
