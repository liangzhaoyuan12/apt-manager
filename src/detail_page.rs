use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box, Button, Label, Orientation, ScrolledWindow, Widget};

use crate::apt_worker::PkgDetail;
use crate::i18n;

/// 详情页
pub struct DetailPage {
    pub root: gtk::Box,
    title: Label,
    badge: Label,
    version_value: Label,
    summary_value: Label,
    desc_value: Label,
    section_value: Label,
    size_value: Label,
    install_btn: Button,
    update_btn: Button,
    remove_btn: Button,
    detail: Option<PkgDetail>,
}

impl DetailPage {
    pub fn new(
        on_back: impl Fn() + 'static,
        on_install: impl Fn(&str) + 'static,
        on_update: impl Fn(&str) + 'static,
        on_remove: impl Fn(&str) + 'static,
    ) -> Rc<RefCell<Self>> {
        let root = Box::new(Orientation::Vertical, 0);

        // ---- 顶栏:返回按钮 ----
        let top = Box::new(Orientation::Horizontal, 8);
        top.set_margin_top(10);
        top.set_margin_bottom(4);
        top.set_margin_start(12);
        top.set_margin_end(12);

        let back_btn = Button::with_label(i18n::t("detail_back"));
        back_btn.add_css_class("ghost-btn");
        back_btn.connect_clicked(move |_| on_back());
        top.append(&back_btn);

        // 返回按钮后留白
        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top.append(&spacer);
        root.append(&top);

        // ---- 内容卡片 ----
        let card = Box::new(Orientation::Vertical, 10);
        card.add_css_class("detail-card");
        card.set_margin_top(4);
        card.set_margin_bottom(12);
        card.set_margin_start(16);
        card.set_margin_end(16);
        card.set_vexpand(true);

        // 标题行:名称 + 徽章
        let title_row = Box::new(Orientation::Horizontal, 10);
        title_row.set_halign(Align::Start);
        let title = Label::new(None);
        title.add_css_class("detail-title");
        let badge = Label::new(None);
        badge.add_css_class("badge");
        title_row.append(&title);
        title_row.append(&badge);
        card.append(&title_row);

        // 版本
        let version_value = detail_row(&card, "detail_version");

        // 分类
        let section_value = detail_row(&card, "detail_section");

        // 大小
        let size_value = detail_row(&card, "detail_size");

        // 简介
        let summary_value = detail_row(&card, "detail_summary");

        // 详细描述
        let desc_label = Label::new(Some(i18n::t("detail_description")));
        desc_label.add_css_class("detail-label");
        desc_label.set_xalign(0.0);
        desc_label.set_margin_top(6);
        card.append(&desc_label);

        let desc_value = Label::new(None);
        desc_value.add_css_class("detail-desc");
        desc_value.set_wrap(true);
        desc_value.set_xalign(0.0);
        desc_value.set_selectable(true);
        desc_value.set_hexpand(true);
        desc_value.set_vexpand(true);
        let desc_scroll = ScrolledWindow::new();
        desc_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        desc_scroll.set_child(Some(&desc_value));
        desc_scroll.set_vexpand(true);
        desc_scroll.set_max_content_height(220);
        card.append(&desc_scroll);

        // ---- 底部操作按钮 ----
        let actions = Box::new(Orientation::Horizontal, 10);
        actions.set_halign(Align::End);
        actions.set_margin_top(8);

        let install_btn = Button::with_label(i18n::t("install"));
        install_btn.add_css_class("install-btn");

        let update_btn = Button::with_label(i18n::t("update"));
        update_btn.add_css_class("update-btn");

        let remove_btn = Button::with_label(i18n::t("uninstall"));
        remove_btn.add_css_class("danger-btn");

        actions.append(&install_btn);
        actions.append(&update_btn);
        actions.append(&remove_btn);
        card.append(&actions);

        root.append(&card);

        let page = Rc::new(RefCell::new(Self {
            root,
            title,
            badge,
            version_value,
            summary_value,
            desc_value,
            section_value,
            size_value,
            install_btn: install_btn.clone(),
            update_btn: update_btn.clone(),
            remove_btn: remove_btn.clone(),
            detail: None,
        }));

        let p = page.clone();
        install_btn.connect_clicked(move |_| {
            let p = p.borrow();
            if let Some(d) = &p.detail {
                let name = d.name.clone();
                on_install(&name);
            }
        });

        let p = page.clone();
        update_btn.connect_clicked(move |_| {
            let p = p.borrow();
            if let Some(d) = &p.detail {
                let name = d.name.clone();
                on_update(&name);
            }
        });

        let p = page.clone();
        remove_btn.connect_clicked(move |_| {
            let p = p.borrow();
            if let Some(d) = &p.detail {
                let name = d.name.clone();
                on_remove(&name);
            }
        });

        page
    }

    pub fn set_detail(&mut self, d: PkgDetail) {
        self.detail = Some(d.clone());
        self.title.set_text(&d.name);
        self.version_value
            .set_text(&format_versions(&d.installed_version, &d.candidate_version));
        self.section_value.set_text(if d.section.is_empty() { "—" } else { &d.section });
        self.size_value.set_text(&format_size(d.size));
        self.summary_value.set_text(if d.summary.is_empty() { "—" } else { &d.summary });
        self.desc_value.set_text(if d.description.is_empty() {
            i18n::t("detail_no_desc")
        } else {
            &d.description
        });

        // 徽章
        let (class, text) = match (d.installed, d.upgradable) {
            (true, true) => ("badge-both", i18n::t("installed_upgradable")),
            (true, false) => ("badge-installed", i18n::t("installed_badge")),
            (false, true) => ("badge-upgradable", i18n::t("upgradable_badge")),
            (false, false) => ("badge-not-installed", i18n::t("not_installed")),
        };
        self.badge.add_css_class(class);
        self.badge.set_text(text);

        // 按钮可见性
        self.install_btn.set_visible(!d.installed);
        self.update_btn.set_visible(d.upgradable);
        self.remove_btn.set_visible(d.installed);
    }
}

fn format_versions(installed: &str, candidate: &str) -> String {
    if installed.is_empty() {
        if candidate.is_empty() {
            "—".to_string()
        } else {
            candidate.to_string()
        }
    } else if candidate.is_empty() || installed == candidate {
        installed.to_string()
    } else {
        format!("{installed} → {candidate}")
    }
}

pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if size >= GB {
        i18n::t("size_gb").replace("{n}", &format!("{:.1}", size as f64 / GB as f64))
    } else if size >= MB {
        i18n::t("size_mb").replace("{n}", &format!("{:.1}", size as f64 / MB as f64))
    } else if size >= KB {
        i18n::t("size_kb").replace("{n}", &(size / KB).to_string())
    } else {
        i18n::t("size_b").replace("{n}", &size.to_string())
    }
}

/// 创建一行 "标签: 值" 并返回值 Label
fn detail_row(card: &Box, key: &str) -> Label {
    let row = Box::new(Orientation::Horizontal, 8);
    row.set_halign(Align::Start);

    let label = Label::new(Some(i18n::t(key)));
    label.add_css_class("detail-label");
    label.set_xalign(0.0);
    label.set_width_chars(14);

    let value = Label::new(None);
    value.add_css_class("detail-value");
    value.set_xalign(0.0);
    value.set_selectable(true);

    row.append(&label);
    row.append(&value);
    card.append(&row);
    value
}

impl DetailPage {
    #[allow(dead_code)]
    fn _keep(&self, _w: &Widget) {}
}
