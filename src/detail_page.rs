use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box, Button, Grid, Label, Orientation, ScrolledWindow, Separator, Widget};

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
    // 信息 Grid 两列各值
    app_name_value: Label,
    pkg_name_value: Label,
    version_info_value: Label,
    arch_value: Label,
    maintainer_value: Label,
    date_value: Label,
    installed_size_value: Label,
    package_size_value: Label,
    section_value: Label,
    homepage_value: Label,
    url_value: Label,
    desktop_value: Label,
    exec_cmd_value: Label,
    exec_path_value: Label,
    deps_value: Label,
    install_btn: Button,
    update_btn: Button,
    remove_btn: Button,
    detail: Option<PkgDetail>,
}

const BADGE_CLASSES: [&str; 4] = [
    "badge-installed",
    "badge-upgradable",
    "badge-both",
    "badge-not-installed",
];

impl DetailPage {
    pub fn new(
        on_back: impl Fn() + 'static,
        on_install: impl Fn(&str) + 'static,
        on_update: impl Fn(&str) + 'static,
        on_remove: impl Fn(&str) + 'static,
    ) -> Rc<RefCell<Self>> {
        let root = Box::new(Orientation::Vertical, 0);

        // ---- 顶栏:返回按钮(固定不滚动)----
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

        // ---- 内容区:整体可滚动(依赖/描述可能很长)----
        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);

        let content = Box::new(Orientation::Vertical, 0);
        content.set_margin_top(4);
        content.set_margin_bottom(12);
        content.set_margin_start(16);
        content.set_margin_end(16);
        scroll.set_child(Some(&content));

        let card = Box::new(Orientation::Vertical, 12);
        card.add_css_class("detail-card");
        content.append(&card);

        // 标题行:应用名 + 徽章
        let title_row = Box::new(Orientation::Horizontal, 10);
        title_row.set_halign(Align::Start);
        let title = Label::new(None);
        title.add_css_class("detail-title");
        let badge = Label::new(None);
        badge.add_css_class("badge");
        title_row.append(&title);
        title_row.append(&badge);
        card.append(&title_row);

        // 版本(hero 副标题)
        let version_value = Label::new(None);
        version_value.add_css_class("detail-version");
        version_value.set_xalign(0.0);
        version_value.set_selectable(true);
        card.append(&version_value);

        // ---- 信息两列 Grid ----
        let grid = Grid::new();
        grid.set_column_spacing(18);
        grid.set_row_spacing(6);
        grid.set_margin_top(6);
        grid.set_hexpand(true);

        let app_name_value = grid_cell(&grid, 0, 0, 1, "detail_app_name");
        let pkg_name_value = grid_cell(&grid, 1, 0, 1, "detail_package_name");
        let version_info_value = grid_cell(&grid, 0, 1, 1, "detail_version");
        let arch_value = grid_cell(&grid, 1, 1, 1, "detail_arch");
        let maintainer_value = grid_cell(&grid, 0, 2, 1, "detail_maintainer");
        let date_value = grid_cell(&grid, 1, 2, 1, "detail_date");
        let installed_size_value = grid_cell(&grid, 0, 3, 1, "detail_installed_size");
        let package_size_value = grid_cell(&grid, 1, 3, 1, "detail_package_size");
        let section_value = grid_cell(&grid, 0, 4, 1, "detail_section");
        let homepage_value = grid_cell(&grid, 1, 4, 1, "detail_homepage");
        let url_value = grid_cell(&grid, 0, 5, 2, "detail_url");
        let desktop_value = grid_cell(&grid, 0, 6, 2, "detail_desktop");
        let exec_cmd_value = grid_cell(&grid, 0, 7, 1, "detail_exec_cmd");
        let exec_path_value = grid_cell(&grid, 1, 7, 1, "detail_exec_path");
        let deps_value = grid_cell(&grid, 0, 8, 2, "detail_deps");
        card.append(&grid);

        // 简介(自动换行)
        let summary_value = detail_row(&card, "detail_summary");
        summary_value.set_wrap(true);

        // 分区线
        card.append(&Separator::new(Orientation::Horizontal));

        // 详细描述
        let desc_label = Label::new(Some(i18n::t("detail_description")));
        desc_label.add_css_class("detail-label");
        desc_label.set_xalign(0.0);
        desc_label.set_margin_top(4);
        card.append(&desc_label);

        let desc_value = Label::new(None);
        desc_value.add_css_class("detail-desc");
        desc_value.set_wrap(true);
        desc_value.set_xalign(0.0);
        desc_value.set_selectable(true);
        desc_value.set_hexpand(true);
        card.append(&desc_value);

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

        root.append(&scroll);

        let page = Rc::new(RefCell::new(Self {
            root,
            title,
            badge,
            version_value,
            summary_value,
            desc_value,
            app_name_value,
            pkg_name_value,
            version_info_value,
            arch_value,
            maintainer_value,
            date_value,
            installed_size_value,
            package_size_value,
            section_value,
            homepage_value,
            url_value,
            desktop_value,
            exec_cmd_value,
            exec_path_value,
            deps_value,
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
        self.title.set_text(&d.display_name);
        self.version_value
            .set_text(&format_versions(&d.installed_version, &d.candidate_version));
        self.app_name_value.set_text(&d.display_name);
        self.pkg_name_value.set_text(&d.name);
        self.version_info_value
            .set_text(if d.candidate_version.is_empty() { &d.installed_version } else { &d.candidate_version });
        self.arch_value.set_text(or_dash(&d.architecture));
        self.maintainer_value.set_text(or_dash(&d.maintainer));
        self.date_value.set_text(or_dash(&d.date));
        self.installed_size_value.set_text(&format_size(d.installed_size));
        self.package_size_value.set_text(&format_size(d.package_size));
        self.section_value.set_text(or_dash(&d.section));
        self.homepage_value.set_text(or_dash(&d.homepage));
        self.url_value.set_text(or_dash(&d.download_url));
        self.desktop_value.set_text(or_dash(&d.desktop_file));
        self.exec_cmd_value.set_text(or_dash(&d.exec_command));
        self.exec_path_value.set_text(or_dash(&d.exec_path));
        self.deps_value.set_text(or_dash(&d.depends));
        self.summary_value.set_text(or_dash(&d.summary));
        self.desc_value.set_text(if d.description.is_empty() {
            i18n::t("detail_no_desc")
        } else {
            &d.description
        });

        // 徽章(先清掉旧类,避免换包后类叠加)
        for c in BADGE_CLASSES {
            self.badge.remove_css_class(c);
        }
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

fn or_dash(s: &str) -> &str {
    if s.is_empty() { "—" } else { s }
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

/// 创建一行 "标签: 值"(行占满容器宽度,便于两列并排平分)并返回值 Label
fn detail_row(container: &Box, key: &str) -> Label {
    let row = Box::new(Orientation::Horizontal, 10);
    // 注意:不能设 halign Start —— 非 Fill 时 Box 只分配自然宽,hexpand 失效
    row.set_hexpand(true);

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
    container.append(&row);
    value
}

/// Grid 单元格:label + 可换行、可选择的 value。span 为跨列数(整行传 2)。
fn grid_cell(grid: &Grid, col: i32, row: i32, span: i32, key: &str) -> Label {
    let row_box = Box::new(Orientation::Horizontal, 10);
    row_box.set_hexpand(true);

    let label = Label::new(Some(i18n::t(key)));
    label.add_css_class("detail-label");
    label.set_xalign(0.0);
    label.set_width_chars(15);

    let value = Label::new(None);
    value.add_css_class("detail-value");
    value.set_xalign(0.0);
    value.set_selectable(true);
    value.set_wrap(true);

    row_box.append(&label);
    row_box.append(&value);
    grid.attach(&row_box, col, row, span, 1);
    value
}

impl DetailPage {
    #[allow(dead_code)]
    fn _keep(&self, _w: &Widget) {}
}
