use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use glib::translate::FromGlibPtrNone;
use gtk::gdk::prelude::*;
use gtk::prelude::*;
use adw::prelude::*;
use gtk::{
    Align, Box, Button, Image, Label, Orientation,
    ScrolledWindow, Separator, Spinner, TextView,
};

use crate::apt_worker::run_apt_op;
use crate::i18n;

/// 主窗口指针(后台线程请求密码时在主线程重建窗口用)
static MAIN_WINDOW: AtomicUsize = AtomicUsize::new(0);

/// 记录主窗口(应用启动时调用一次)
pub fn set_main_window(w: &adw::ApplicationWindow) {
    MAIN_WINDOW.store(w.as_ptr() as usize, Ordering::Relaxed);
}

/// 取主窗口(借用,不增加引用计数;应用存活期间有效)
pub(crate) fn main_window() -> Option<adw::ApplicationWindow> {
    let ptr = MAIN_WINDOW.load(Ordering::Relaxed) as *mut adw::ffi::AdwApplicationWindow;
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { adw::ApplicationWindow::from_glib_none(ptr) })
}

/// 应用图标名(打包时安装到 hicolor)
const APP_ICON_NAME: &str = "com.liangzhaoyuan12.aptmanager";

/// .deb 安装包控制信息
struct DebInfo {
    package: String,
    version: String,
    architecture: String,
    maintainer: String,
    installed_size: u64, // KB
    depends: String,
    homepage: String,
    description: String,
    file_size: u64,
}

/// 读取 .deb 控制信息(走 dpkg-deb -f,无需额外压缩库)
fn read_deb_info(path: &str) -> Option<DebInfo> {
    let out = std::process::Command::new("dpkg-deb")
        .arg("-f")
        .arg(path)
        .args([
            "Package",
            "Version",
            "Architecture",
            "Maintainer",
            "Installed-Size",
            "Depends",
            "Homepage",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let mut next = || lines.next().unwrap_or("").trim().to_string();
    let package = next();
    let version = next();
    let architecture = next();
    let maintainer = next();
    let installed_size = next().parse().unwrap_or(0);
    let depends = next();
    let homepage = next();
    // Description 多行,单独取
    let desc_out = std::process::Command::new("dpkg-deb")
        .arg("-f")
        .arg(path)
        .arg("Description")
        .output()
        .ok()?;
    let description = String::from_utf8_lossy(&desc_out.stdout).trim().to_string();
    let file_size = std::fs::metadata(path).ok()?.len();
    Some(DebInfo {
        package,
        version,
        architecture,
        maintainer,
        installed_size,
        depends,
        homepage,
        description,
        file_size,
    })
}

/// .deb 包安装详情对话框:显示控制信息 + 安装按钮。
/// on_installed 在安装完成后回调(主窗口刷新用)。
pub fn show_deb_dialog(path: &str, on_installed: impl Fn() + 'static) {
    let win = main_window(); // 可能为空(纯 deb 打开时无主窗口)
    let Some(info) = read_deb_info(path) else {
        eprintln!("无法读取 deb 包信息: {path}");
        show_deb_error(win.as_ref(), path);
        return;
    };

    let dialog = adw::Dialog::new();
    dialog.set_title(i18n::t("deb_install"));
    dialog.set_content_width(480);
    dialog.set_content_height(400);
    dialog.add_css_class("about-dialog");

    let root_content = Box::new(Orientation::Vertical, 8);
    root_content.set_margin_top(16);
    root_content.set_margin_bottom(12);
    root_content.set_margin_start(20);
    root_content.set_margin_end(20);

    // ---- 顶栏:图标 + 包名/版本(左),安装按钮(右上)----
    let top = Box::new(Orientation::Horizontal, 10);

    let icon = Image::from_icon_name("package-x-generic");
    icon.set_pixel_size(40);
    icon.set_valign(Align::Center);
    top.append(&icon);

    let title_box = Box::new(Orientation::Vertical, 0);
    title_box.set_valign(Align::Center);
    let name = Label::new(Some(&info.package));
    name.add_css_class("about-app-name");
    name.set_xalign(0.0);
    title_box.append(&name);
    let ver = Label::new(Some(&info.version));
    ver.add_css_class("about-version");
    ver.set_xalign(0.0);
    title_box.append(&ver);
    top.append(&title_box);

    let spacer = Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&spacer);

    let install = Button::with_label(i18n::t("install"));
    install.add_css_class("suggested-action");
    top.append(&install);
    root_content.append(&top);

    // ---- 滚动区:信息(单列)+ 描述,只保留垂直滚动 ----
    let scroll = ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);

    let info_content = Box::new(Orientation::Vertical, 8);
    info_content.set_margin_top(4);

    let info_box = Box::new(Orientation::Vertical, 4);
    deb_row(&info_box, "detail_maintainer", &info.maintainer);
    deb_row(&info_box, "detail_arch", &info.architecture);
    deb_row(
        &info_box,
        "detail_installed_size",
        &crate::detail_page::format_size(info.installed_size.saturating_mul(1024)),
    );
    deb_row(
        &info_box,
        "detail_package_size",
        &crate::detail_page::format_size(info.file_size),
    );
    deb_row(&info_box, "deb_source", path);
    deb_row(&info_box, "detail_homepage", &info.homepage);
    deb_row(&info_box, "detail_deps", &info.depends);
    info_content.append(&info_box);

    info_content.append(&Separator::new(Orientation::Horizontal));
    let desc_label = Label::new(Some(i18n::t("detail_description")));
    desc_label.add_css_class("about-info-label");
    desc_label.set_xalign(0.0);
    info_content.append(&desc_label);
    let desc = Label::new(Some(if info.description.is_empty() {
        "—"
    } else {
        &info.description
    }));
    desc.add_css_class("about-desc");
    desc.set_wrap(true);
    desc.set_xalign(0.0);
    desc.set_selectable(true);
    info_content.append(&desc);

    scroll.set_child(Some(&info_content));
    root_content.append(&scroll);

    // ---- 底部:关闭 ----
    let close_row = Box::new(Orientation::Horizontal, 8);
    close_row.set_halign(Align::End);
    let close = Button::with_label(i18n::t("op_close"));
    close.add_css_class("ghost-btn");
    close_row.append(&close);
    root_content.append(&close_row);

    dialog.set_child(Some(&root_content));
    close.connect_clicked({
        let d = dialog.clone();
        move |_| { d.close(); }
    });
    // on_installed 只能触发一次,用 RefCell 包装供 Fn 闭包取用
    let on_installed = Rc::new(RefCell::new(Some(on_installed)));
    install.connect_clicked({
        let d = dialog.clone();
        let path = path.to_string();
        let package = info.package.clone();
        let win2 = win.clone();
        let cb = on_installed.clone();
        move |_| {
            d.close();
            let title = format!("{}: {package}", i18n::t("install"));
            let args = vec!["install", "-y", path.as_str()];
            if let Some(cb) = cb.borrow_mut().take() {
                run_op_dialog(win2.as_ref(), &title, &args, cb);
            }
        }
    });

    if let Some(w) = &win {
        dialog.present(Some(w));
    } else {
        dialog.present(None::<&adw::ApplicationWindow>);
    }
}

/// deb 读取失败提示框(避免静默无反应)
fn show_deb_error(win: Option<&adw::ApplicationWindow>, path: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(i18n::t("deb_open_failed"))
        .body(path)
        .build();
    dialog.add_response("close", i18n::t("op_close"));
    dialog.set_response_appearance("close", adw::ResponseAppearance::Suggested);
    if let Some(w) = win {
        dialog.present(Some(w));
    } else {
        dialog.present(None::<&adw::ApplicationWindow>);
    }
}

/// 信息行:左侧灰色标签 + 右侧可换行值(单列)
fn deb_row(parent: &Box, key: &str, value: &str) {
    let row = Box::new(Orientation::Horizontal, 10);
    row.set_hexpand(true);

    let label = Label::new(Some(i18n::t(key)));
    label.add_css_class("about-info-label");
    label.set_xalign(0.0);
    label.set_valign(Align::Start);
    label.set_width_chars(14);

    let val = Label::new(Some(if value.is_empty() { "—" } else { value }));
    val.add_css_class("about-info-value");
    val.set_xalign(0.0);
    val.set_selectable(true);
    val.set_wrap(true);

    row.append(&label);
    row.append(&val);
    parent.append(&row);
}

/// 关于对话框(GNOME 风格:使用 adw::AboutWindow)
pub fn show_about(parent: &adw::ApplicationWindow) {
    let about = adw::AboutWindow::builder()
        .transient_for(parent)
        .modal(true)
        .application_name(i18n::t("app_name"))
        .application_icon(APP_ICON_NAME)
        .developer_name("liangzhaoyuan12")
        .version(env!("CARGO_PKG_VERSION"))
        .developers(vec!["liangzhaoyuan12"])
        .copyright("© 2024 liangzhaoyuan12")
        .license_type(gtk::License::MitX11)
        .website("https://github.com/liangzhaoyuan12/apt-manager")
        .issue_url("https://github.com/liangzhaoyuan12/apt-manager/issues")
        .build();
    about.present();
}

/// 执行操作对话框:弹窗显示 apt-get 输出,结束后回调
pub fn run_op_dialog(
    parent: Option<&adw::ApplicationWindow>,
    title: &str,
    args: &[&str],
    on_finished: impl Fn() + 'static,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title(title);
    dialog.set_content_width(560);
    dialog.set_content_height(360);
    dialog.add_css_class("op-dialog");

    let content = Box::new(Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let top = Box::new(Orientation::Horizontal, 8);
    let spinner = Spinner::new();
    spinner.set_size_request(20, 20);
    spinner.start();
    let status = Label::new(Some(i18n::t("op_running")));
    status.add_css_class("loading-label");
    top.append(&spinner);
    top.append(&status);

    let text_view = TextView::new();
    text_view.set_editable(false);
    text_view.set_monospace(true);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk::WrapMode::WordChar);
    text_view.set_vexpand(true);
    text_view.add_css_class("op-output");
    let buffer = text_view.buffer();

    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&text_view));
    scrolled.set_vexpand(true);

    let close_btn = Button::with_label(i18n::t("op_close"));
    close_btn.add_css_class("ghost-btn");
    close_btn.set_halign(Align::End);
    close_btn.set_sensitive(false);

    content.append(&top);
    content.append(&scrolled);
    content.append(&close_btn);
    dialog.set_child(Some(&content));

    if let Some(p) = parent {
        dialog.present(Some(p));
    } else {
        dialog.present(None::<&adw::ApplicationWindow>);
    }

    // 输出追加到文本框:后台线程写 mpsc,主循环 timeout 轮询刷新
    let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<bool>();

    let on_line = move |line: String| {
        let _ = line_tx.send(line);
    };

    let on_done = move |ok: bool| {
        let _ = done_tx.send(ok);
    };

    let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    run_apt_op(args, on_line, on_done);

    // 操作结束标志:done 源置位后,输出源再 drain 一次即停止,
    // 避免每次操作都永久泄漏一个 40ms 轮询源
    let done_flag = Arc::new(AtomicBool::new(false));

    // 主循环轮询输出行
    let buf = buffer.clone();
    let df = done_flag.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        if df.load(Ordering::Relaxed) {
            // 操作已结束:drain 剩余行后停止本源
            while let Ok(line) = line_rx.try_recv() {
                let mut end = buf.end_iter();
                buf.insert(&mut end, &format!("{line}\n"));
            }
            return glib::ControlFlow::Break;
        }
        while let Ok(line) = line_rx.try_recv() {
            let mut end = buf.end_iter();
            buf.insert(&mut end, &format!("{line}\n"));
        }
        glib::ControlFlow::Continue
    });

    // 主循环轮询完成状态
    let df = done_flag.clone();
    let status = status.clone();
    let spinner = spinner.clone();
    let close_btn = close_btn.clone();
    let dialog = dialog.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        match done_rx.try_recv() {
            Ok(ok) => {
                df.store(true, Ordering::Relaxed);
                spinner.stop();
                if ok {
                    status.set_text(i18n::t("op_done"));
                    status.add_css_class("badge-installed");
                } else {
                    status.set_text(i18n::t("op_failed"));
                    status.add_css_class("badge-upgradable");
                }
                close_btn.set_sensitive(true);
                let d = dialog.clone();
                close_btn.connect_clicked(move |_| {
                    d.close();
                });
                on_finished();
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}
