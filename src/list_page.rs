use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SearchEntry, Widget};

use crate::apt_worker::PkgInfo;
use crate::i18n;

/// 列表页模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 所有软件
    All,
    /// 已安装(含可更新)
    Installed,
    /// 可更新
    Upgradable,
}

/// 通用软件列表页:搜索 + 分页(每页 50)
pub struct PkgListPage {
    pub root: gtk::Box,
    list: ListBox,
    search: SearchEntry,
    page_label: Label,
    total_label: Label,
    prev_btn: Button,
    next_btn: Button,
    page: usize,
    page_size: usize,
    mode: Mode,
    query: String,
    /// 当前过滤后的数据(快照索引)
    filtered: Vec<usize>,
    /// 完整快照
    snapshot: Vec<PkgInfo>,
    /// row -> 快照索引 映射
    row_map: HashMap<Widget, usize>,
}

impl PkgListPage {
    pub fn new(
        mode: Mode,
        on_open_detail: impl Fn(&str) + 'static,
        on_action: Option<std::boxed::Box<dyn Fn()>>,
        on_check: Option<std::boxed::Box<dyn Fn()>>,
    ) -> Rc<RefCell<Self>> {
        let page_size = 50;

        let root = Box::new(Orientation::Vertical, 0);

        // ---- 顶部窄条:居中搜索 + 右侧操作按钮 ----
        let top = Box::new(Orientation::Horizontal, 8);
        top.set_margin_top(10);
        top.set_margin_bottom(4);
        top.set_margin_start(12);
        top.set_margin_end(12);

        let search = SearchEntry::new();
        search.set_placeholder_text(Some(i18n::t("search_placeholder")));
        search.set_hexpand(true);
        search.set_halign(Align::Center);
        search.set_width_chars(38);
        search.add_css_class("search-bar");

        let search_btn = Button::with_label(i18n::t("search"));
        search_btn.add_css_class("ghost-btn");

        let search_box = Box::new(Orientation::Horizontal, 4);
        search_box.set_halign(Align::Center);
        search_box.set_hexpand(true);
        search_box.append(&search);
        search_box.append(&search_btn);

        let action_btn = Button::new();
        let check_btn = Button::new();
        check_btn.set_visible(mode == Mode::Upgradable);
        match mode {
            Mode::All => {
                action_btn.set_label(i18n::t("update_sources"));
                action_btn.add_css_class("ghost-btn");
                if let Some(cb) = on_action {
                    action_btn.connect_clicked(move |_| cb());
                }
            }
            Mode::Installed => {
                action_btn.set_label(i18n::t("auto_remove"));
                action_btn.add_css_class("danger-btn");
                if let Some(cb) = on_action {
                    action_btn.connect_clicked(move |_| cb());
                }
            }
            Mode::Upgradable => {
                // 检查更新(apt update)+ 全部更新
                check_btn.set_label(i18n::t("check_updates"));
                check_btn.add_css_class("ghost-btn");
                if let Some(cb) = on_check {
                    check_btn.connect_clicked(move |_| cb());
                }
                action_btn.set_label(i18n::t("upgrade_all"));
                action_btn.add_css_class("update-btn");
                if let Some(cb) = on_action {
                    action_btn.connect_clicked(move |_| cb());
                }
            }
        }

        top.append(&search_box);
        top.append(&check_btn);
        top.append(&action_btn);
        root.append(&top);

        // ---- 中部:滚动列表 ----
        let list = ListBox::new();
        list.add_css_class("pkg-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_vexpand(true);

        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&list));
        scrolled.set_vexpand(true);
        root.append(&scrolled);

        // ---- 底部:分页 ----
        let pager = Box::new(Orientation::Horizontal, 8);
        pager.add_css_class("pager");
        pager.set_margin_top(6);
        pager.set_margin_bottom(10);
        pager.set_margin_start(12);
        pager.set_margin_end(12);

        let total_label = Label::new(None);
        total_label.set_halign(Align::Start);
        total_label.set_hexpand(true);

        let prev_btn = Button::with_label(i18n::t("prev"));
        let page_label = Label::new(None);
        let next_btn = Button::with_label(i18n::t("next"));

        pager.append(&total_label);
        pager.append(&prev_btn);
        pager.append(&page_label);
        pager.append(&next_btn);
        root.append(&pager);

        let page = Rc::new(RefCell::new(Self {
            root,
            list: list.clone(),
            search: search.clone(),
            page_label,
            total_label,
            prev_btn: prev_btn.clone(),
            next_btn: next_btn.clone(),
            page: 0,
            page_size,
            mode,
            query: String::new(),
            filtered: Vec::new(),
            snapshot: Vec::new(),
            row_map: HashMap::new(),
        }));

        // ---- 信号 ----
        let p = page.clone();
        search_btn.connect_clicked(move |_| {
            let mut p = p.borrow_mut();
            p.query = p.search.text().to_string();
            p.page = 0;
            p.apply_filter();
            p.render();
        });

        let p = page.clone();
        search.connect_activate(move |_| {
            let mut p = p.borrow_mut();
            p.query = p.search.text().to_string();
            p.page = 0;
            p.apply_filter();
            p.render();
        });

        let p = page.clone();
        prev_btn.connect_clicked(move |_| {
            let mut p = p.borrow_mut();
            if p.page > 0 {
                p.page -= 1;
                p.render();
            }
        });

        let p = page.clone();
        next_btn.connect_clicked(move |_| {
            let mut p = p.borrow_mut();
            let pages = p.total_pages();
            if p.page + 1 < pages {
                p.page += 1;
                p.render();
            }
        });

        let p = page.clone();
        list.connect_row_activated(move |_, row| {
            let p = p.borrow();
            if let Some(w) = row.child() {
                if let Some(&si) = p.row_map.get(&w) {
                    if si < p.snapshot.len() {
                        let name = p.snapshot[si].name.clone();
                        drop(p);
                        on_open_detail(&name);
                    }
                }
            }
        });

        page
    }

    pub fn set_snapshot(&mut self, snapshot: Vec<PkgInfo>) {
        self.snapshot = snapshot;
        self.page = 0;
        self.apply_filter();
        self.render();
    }

    fn total_pages(&self) -> usize {
        if self.filtered.is_empty() {
            1
        } else {
            (self.filtered.len() + self.page_size - 1) / self.page_size
        }
    }

    /// 根据模式+搜索词过滤
    fn apply_filter(&mut self) {
        let q = self.query.trim().to_lowercase();
        self.filtered.clear();
        for (i, p) in self.snapshot.iter().enumerate() {
            match self.mode {
                Mode::All => {}
                Mode::Installed => {
                    if !p.installed {
                        continue;
                    }
                }
                Mode::Upgradable => {
                    if !p.upgradable {
                        continue;
                    }
                }
            }
            if !q.is_empty() && !p.name.to_lowercase().contains(&q) {
                continue;
            }
            self.filtered.push(i);
        }
        let pages = self.total_pages();
        if self.page >= pages {
            self.page = pages.saturating_sub(1);
        }
    }

    fn render(&mut self) {
        // GTK < 4.18 无 remove_all,循环移除
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.row_map.clear();
        let pages = self.total_pages();
        let start = self.page * self.page_size;
        let end = (start + self.page_size).min(self.filtered.len());

        for i in start..end {
            let si = self.filtered[i];
            if si >= self.snapshot.len() {
                continue;
            }
            let p = &self.snapshot[si];
            let row = build_row(self.mode, p);
            if let Some(w) = row.child() {
                self.row_map.insert(w, si);
            }
            self.list.append(&row);
        }

        // 空状态
        if self.filtered.is_empty() {
            let empty = Label::new(Some(i18n::t("empty")));
            empty.add_css_class("loading-label");
            empty.set_margin_top(40);
            self.list.set_placeholder(Some(&empty));
        }

        let page_text = i18n::t("page")
            .replace("{page}", &(self.page + 1).to_string())
            .replace("{total}", &pages.to_string());
        self.page_label.set_text(&page_text);
        let total_text = i18n::t("total_pkgs").replace("{n}", &self.filtered.len().to_string());
        self.total_label.set_text(&total_text);
        self.prev_btn.set_sensitive(self.page > 0);
        self.next_btn.set_sensitive(self.page + 1 < pages);
    }
}

/// 构建一行:名称 + 摘要 + 版本 + 状态徽章
fn build_row(mode: Mode, p: &PkgInfo) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_activatable(true);
    row.set_selectable(true);

    let hbox = Box::new(Orientation::Horizontal, 10);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);

    // 左侧:名称 + 摘要
    let vbox = Box::new(Orientation::Vertical, 2);
    vbox.set_hexpand(true);
    let name = Label::new(Some(&p.name));
    name.add_css_class("pkg-name");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    vbox.append(&name);

    let summary_text = if p.summary.is_empty() { " " } else { &p.summary };
    let summary = Label::new(Some(summary_text));
    summary.add_css_class("pkg-summary");
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.set_max_width_chars(70);
    vbox.append(&summary);

    hbox.append(&vbox);

    // 版本
    let ver = Label::new(Some(&p.version));
    ver.add_css_class("pkg-version");
    ver.set_xalign(0.0);
    hbox.append(&ver);

    // 状态徽章
    let badge = match (p.installed, p.upgradable) {
        (true, true) => ("badge-both", i18n::t("installed_upgradable")),
        (true, false) => ("badge-installed", i18n::t("installed_badge")),
        (false, true) => ("badge-upgradable", i18n::t("upgradable_badge")),
        (false, false) => ("badge-not-installed", i18n::t("not_installed")),
    };
    let badge_label = Label::new(Some(badge.1));
    badge_label.add_css_class("badge");
    badge_label.add_css_class(badge.0);
    hbox.append(&badge_label);

    // 行尾提示箭头(仅列表页)
    if mode == Mode::All || mode == Mode::Installed || mode == Mode::Upgradable {
        // 保持简洁,不加箭头
    }

    row.set_child(Some(&hbox));
    row
}
