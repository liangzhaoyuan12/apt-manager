use std::sync::atomic::{AtomicU8, Ordering};

use crate::config::LangPref;

/// 当前生效语言:0=zh, 1=en
static CUR: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(pref: LangPref) {
    let zh = match pref {
        LangPref::Zh => true,
        LangPref::En => false,
        LangPref::System => system_is_zh(),
    };
    CUR.store(if zh { 0 } else { 1 }, Ordering::Relaxed);
}

/// 跟随系统:根据 LANG/LC_ALL 判断是否为中文环境
pub fn system_is_zh() -> bool {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    lang.to_lowercase().starts_with("zh")
        || lang.to_lowercase().starts_with("cmn")
        || lang.to_lowercase().contains("_cn")
}

pub fn t(key: &str) -> &'static str {
    if CUR.load(Ordering::Relaxed) == 0 {
        ZH.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or("")
    } else {
        EN.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or("")
    }
}

macro_rules! tr {
    ($key:literal) => {
        crate::i18n::t($key)
    };
}
pub(crate) use tr;

// 语言名(用于下拉框显示)
pub fn lang_self_name(pref: LangPref) -> &'static str {
    match pref {
        LangPref::System => t("lang_system"),
        LangPref::Zh => "简体中文",
        LangPref::En => "English",
    }
}

pub fn theme_self_name(pref: crate::config::ThemePref) -> &'static str {
    match pref {
        crate::config::ThemePref::System => t("theme_system"),
        crate::config::ThemePref::Light => t("theme_light"),
        crate::config::ThemePref::Dark => t("theme_dark"),
    }
}

static ZH: &[(&str, &str)] = &[
    ("app_name", "APT 管理器"),
    ("tab_all", "所有软件"),
    ("tab_installed", "已安装"),
    ("tab_upgradable", "可更新"),
    ("theme", "主题"),
    ("theme_system", "跟随系统"),
    ("theme_light", "浅色"),
    ("theme_dark", "深色"),
    ("language", "语言"),
    ("lang_system", "跟随系统"),
    ("menu", "菜单"),
    ("about", "关于"),
    ("about_title", "关于 APT 管理器"),
    ("about_dev", "开发者"),
    ("about_license", "开源协议"),
    ("about_repo", "软件仓库"),
    ("search_placeholder", "搜索软件包…"),
    ("search", "搜索"),
    ("refresh", "刷新"),
    ("update_sources", "更新软件源"),
    ("check_updates", "检查更新"),
    ("confirm_check_updates", "确定要检查软件更新(apt update)吗?"),
    ("auto_remove", "自动移除无用依赖"),
    ("upgrade_all", "全部更新"),
    ("installed_badge", "已安装"),
    ("upgradable_badge", "可更新"),
    ("installed_upgradable", "已安装 · 可更新"),
    ("not_installed", "未安装"),
    ("loading", "正在加载软件列表…"),
    ("empty", "没有匹配的软件"),
    ("page", "第 {page} 页 / 共 {total} 页"),
    ("prev", "上一页"),
    ("next", "下一页"),
    ("total_pkgs", "共 {n} 个软件包"),
    ("detail_back", "返回"),
    ("detail_version", "版本"),
    ("detail_installed_version", "已安装版本"),
    ("detail_candidate_version", "可安装版本"),
    ("detail_summary", "简介"),
    ("detail_description", "详细描述"),
    ("detail_section", "分类"),
    ("detail_size", "大小"),
    ("detail_deps", "依赖"),
    ("detail_homepage", "主页"),
    ("detail_no_desc", "(无描述)"),
    ("install", "安装"),
    ("update", "更新"),
    ("uninstall", "卸载"),
    ("cancel", "取消"),
    ("confirm", "确定"),
    ("confirm_title", "确认操作"),
    ("confirm_install", "确定要安装 {pkg} 吗?"),
    ("confirm_update", "确定要更新 {pkg} 吗?"),
    ("confirm_uninstall", "确定要卸载 {pkg} 吗?"),
    ("confirm_autoremove", "确定要自动移除不再需要的依赖包吗?"),
    ("confirm_update_sources", "确定要更新软件源(apt update)吗?"),
    ("confirm_upgrade_all", "确定要更新所有可更新的软件吗?"),
    ("op_running", "正在执行操作…"),
    ("op_done", "操作完成"),
    ("op_failed", "操作失败"),
    ("op_close", "关闭"),
    ("requires_root", "此操作需要 root 权限,将弹出授权窗口。"),
    ("size_b", "{n} B"),
    ("size_kb", "{n} KB"),
    ("size_mb", "{n} MB"),
    ("size_gb", "{n} GB"),
];

static EN: &[(&str, &str)] = &[
    ("app_name", "APT Manager"),
    ("tab_all", "All Packages"),
    ("tab_installed", "Installed"),
    ("tab_upgradable", "Upgradable"),
    ("theme", "Theme"),
    ("theme_system", "Follow System"),
    ("theme_light", "Light"),
    ("theme_dark", "Dark"),
    ("language", "Language"),
    ("lang_system", "Follow System"),
    ("menu", "Menu"),
    ("about", "About"),
    ("about_title", "About APT Manager"),
    ("about_dev", "Developer"),
    ("about_license", "License"),
    ("about_repo", "Repository"),
    ("search_placeholder", "Search packages…"),
    ("search", "Search"),
    ("refresh", "Refresh"),
    ("update_sources", "Update Sources"),
    ("check_updates", "Check Updates"),
    ("confirm_check_updates", "Check for package updates (apt update)?"),
    ("auto_remove", "Auto Remove"),
    ("upgrade_all", "Upgrade All"),
    ("installed_badge", "Installed"),
    ("upgradable_badge", "Upgradable"),
    ("installed_upgradable", "Installed · Upgradable"),
    ("not_installed", "Not Installed"),
    ("loading", "Loading package list…"),
    ("empty", "No matching packages"),
    ("page", "Page {page} of {total}"),
    ("prev", "Previous"),
    ("next", "Next"),
    ("total_pkgs", "{n} packages in total"),
    ("detail_back", "Back"),
    ("detail_version", "Version"),
    ("detail_installed_version", "Installed Version"),
    ("detail_candidate_version", "Candidate Version"),
    ("detail_summary", "Summary"),
    ("detail_description", "Description"),
    ("detail_section", "Section"),
    ("detail_size", "Size"),
    ("detail_deps", "Dependencies"),
    ("detail_homepage", "Homepage"),
    ("detail_no_desc", "(no description)"),
    ("install", "Install"),
    ("update", "Update"),
    ("uninstall", "Remove"),
    ("cancel", "Cancel"),
    ("confirm", "Confirm"),
    ("confirm_title", "Confirm Action"),
    ("confirm_install", "Install {pkg}?"),
    ("confirm_update", "Update {pkg}?"),
    ("confirm_uninstall", "Remove {pkg}?"),
    ("confirm_autoremove", "Auto-remove packages that are no longer needed?"),
    ("confirm_update_sources", "Update package sources (apt update)?"),
    ("confirm_upgrade_all", "Upgrade all upgradable packages?"),
    ("op_running", "Operation in progress…"),
    ("op_done", "Operation completed"),
    ("op_failed", "Operation failed"),
    ("op_close", "Close"),
    ("requires_root", "This operation requires root privileges; an authorization dialog will appear."),
    ("size_b", "{n} B"),
    ("size_kb", "{n} KB"),
    ("size_mb", "{n} MB"),
    ("size_gb", "{n} GB"),
];
