// GTK 4.10+ Dialog 标记弃用,但 AlertDialog 不适合流式输出场景,此处保留使用
#![allow(deprecated)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{
    Align, ApplicationWindow, Box, Button, Dialog, Label, Orientation, ScrolledWindow, Spinner,
    TextView,
};

use crate::apt_worker::run_apt_op;
use crate::i18n;

/// 关于对话框
pub fn show_about(parent: &ApplicationWindow) {
    let message = i18n::t("about_title");
    let dev = format!("{}: liangzhaoyuan12", i18n::t("about_dev"));
    let license = format!("{}: MIT", i18n::t("about_license"));
    let repo = format!("{}: https://github.com/liangzhaoyuan12/apt-manager", i18n::t("about_repo"));
    let detail = format!("{dev}\n{license}\n{repo}");

    let dialog = Dialog::new();
    dialog.set_title(Some(&message));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let content = Box::new(Orientation::Vertical, 8);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let msg_label = Label::new(Some(&message));
    msg_label.set_halign(Align::Start);
    let detail_label = Label::new(Some(&detail));
    detail_label.set_selectable(true);
    detail_label.set_halign(Align::Start);
    detail_label.set_wrap(true);
    content.append(&msg_label);
    content.append(&detail_label);

    let close_btn = Button::with_label(i18n::t("op_close"));
    close_btn.set_halign(Align::End);
    close_btn.add_css_class("ghost-btn");
    content.append(&close_btn);

    dialog.content_area().append(&content);
    close_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });
    dialog.present();
}

/// 确认对话框,回调返回是否确认
pub fn confirm(
    parent: &ApplicationWindow,
    message: &str,
    detail: &str,
    on_confirm: impl Fn() + 'static,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some(message));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let content = Box::new(Orientation::Vertical, 8);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let msg_label = Label::new(Some(message));
    msg_label.set_halign(Align::Start);
    let detail_label = Label::new(Some(detail));
    detail_label.set_selectable(true);
    detail_label.set_halign(Align::Start);
    detail_label.set_wrap(true);
    content.append(&msg_label);
    content.append(&detail_label);

    let btn_row = Box::new(Orientation::Horizontal, 8);
    btn_row.set_halign(Align::End);
    let cancel_btn = Button::with_label(i18n::t("cancel"));
    let confirm_btn = Button::with_label(i18n::t("confirm"));
    confirm_btn.add_css_class("suggested-action");
    btn_row.append(&cancel_btn);
    btn_row.append(&confirm_btn);
    content.append(&btn_row);

    dialog.content_area().append(&content);
    cancel_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });
    confirm_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            on_confirm();
            dialog.close();
        }
    });
    dialog.present();
}

/// 执行操作对话框:弹窗显示 apt-get 输出,结束后回调
pub fn run_op_dialog(
    _parent: &ApplicationWindow,
    title: &str,
    args: &[&str],
    on_finished: impl Fn() + 'static,
) {
    let dialog = Dialog::new();
    dialog.set_title(Some(title));
    dialog.set_modal(true);
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
