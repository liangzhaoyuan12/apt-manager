use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{
    Align, Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SearchEntry,
    Widget,
};

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

/// 批量操作类型(多选模式)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchAction {
    Install,
    Remove,
    Update,
}

/// 通用软件列表页:搜索 + 分页(每页 50)+ 多选批量操作
pub struct PkgListPage {
    pub root: gtk::Box,
    list: ListBox,
    search: SearchEntry,
    page_label: Label,
    total_label: Label,
    prev_btn: Button,
    next_btn: Button,
    multi_btn: Button,
    batch_count: Label,
    select_all_btn: Button,
    batch_btn: Button,
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
    /// 多选回调(批量操作)
    on_batch: Option<std::boxed::Box<dyn Fn(BatchAction, Vec<String>)>>,
    /// 多选模式
    multi_select: bool,
    /// 选中项(快照索引)
    selected: HashSet<usize>,
}

impl PkgListPage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: Mode,
        on_open_detail: impl Fn(&str) + 'static,
        on_action: Option<std::boxed::Box<dyn Fn()>>,
        on_check: Option<std::boxed::Box<dyn Fn()>>,
        on_refresh: Option<std::boxed::Box<dyn Fn()>>,
        on_batch: Option<std::boxed::Box<dyn Fn(BatchAction, Vec<String>)>>,
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

        // 多选入口(所有 tab 都有)
        let multi_btn = Button::with_label(i18n::t("multi_select"));
        multi_btn.add_css_class("ghost-btn");

        // 刷新按钮:仅在传入 on_refresh 回调时显示(目前用于已安装页)
        let refresh_btn = Button::new();
        match on_refresh {
            Some(cb) => {
                refresh_btn.set_label(i18n::t("refresh"));
                refresh_btn.add_css_class("ghost-btn");
                refresh_btn.connect_clicked(move |_| cb());
            }
            None => {
                refresh_btn.set_visible(false);
            }
        }
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
        top.append(&refresh_btn);
        top.append(&check_btn);
        top.append(&multi_btn);
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

        // ---- 底部:统一操作栏(共N个/已选N项 左,全选/批量/分页 右)----
        let bottom = Box::new(Orientation::Horizontal, 8);
        bottom.add_css_class("pager");
        bottom.set_margin_top(6);
        bottom.set_margin_bottom(10);
        bottom.set_margin_start(12);
        bottom.set_margin_end(12);

        // 左侧:总数 + 多选计数(多选时可见)
        let total_label = Label::new(None);
        total_label.set_halign(Align::Start);

        let batch_count = Label::new(None);
        batch_count.set_halign(Align::Start);
        batch_count.set_visible(false);

        bottom.append(&total_label);
        bottom.append(&batch_count);

        // 弹性空间:左侧内容靠左,右侧控件靠右
        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        bottom.append(&spacer);

        // 右侧:全选 / 批量(多选时可见),分页组永远可见(最右)
        let select_all_btn = Button::with_label(i18n::t("multi_select_all"));
        select_all_btn.add_css_class("ghost-btn");
        select_all_btn.set_visible(false);

        let batch_label = match mode {
            Mode::All => i18n::t("batch_install"),
            Mode::Installed => i18n::t("batch_remove"),
            Mode::Upgradable => i18n::t("batch_update"),
        };
        let batch_btn = Button::with_label(batch_label);
        batch_btn.add_css_class("suggested-action");
        batch_btn.set_sensitive(false);
        batch_btn.set_visible(false);

        let prev_btn = Button::with_label(i18n::t("prev"));
        let page_label = Label::new(None);
        let next_btn = Button::with_label(i18n::t("next"));

        bottom.append(&select_all_btn);
        bottom.append(&batch_btn);
        bottom.append(&prev_btn);
        bottom.append(&page_label);
        bottom.append(&next_btn);
        root.append(&bottom);

        let page = Rc::new(RefCell::new(Self {
            root,
            list: list.clone(),
            search: search.clone(),
            page_label,
            total_label,
            prev_btn: prev_btn.clone(),
            next_btn: next_btn.clone(),
            multi_btn: multi_btn.clone(),
            batch_count,
            select_all_btn: select_all_btn.clone(),
            batch_btn: batch_btn.clone(),
            page: 0,
            page_size,
            mode,
            query: String::new(),
            filtered: Vec::new(),
            snapshot: Vec::new(),
            row_map: HashMap::new(),
            on_batch,
            multi_select: false,
            selected: HashSet::new(),
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
            // 多选模式下点击不打开详情
            if p.multi_select {
                return;
            }
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

        // 多选模式:选中行变化时同步 selected 集合(跨页保留:只更新当前页,其他页选中不动)
        // 注意:set_selection_mode / select_row 会同步触发本信号,而调用方(set_multi/
        // render/全选)可能已持有可变借用,必须 try_borrow_mut 避免双重借用 panic
        let p = page.clone();
        list.connect_selected_rows_changed(move |list| {
            let Ok(mut p) = p.try_borrow_mut() else {
                return;
            };
            if !p.multi_select {
                return;
            }
            // 当前页可见索引
            let page_visible: HashSet<usize> = p.row_map.values().copied().collect();
            let mut next: HashSet<usize> = HashSet::new();
            for row in list.selected_rows() {
                if let Some(w) = row.child() {
                    if let Some(&si) = p.row_map.get(&w) {
                        next.insert(si);
                    }
                }
            }
            // 保留其他页的选中(翻页/搜索后回到旧页仍在)
            for &i in &p.selected {
                if !page_visible.contains(&i) {
                    next.insert(i);
                }
            }
            p.selected = next;
            p.update_batch_ui();
        });

        // 多选按钮:切换多选模式
        let p = page.clone();
        multi_btn.connect_clicked(move |_| {
            let mut p = p.borrow_mut();
            let on = !p.multi_select;
            p.set_multi(on);
        });

        // 全选 / 清空(直接维护 selected,不依赖信号,天然跨页)
        let p = page.clone();
        select_all_btn.connect_clicked(move |_| {
            let mut p = p.borrow_mut();
            if !p.multi_select {
                return;
            }
            if p.selected.is_empty() {
                // 全选:当前页所有可操作行(其他页已有选中则保留)
                let mut child = p.list.first_child();
                while let Some(w) = child {
                    if let Ok(row) = w.clone().downcast::<gtk::ListBoxRow>() {
                        if let Some(cw) = row.child() {
                            if let Some(&si) = p.row_map.get(&cw) {
                                if p.actionable(si) {
                                    p.selected.insert(si);
                                    p.list.select_row(Some(&row));
                                }
                            }
                        }
                    }
                    child = w.next_sibling();
                }
            } else {
                p.selected.clear();
                p.list.unselect_all();
            }
            p.update_batch_ui();
        });

        // 批量操作
        let p = page.clone();
        batch_btn.connect_clicked(move |_| {
            let (action, names) = {
                let mut p = p.borrow_mut();
                let names = p.collect_selected_names();
                if names.is_empty() {
                    return;
                }
                let action = match p.mode {
                    Mode::All => BatchAction::Install,
                    Mode::Installed => BatchAction::Remove,
                    Mode::Upgradable => BatchAction::Update,
                };
                p.set_multi(false);
                (action, names)
            };
            if let Some(cb) = &p.borrow().on_batch {
                cb(action, names);
            }
        });

        page
    }

    pub fn set_snapshot(&mut self, snapshot: Vec<PkgInfo>) {
        self.snapshot = snapshot;
        self.page = 0;
        if self.multi_select {
            self.set_multi(false);
        }
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

    /// 该包在当前 tab 是否可被批量操作(All 只装未安装,Installed 只卸已装,Upgradable 只更可更新)
    fn actionable(&self, si: usize) -> bool {
        let Some(p) = self.snapshot.get(si) else {
            return false;
        };
        match self.mode {
            Mode::All => !p.installed,
            Mode::Installed => p.installed,
            Mode::Upgradable => p.upgradable,
        }
    }

    /// 切换多选模式
    fn set_multi(&mut self, on: bool) {
        if self.multi_select == on {
            return;
        }
        self.multi_select = on;
        self.selected.clear();
        self.list.set_selection_mode(if on {
            gtk::SelectionMode::Multiple
        } else {
            gtk::SelectionMode::Single
        });
        // 只切换多选控件可见性;分页组(底部最右)始终可见
        self.batch_count.set_visible(on);
        self.select_all_btn.set_visible(on);
        self.batch_btn.set_visible(on);
        self.multi_btn
            .set_label(i18n::t(if on { "multi_exit" } else { "multi_select" }));
        self.update_batch_ui();
    }

    /// 刷新多选操作栏(计数/按钮可用性/全选按钮文案)
    fn update_batch_ui(&mut self) {
        let n = self.selected.len();
        self.batch_count
            .set_text(&i18n::t("multi_count").replace("{n}", &n.to_string()));
        let actionable = n > 0 && self.selected.iter().any(|&i| self.actionable(i));
        self.batch_btn.set_sensitive(actionable);
        self.select_all_btn.set_label(if n > 0 {
            i18n::t("multi_clear")
        } else {
            i18n::t("multi_select_all")
        });
    }

    /// 收集选中且可操作的包名(排序保证命令稳定)
    fn collect_selected_names(&self) -> Vec<String> {
        let mut idx: Vec<usize> = self.selected.iter().copied().collect();
        idx.sort_unstable();
        idx.iter()
            .filter(|&&i| self.actionable(i))
            .filter_map(|&i| self.snapshot.get(i).map(|p| p.name.clone()))
            .collect()
    }

    fn render(&mut self) {
        // GTK < 4.18 无 remove_all,循环移除
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.row_map.clear();
        // 注意:selected 不清空,跨页/跨搜索保留;set_snapshot(数据刷新)才退出多选
        self.list.set_selection_mode(if self.multi_select {
            gtk::SelectionMode::Multiple
        } else {
            gtk::SelectionMode::Single
        });
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

        // 恢复跨页选中的行视觉(select_row 触发的信号在借用冲突时会被跳过,无副作用)
        if self.multi_select && !self.selected.is_empty() {
            let mut child = self.list.first_child();
            while let Some(w) = child {
                if let Ok(row) = w.clone().downcast::<gtk::ListBoxRow>() {
                    if let Some(cw) = row.child() {
                        if let Some(&si) = self.row_map.get(&cw) {
                            if self.selected.contains(&si) {
                                self.list.select_row(Some(&row));
                            }
                        }
                    }
                }
                child = w.next_sibling();
            }
        }
        self.update_batch_ui();

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

    row.set_child(Some(&hbox));
    let _ = mode;
    row
}
