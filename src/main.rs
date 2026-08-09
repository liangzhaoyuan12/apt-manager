mod apt_worker;
mod config;
mod detail_page;
mod dialogs;
mod i18n;
mod list_page;
mod theme;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, ComboBoxText, HeaderBar, Stack,
    StackTransitionType, ToggleButton,
};

use apt_worker::AptWorker;
use config::{Config, LangPref, ThemePref};
use i18n::tr;
use list_page::{Mode, PkgListPage};

/// 统一 UI 事件(页面 -> 主循环)
enum UiEvent {
    OpenDetail(String),
    Back,
    OpInstall(String),
    OpUpdate(String),
    OpRemove(String),
    ActionUpdateSources,
    ActionAutoRemove,
    ActionUpgradeAll,
    ActionCheckUpdates,
}

/// 共享应用状态
struct AppState {
    worker: Arc<AptWorker>,
    config: Config,
    page_all: Rc<RefCell<PkgListPage>>,
    page_installed: Rc<RefCell<PkgListPage>>,
    page_upgradable: Rc<RefCell<PkgListPage>>,
    detail: Rc<RefCell<detail_page::DetailPage>>,
    stack: Stack,
    current_tab: String,
    window: ApplicationWindow,
}

fn main() {
    let app = Application::builder()
        .application_id("com.liangzhaoyuan12.aptmanager")
        .build();

    let worker = Arc::new(AptWorker::start());

    app.connect_activate(move |app| {
        theme::load_css();
        let config = Config::load();
        i18n::set_lang(config.language);
        build_window(app, worker.clone(), config);
    });

    app.run();
}

/// 构建整个主窗口(语言切换时重建)
fn build_window(app: &Application, worker: Arc<AptWorker>, config: Config) {
    // 事件通道(glib 0.22 无 MainContext::channel,用 std mpsc + idle 轮询)
    let (ev_tx, ev_rx) = std::sync::mpsc::channel::<UiEvent>();

    let window = ApplicationWindow::builder()
        .application(app)
        .title(tr!("app_name"))
        .default_width(1024)
        .default_height(700)
        .build();

    // ---- HeaderBar ----
    let header = HeaderBar::new();
    header.set_show_title_buttons(true);
    window.set_titlebar(Some(&header));

    // ---- 内容 Stack ----
    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::Crossfade);
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    window.set_child(Some(&stack));

    // ---- 创建三个列表页 ----
    let ev = ev_tx.clone();
    let page_all = PkgListPage::new(
        Mode::All,
        move |name| {
            let _ = ev.send(UiEvent::OpenDetail(name.to_string()));
        },
        {
            let ev = ev_tx.clone();
            Some(std::boxed::Box::new(move || {
                let _ = ev.send(UiEvent::ActionUpdateSources);
            }))
        },
        None,
    );

    let ev = ev_tx.clone();
    let page_installed = PkgListPage::new(
        Mode::Installed,
        move |name| {
            let _ = ev.send(UiEvent::OpenDetail(name.to_string()));
        },
        {
            let ev = ev_tx.clone();
            Some(std::boxed::Box::new(move || {
                let _ = ev.send(UiEvent::ActionAutoRemove);
            }))
        },
        None,
    );

    let ev = ev_tx.clone();
    let page_upgradable = PkgListPage::new(
        Mode::Upgradable,
        move |name| {
            let _ = ev.send(UiEvent::OpenDetail(name.to_string()));
        },
        {
            let ev = ev_tx.clone();
            Some(std::boxed::Box::new(move || {
                let _ = ev.send(UiEvent::ActionUpgradeAll);
            }))
        },
        {
            let ev = ev_tx.clone();
            Some(std::boxed::Box::new(move || {
                let _ = ev.send(UiEvent::ActionCheckUpdates);
            }))
        },
    );

    // ---- 详情页 ----
    let ev = ev_tx.clone();
    let detail = detail_page::DetailPage::new(
        move || {
            let _ = ev.send(UiEvent::Back);
        },
        {
            let ev = ev_tx.clone();
            move |name| {
                let _ = ev.send(UiEvent::OpInstall(name.to_string()));
            }
        },
        {
            let ev = ev_tx.clone();
            move |name| {
                let _ = ev.send(UiEvent::OpUpdate(name.to_string()));
            }
        },
        {
            let ev = ev_tx.clone();
            move |name| {
                let _ = ev.send(UiEvent::OpRemove(name.to_string()));
            }
        },
    );

    // ---- 加入 Stack ----
    stack.add_named(&page_all.borrow().root, Some("all"));
    stack.add_named(&page_installed.borrow().root, Some("installed"));
    stack.add_named(&page_upgradable.borrow().root, Some("upgradable"));
    stack.add_named(&detail.borrow().root, Some("detail"));

    // 默认进入已安装页
    stack.set_visible_child_name("installed");

    // 应用主题(窗口类)
    theme::apply_theme(config.theme, &window);

    // ---- 导航切换按钮(header 居中)----
    let nav = Box::new(gtk::Orientation::Horizontal, 2);
    nav.add_css_class("nav-switcher");
    header.set_title_widget(Some(&nav));

    let tabs: [(&str, &str); 3] = [
        ("all", tr!("tab_all")),
        ("installed", tr!("tab_installed")),
        ("upgradable", tr!("tab_upgradable")),
    ];
    let mut nav_btns: Vec<ToggleButton> = Vec::new();
    for (name, label) in tabs {
        let btn = ToggleButton::with_label(label);
        btn.set_widget_name(name);
        btn.set_has_frame(false);
        nav.append(&btn);
        nav_btns.push(btn);
    }
    nav_btns[1].set_active(true);

    // ---- 右侧:主题 / 语言 / 关于 ----
    let theme_dd = ComboBoxText::new();
    for name in [
        i18n::theme_self_name(ThemePref::System),
        i18n::theme_self_name(ThemePref::Light),
        i18n::theme_self_name(ThemePref::Dark),
    ] {
        theme_dd.append_text(&name);
    }
    theme_dd.set_active(Some(match config.theme {
        ThemePref::System => 0,
        ThemePref::Light => 1,
        ThemePref::Dark => 2,
    }));
    theme_dd.set_tooltip_text(Some(tr!("theme")));

    let lang_dd = ComboBoxText::new();
    for name in [
        i18n::lang_self_name(LangPref::System),
        i18n::lang_self_name(LangPref::Zh),
        i18n::lang_self_name(LangPref::En),
    ] {
        lang_dd.append_text(&name);
    }
    lang_dd.set_active(Some(match config.language {
        LangPref::System => 0,
        LangPref::Zh => 1,
        LangPref::En => 2,
    }));
    lang_dd.set_tooltip_text(Some(tr!("language")));

    let about_btn = Button::new();
    about_btn.set_icon_name("help-about-symbolic");
    about_btn.set_tooltip_text(Some(tr!("about")));

    // 主题选择:标签 + 下拉
    let theme_label = gtk::Label::new(Some(tr!("theme")));
    theme_label.add_css_class("header-label");
    let theme_box = Box::new(gtk::Orientation::Horizontal, 4);
    theme_box.append(&theme_label);
    theme_box.append(&theme_dd);

    // 语言选择:标签 + 下拉
    let lang_label = gtk::Label::new(Some(tr!("language")));
    lang_label.add_css_class("header-label");
    let lang_box = Box::new(gtk::Orientation::Horizontal, 4);
    lang_box.append(&lang_label);
    lang_box.append(&lang_dd);

    header.pack_end(&about_btn);
    header.pack_end(&lang_box);
    header.pack_end(&theme_box);

    // ---- 组装 state ----
    let state = Rc::new(RefCell::new(AppState {
        worker,
        config,
        page_all,
        page_installed,
        page_upgradable,
        detail,
        stack: stack.clone(),
        current_tab: "installed".to_string(),
        window: window.clone(),
    }));

    // ---- 导航切换信号 ----
    {
        let st = state.clone();
        for (i, btn) in nav_btns.iter().enumerate() {
            let btn = btn.clone();
            let others: Vec<ToggleButton> = nav_btns
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| b.clone())
                .collect();
            let stack = stack.clone();
            let st = st.clone();
            btn.connect_clicked(move |b| {
                if b.is_active() {
                    for o in &others {
                        o.set_active(false);
                    }
                    let name = b.widget_name().to_string();
                    stack.set_visible_child_name(&name);
                    st.borrow_mut().current_tab = name.clone();
                } else {
                    b.set_active(true);
                }
            });
        }
    }

    // ---- 事件处理(主循环 idle 轮询)----
    let st = state.clone();
    glib::idle_add_local(move || {
        while let Ok(ev) = ev_rx.try_recv() {
            let st = st.clone();
            handle_event(&st, ev);
        }
        glib::ControlFlow::Continue
    });

    // ---- 关于按钮 ----
    let st = state.clone();
    about_btn.connect_clicked(move |_| {
        let s = st.borrow();
        let win = s.window.clone();
        dialogs::show_about(&win);
    });

    // ---- 主题切换 ----
    let st = state.clone();
    theme_dd.connect_active_notify(move |dd| {
        let pref = match dd.active().unwrap_or(0) {
            1 => ThemePref::Light,
            2 => ThemePref::Dark,
            _ => ThemePref::System,
        };
        let win = st.borrow().window.clone();
        theme::apply_theme(pref, &win);
        let mut s = st.borrow_mut();
        s.config.theme = pref;
        s.config.save();
    });

    // ---- 语言切换:保存配置 + 重建界面 ----
    let app = app.clone();
    let worker = state.borrow().worker.clone();
    let state2 = state.clone();
    lang_dd.connect_active_notify(move |dd| {
        let pref = match dd.active().unwrap_or(0) {
            1 => LangPref::Zh,
            2 => LangPref::En,
            _ => LangPref::System,
        };
        let theme = state2.borrow().config.theme;
        let cfg = Config {
            theme,
            language: pref,
        };
        cfg.save();
        i18n::set_lang(pref);
        // 关闭旧窗口并重建
        let win: Option<gtk::Window> = {
            let s = state2.borrow();
            Some(s.window.clone().upcast::<gtk::Window>())
        };
        if let Some(win) = win {
            win.close();
        }
        build_window(&app, worker.clone(), cfg);
    });

    // ---- 启动时加载快照 ----
    let st = state.clone();
    let worker = st.borrow().worker.clone();
    worker.request_snapshot(move |snap| {
        let s = st.borrow();
        s.page_all.borrow_mut().set_snapshot(snap.clone());
        s.page_installed.borrow_mut().set_snapshot(snap.clone());
        s.page_upgradable.borrow_mut().set_snapshot(snap);
    });

    window.present();
}

/// 事件分发
fn handle_event(state: &Rc<RefCell<AppState>>, ev: UiEvent) {
    match ev {
        UiEvent::OpenDetail(name) => {
            let s = state.borrow();
            let worker = s.worker.clone();
            let detail = s.detail.clone();
            let stack = s.stack.clone();
            worker.request_detail(name, move |d| {
                if let Some(d) = d {
                    detail.borrow_mut().set_detail(d);
                    stack.set_visible_child_name("detail");
                }
            });
        }
        UiEvent::Back => {
            let s = state.borrow();
            let stack = s.stack.clone();
            let tab = s.current_tab.clone();
            stack.set_visible_child_name(&tab);
        }
        UiEvent::OpInstall(name) => {
            let s = state.borrow();
            let win = s.window.clone();
            let msg = i18n::t("confirm_install").replace("{pkg}", &name);
            let st = state.clone();
            dialogs::confirm(&win, &msg, i18n::t("requires_root"), move || {
                let st2 = st.clone();
                let s = st.borrow();
                let win = s.window.clone();
                let title = format!("{}: {name}", i18n::t("install"));
                dialogs::run_op_dialog(
                    &win,
                    &title,
                    &["install", "-y", "--no-install-recommends", &name],
                    move || refresh_all(&st2),
                );
            });
        }
        UiEvent::OpUpdate(name) => {
            let s = state.borrow();
            let win = s.window.clone();
            let msg = i18n::t("confirm_update").replace("{pkg}", &name);
            let st = state.clone();
            dialogs::confirm(&win, &msg, i18n::t("requires_root"), move || {
                let st2 = st.clone();
                let s = st.borrow();
                let win = s.window.clone();
                let title = format!("{}: {name}", i18n::t("update"));
                dialogs::run_op_dialog(
                    &win,
                    &title,
                    &["install", "--only-upgrade", "-y", &name],
                    move || refresh_all(&st2),
                );
            });
        }
        UiEvent::OpRemove(name) => {
            let s = state.borrow();
            let win = s.window.clone();
            let msg = i18n::t("confirm_uninstall").replace("{pkg}", &name);
            let st = state.clone();
            dialogs::confirm(&win, &msg, i18n::t("requires_root"), move || {
                let st2 = st.clone();
                let s = st.borrow();
                let win = s.window.clone();
                let title = format!("{}: {name}", i18n::t("uninstall"));
                dialogs::run_op_dialog(&win, &title, &["remove", "-y", &name], move || {
                    refresh_all(&st2);
                });
            });
        }
        UiEvent::ActionUpdateSources => {
            let s = state.borrow();
            let win = s.window.clone();
            let st = state.clone();
            dialogs::confirm(
                &win,
                i18n::t("confirm_update_sources"),
                i18n::t("requires_root"),
                move || {
                    let st2 = st.clone();
                    let s = st.borrow();
                    let win = s.window.clone();
                    dialogs::run_op_dialog(&win, tr!("update_sources"), &["update"], move || {
                        refresh_all(&st2);
                    });
                },
            );
        }
        UiEvent::ActionAutoRemove => {
            let s = state.borrow();
            let win = s.window.clone();
            let st = state.clone();
            dialogs::confirm(
                &win,
                i18n::t("confirm_autoremove"),
                i18n::t("requires_root"),
                move || {
                    let st2 = st.clone();
                    let s = st.borrow();
                    let win = s.window.clone();
                    dialogs::run_op_dialog(&win, tr!("auto_remove"), &["autoremove", "-y"], move || {
                        refresh_all(&st2);
                    });
                },
            );
        }
        UiEvent::ActionUpgradeAll => {
            let s = state.borrow();
            let win = s.window.clone();
            let st = state.clone();
            dialogs::confirm(
                &win,
                i18n::t("confirm_upgrade_all"),
                i18n::t("requires_root"),
                move || {
                    let st2 = st.clone();
                    let s = st.borrow();
                    let win = s.window.clone();
                    dialogs::run_op_dialog(&win, tr!("upgrade_all"), &["upgrade", "-y"], move || {
                        refresh_all(&st2);
                    });
                },
            );
        }
        UiEvent::ActionCheckUpdates => {
            let s = state.borrow();
            let win = s.window.clone();
            let st = state.clone();
            dialogs::confirm(
                &win,
                i18n::t("confirm_check_updates"),
                i18n::t("requires_root"),
                move || {
                    let st2 = st.clone();
                    let s = st.borrow();
                    let win = s.window.clone();
                    dialogs::run_op_dialog(&win, tr!("check_updates"), &["update"], move || {
                        refresh_all(&st2);
                    });
                },
            );
        }
    }
}

/// 操作完成后刷新三个页面并回到当前 tab
fn refresh_all(state: &Rc<RefCell<AppState>>) {
    let st = state.clone();
    let worker = st.borrow().worker.clone();
    worker.request_snapshot(move |snap| {
        let s = st.borrow();
        s.page_all.borrow_mut().set_snapshot(snap.clone());
        s.page_installed.borrow_mut().set_snapshot(snap.clone());
        s.page_upgradable.borrow_mut().set_snapshot(snap);
        let stack = s.stack.clone();
        let tab = s.current_tab.clone();
        stack.set_visible_child_name(&tab);
    });
}

// 保留 Align 引用,避免未使用告警(分页对齐已在 list_page 处理)
#[allow(unused_imports)]
use gtk::Align as _Align;
