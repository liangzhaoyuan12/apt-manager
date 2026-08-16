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

/// 当前界面语言是否为中文(后台线程也读此值,如 desktop 文件 Name 本地化)
pub fn current_is_zh() -> bool {
    CUR.load(Ordering::Relaxed) == 0
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
    ("auth_title", "需要授权"),
    ("auth_password", "密码"),
    ("auth_confirm", "授权"),
    ("auth_hint", "需要 root 权限执行此操作,请输入密码:"),
    ("deb_install", "安装 .deb 包"),
    ("deb_source", "来源文件"),
    ("deb_open_failed", "无法读取 deb 包信息"),
    ("about_title", "关于 APT 管理器"),
    ("about_version", "版本"),
    ("about_desc", "基于 GTK4 + Rust 的 Debian 系软件包管理器,轻量、快速、开箱即用。"),
    ("about_dev", "开发者"),
    ("about_license", "开源协议"),
    ("about_repo", "软件仓库"),
    ("search_placeholder", "搜索软件包…"),
    ("search", "搜索"),
    ("refresh", "刷新"),
    ("multi_select", "多选"),
    ("multi_exit", "退出多选"),
    ("multi_select_all", "全选"),
    ("multi_clear", "清空"),
    ("multi_count", "已选 {n} 项"),
    ("batch_install", "批量安装"),
    ("batch_remove", "批量卸载"),
    ("batch_update", "批量更新"),
    ("update_sources", "更新软件源"),
    ("check_updates", "检查更新"),
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
    ("detail_app_name", "应用名"),
    ("detail_package_name", "包名"),
    ("detail_arch", "架构"),
    ("detail_maintainer", "维护者"),
    ("detail_date", "更新时间"),
    ("detail_installed_size", "安装后占用"),
    ("detail_package_size", "安装包大小"),
    ("detail_url", "下载地址"),
    ("detail_desktop", "桌面文件"),
    ("detail_exec_cmd", "执行命令"),
    ("detail_exec_path", "执行路径"),
    ("detail_deps", "依赖"),
    ("detail_homepage", "主页"),
    ("detail_no_desc", "(无描述)"),
    ("install", "安装"),
    ("update", "更新"),
    ("uninstall", "卸载"),
    ("cancel", "取消"),
    ("confirm", "确定"),
    ("op_running", "正在执行操作…"),
    ("op_done", "操作完成"),
    ("op_failed", "操作失败"),
    ("op_close", "关闭"),
    ("no_admin_privilege", "当前用户没有管理员(sudo)权限,无法执行此操作。请将用户加入 sudo 组后再试。"),
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
    ("auth_title", "Authentication Required"),
    ("auth_password", "Password"),
    ("auth_confirm", "Authenticate"),
    ("auth_hint", "Root privileges are required for this operation. Enter your password:"),
    ("deb_install", "Install .deb Package"),
    ("deb_source", "Source File"),
    ("deb_open_failed", "Failed to read deb package information"),
    ("about_title", "About APT Manager"),
    ("about_version", "Version"),
    ("about_desc", "A lightweight, fast package manager for Debian-based systems, built with GTK4 + Rust."),
    ("about_dev", "Developer"),
    ("about_license", "License"),
    ("about_repo", "Repository"),
    ("search_placeholder", "Search packages…"),
    ("search", "Search"),
    ("refresh", "Refresh"),
    ("multi_select", "Multi-select"),
    ("multi_exit", "Done"),
    ("multi_select_all", "Select All"),
    ("multi_clear", "Clear"),
    ("multi_count", "{n} selected"),
    ("batch_install", "Install All"),
    ("batch_remove", "Remove All"),
    ("batch_update", "Update All"),
    ("update_sources", "Update Sources"),
    ("check_updates", "Check Updates"),
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
    ("detail_app_name", "App Name"),
    ("detail_package_name", "Package Name"),
    ("detail_arch", "Architecture"),
    ("detail_maintainer", "Maintainer"),
    ("detail_date", "Updated"),
    ("detail_installed_size", "Installed Size"),
    ("detail_package_size", "Package Size"),
    ("detail_url", "Download URL"),
    ("detail_desktop", "Desktop File"),
    ("detail_exec_cmd", "Exec Command"),
    ("detail_exec_path", "Exec Path"),
    ("detail_deps", "Dependencies"),
    ("detail_homepage", "Homepage"),
    ("detail_no_desc", "(no description)"),
    ("install", "Install"),
    ("update", "Update"),
    ("uninstall", "Remove"),
    ("cancel", "Cancel"),
    ("confirm", "Confirm"),
    ("op_running", "Operation in progress…"),
    ("op_done", "Operation completed"),
    ("op_failed", "Operation failed"),
    ("op_close", "Close"),
    ("no_admin_privilege", "The current user has no administrator (sudo) privileges, so this operation cannot be performed. Add the user to the sudo group and try again."),
    ("size_b", "{n} B"),
    ("size_kb", "{n} KB"),
    ("size_mb", "{n} MB"),
    ("size_gb", "{n} GB"),
];
