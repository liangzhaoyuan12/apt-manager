// GTK 4.10+ Dialog 标记弃用,但 AlertDialog 不适合流式输出场景,此处保留使用
#![allow(deprecated)]
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use glib::translate::FromGlibPtrNone;
use gtk::prelude::*;
use gtk::{
    Align, ApplicationWindow, Box, Button, Dialog, Entry, Grid, Image, Label, Orientation,
    ScrolledWindow, Separator, Spinner, TextView,
};

use crate::apt_worker::run_apt_op;
use crate::i18n;

/// 主窗口指针(后台线程请求密码时在主线程重建窗口用)
static MAIN_WINDOW: AtomicUsize = AtomicUsize::new(0);

/// 记录主窗口(应用启动时调用一次)
pub fn set_main_window(w: &ApplicationWindow) {
    MAIN_WINDOW.store(w.as_ptr() as usize, Ordering::Relaxed);
}

/// 取主窗口(借用,不增加引用计数;应用存活期间有效)
fn main_window() -> Option<ApplicationWindow> {
    let ptr = MAIN_WINDOW.load(Ordering::Relaxed) as *mut gtk::ffi::GtkApplicationWindow;
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { ApplicationWindow::from_glib_none(ptr) })
}

/// 授权密码框:后台线程经主循环调用,on_result 收到 Some(密码) 或 None(取消/关闭)。
/// 这是 apt-manager 自己的模态子窗口,替代系统 polkit 认证框(dock 归组正常)。
pub fn show_password_prompt(on_result: impl FnOnce(Option<String>) + 'static) {
    let Some(win) = main_window() else {
        on_result(None);
        return;
    };

    let dialog = Dialog::new();
    dialog.set_title(Some(i18n::t("auth_title")));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&win));
    dialog.add_css_class("about-dialog");

    let content = Box::new(Orientation::Vertical, 10);
    content.set_margin_top(20);
    content.set_margin_bottom(16);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_width_request(380);

    let icon = gtk::Image::from_icon_name("dialog-password");
    icon.set_pixel_size(48);
    icon.set_halign(Align::Center);
    content.append(&icon);

    let msg = Label::new(Some(i18n::t("auth_hint")));
    msg.set_wrap(true);
    msg.set_justify(gtk::Justification::Center);
    msg.set_halign(Align::Center);
    msg.add_css_class("about-desc");
    content.append(&msg);

    let entry = Entry::new();
    entry.set_visibility(false);
    entry.set_placeholder_text(Some(i18n::t("auth_password")));
    entry.set_hexpand(true);
    content.append(&entry);

    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(Align::End);
    let cancel = Button::with_label(i18n::t("cancel"));
    let ok = Button::with_label(i18n::t("auth_confirm"));
    ok.add_css_class("suggested-action");
    ok.set_sensitive(false);
    btns.append(&cancel);
    btns.append(&ok);
    content.append(&btns);

    dialog.content_area().append(&content);

    // 结果只回调一次(取消/确认/回车/关窗 多入口)
    // 注意:此模块导入了 gtk::Box,堆分配必须用全路径 std::boxed::Box
    let slot: Rc<RefCell<Option<std::boxed::Box<dyn FnOnce(Option<String>)>>>> =
        Rc::new(RefCell::new(Some(std::boxed::Box::new(on_result))));

    let d = dialog.clone();
    let s = slot.clone();
    cancel.connect_clicked(move |_| {
        if let Some(f) = s.borrow_mut().take() {
            f(None);
        }
        d.close();
    });

    let d = dialog.clone();
    let e = entry.clone();
    let s = slot.clone();
    ok.connect_clicked(move |_| {
        let pw = e.text().to_string();
        if let Some(f) = s.borrow_mut().take() {
            f(Some(pw));
        }
        d.close();
    });

    let ok = ok.clone();
    entry.connect_changed(move |e| ok.set_sensitive(!e.text().is_empty()));

    let d = dialog.clone();
    let e2 = entry.clone();
    let s = slot.clone();
    entry.connect_activate(move |_| {
        if !e2.text().is_empty() {
            let pw = e2.text().to_string();
            if let Some(f) = s.borrow_mut().take() {
                f(Some(pw));
            }
            d.close();
        }
    });

    // ESC / 窗口关闭按钮 → 视为取消
    let s = slot.clone();
    dialog.connect_close_request(move |_| {
        if let Some(f) = s.borrow_mut().take() {
            f(None);
        }
        glib::Propagation::Proceed
    });

    dialog.present();
    entry.grab_focus();
}

/// 应用图标名(打包时安装到 hicolor)
const APP_ICON_NAME: &str = "com.liangzhaoyuan12.aptmanager";

/// 关于对话框(GNOME 风格:图标 + 名称 + 版本 + 描述 + 信息行 + 链接)
pub fn show_about(parent: &ApplicationWindow) {
    let dialog = Dialog::new();
    dialog.set_title(Some(i18n::t("about_title")));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));
    dialog.set_default_size(420, -1);
    dialog.add_css_class("about-dialog");

    let content = Box::new(Orientation::Vertical, 6);
    content.set_margin_top(28);
    content.set_margin_bottom(22);
    content.set_margin_start(36);
    content.set_margin_end(36);

    // 应用图标(未安装时回退系统图标)
    let icon_name = if gtk::IconTheme::default().has_icon(APP_ICON_NAME) {
        APP_ICON_NAME
    } else {
        "applications-system"
    };
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(96);
    icon.set_halign(Align::Center);
    icon.set_margin_bottom(6);
    content.append(&icon);

    // 应用名
    let title = Label::new(Some(i18n::t("app_name")));
    title.add_css_class("about-app-name");
    title.set_halign(Align::Center);
    content.append(&title);

    // 版本
    let version_text = format!("{} {}", i18n::t("about_version"), env!("CARGO_PKG_VERSION"));
    let version = Label::new(Some(&version_text));
    version.add_css_class("about-version");
    version.set_halign(Align::Center);
    version.set_margin_bottom(10);
    content.append(&version);

    // 简短描述(居中换行)
    let desc = Label::new(Some(i18n::t("about_desc")));
    desc.add_css_class("about-desc");
    desc.set_wrap(true);
    desc.set_justify(gtk::Justification::Center);
    desc.set_halign(Align::Center);
    desc.set_margin_bottom(8);
    content.append(&desc);

    content.append(&Separator::new(Orientation::Horizontal));

    // 信息行:开发者 / 主页(可点击)/ 开源协议
    let info = Grid::new();
    info.set_column_spacing(16);
    info.set_row_spacing(4);
    info.set_halign(Align::Center);
    info.set_margin_top(8);
    info.set_margin_bottom(8);
    about_row(&info, 0, "about_dev", "liangzhaoyuan12", false);
    about_row(&info, 1, "about_repo", "https://github.com/liangzhaoyuan12/apt-manager", true);
    about_row(&info, 2, "about_license", "MIT", false);
    content.append(&info);

    content.append(&Separator::new(Orientation::Horizontal));

    // 关闭按钮(居中,建议样式)
    let close_btn = Button::with_label(i18n::t("op_close"));
    close_btn.add_css_class("suggested-action");
    close_btn.set_halign(Align::Center);
    close_btn.set_margin_top(10);
    content.append(&close_btn);

    dialog.content_area().append(&content);
    close_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });
    dialog.present();
}

/// 关于对话框信息行:左侧灰色标签(右对齐)+ 右侧值。link=true 时值为可点击超链接。
fn about_row(grid: &Grid, row: i32, label_key: &str, value: &str, link: bool) {
    let label = Label::new(Some(i18n::t(label_key)));
    label.add_css_class("about-info-label");
    label.set_halign(Align::End);

    let val = Label::new(None);
    val.add_css_class("about-info-value");
    val.set_halign(Align::Start);
    val.set_selectable(true);
    if link {
        val.set_markup(&format!("<a href=\"{value}\">{value}</a>"));
        val.connect_activate_link(|_l, uri| {
            // timestamp 0 = 无事件时间,GTK 允许
            gtk::show_uri(None::<&gtk::Window>, uri, 0);
            glib::Propagation::Stop
        });
    } else {
        val.set_text(value);
    }

    grid.attach(&label, 0, row, 1, 1);
    grid.attach(&val, 1, row, 1, 1);
}

/// 执行操作对话框:弹窗显示 apt-get 输出,结束后回调
pub fn run_op_dialog(
    parent: &ApplicationWindow,
    title: &str,
    args: &[&str],
    on_finished: impl Fn() + 'static,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some(title));
    dialog.set_modal(true);
    // 关键:必须挂到主窗口,否则是独立顶层窗口,dock 不归组
    dialog.set_transient_for(Some(parent));
    dialog.set_default_size(560, 360);
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
    dialog.content_area().append(&content);

    dialog.present();

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
