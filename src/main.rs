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

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::prelude::*;
use adw::prelude::*;
use gtk::{
    Box, HeaderBar, MenuButton, PopoverMenu, Stack,
    StackTransitionType, ToggleButton,
};

use apt_worker::AptWorker;
use config::{Config, LangPref, ThemePref};
use i18n::tr;
use list_page::{BatchAction, Mode, PkgListPage};

/// 统一 UI 事件(页面 -> 主循环)
enum UiEvent {
    OpenDetail(String),
    Back,
    OpInstall(String),
    OpUpdate(String),
    OpRemove(String),
    BatchOp(BatchAction, Vec<String>),
    ActionUpdateSources,
    ActionAutoRemove,
    ActionUpgradeAll,
    ActionCheckUpdates,
    ActionRefresh,
}

/// 全局 UI 事件通道(供 deb 安装完成等跨模块触发列表刷新)
static EVENT_TX: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::mpsc::Sender<UiEvent>>>> =
    std::sync::OnceLock::new();

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
    window: adw::ApplicationWindow,
}

fn main() {
    let app = adw::Application::builder()
        .application_id("com.liangzhaoyuan12.aptmanager")
        .flags(
            gio::ApplicationFlags::HANDLES_COMMAND_LINE | gio::ApplicationFlags::HANDLES_OPEN,
        )
        .build();

    let worker = Arc::new(AptWorker::start());
    let window_built = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // 公共:确保主窗口已构建。show=true 时首次构建即显示、已有窗口则重新显示;
    // show=false(纯 deb 打开)构建隐藏窗口:应用保持存活、对话框有 parent,但不进主页面。
    let ensure_window = |app: &adw::Application,
                         worker: &Arc<AptWorker>,
                         wb: &Arc<std::sync::atomic::AtomicBool>,
                         show: bool| {
        if !wb.load(std::sync::atomic::Ordering::Relaxed) {
            theme::load_css();
            let config = Config::load();
            i18n::set_lang(config.language);
            build_window(app, worker.clone(), config, show);
            wb.store(true, std::sync::atomic::Ordering::Relaxed);
        } else if show {
            if let Some(w) = dialogs::main_window() {
                w.present();
            }
        }
    };

    // open 信号:文件管理器经 D-Bus org.freedesktop.Application.Open 打开 deb。
    // 带 .deb 时只弹安装框,不进主页面;否则正常启动主窗口。
    {
        let worker = worker.clone();
        let wb = window_built.clone();
        app.connect_open(move |app, files, _hint| {
            let mut deb: Option<String> = None;
            for f in files {
                if let Some(p) = f.path() {
                    let p = p.to_string_lossy();
                    if p.ends_with(".deb") {
                        deb = Some(p.into_owned());
                        break;
                    }
                }
            }
            match deb {
                Some(path) => {
                    // 隐藏构建主窗口(应用存活 + 对话框有 parent),不进主页面
                    ensure_window(app, &worker, &wb, false);
                    dialogs::show_deb_dialog(&path, refresh_cb());
                }
                None => ensure_window(app, &worker, &wb, true),
            }
        });
    }

    // command-line 信号:命令行直接传 deb 文件
    {
        let worker = worker.clone();
        let wb = window_built.clone();
        app.connect_command_line(move |app, cmd| {
            // 命令行带 .deb 文件 → 只弹安装详情对话框,不进主页面
            // (文件管理器经 MimeType 打开时传 file:// URI,需转本地路径)
            let deb = cmd
                .arguments()
                .iter()
                .skip(1)
                .map(|a| a.to_string_lossy().into_owned())
                .find(|a| a.ends_with(".deb"));
            match deb {
                Some(uri) => {
                    let path = uri_to_local_path(&uri);
                    // 隐藏构建主窗口(应用存活 + 对话框有 parent),不进主页面
                    ensure_window(app, &worker, &wb, false);
                    dialogs::show_deb_dialog(&path, refresh_cb());
                }
                None => ensure_window(app, &worker, &wb, true),
            }
            // 回调返回值即退出码,GLib 自动完成 command-line
            glib::ExitCode::SUCCESS
        });
    }

    app.run();
}

/// deb 安装完成后的刷新回调(全局事件通道 → 主窗口列表刷新)
fn refresh_cb() -> impl Fn() + 'static {
    move || {
        if let Some(tx) = EVENT_TX.get() {
            if let Some(tx) = tx.lock().unwrap().as_ref() {
                let _ = tx.send(UiEvent::ActionRefresh);
            }
        }
    }
}

/// 多选批量操作回调:转发为 UiEvent::BatchOp
fn batch_cb(
    ev_tx: &std::sync::mpsc::Sender<UiEvent>,
) -> Option<std::boxed::Box<dyn Fn(BatchAction, Vec<String>)>> {
    let ev = ev_tx.clone();
    Some(std::boxed::Box::new(move |action, names| {
        let _ = ev.send(UiEvent::BatchOp(action, names));
    }))
}

/// file:// URI → 本地路径(文件管理器打开 deb 时传 URI,含百分号编码)
fn uri_to_local_path(s: &str) -> String {
    if s.starts_with("file://") {
        let f = gtk::gio::File::for_uri(s);
        if let Some(p) = f.path() {
            return p.to_string_lossy().into_owned();
        }
    }
    s.to_string()
}

/// 构建整个主窗口(语言切换时重建)
fn build_window(app: &adw::Application, worker: Arc<AptWorker>, config: Config, show: bool) {
    // 事件通道(glib 0.22 无 MainContext::channel,用 std mpsc + idle 轮询)
    let (ev_tx, ev_rx) = std::sync::mpsc::channel::<UiEvent>();
    // 注册全局通道(语言切换重建时覆盖,供 deb 安装完成等触发刷新)
    *EVENT_TX
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap() = Some(ev_tx.clone());

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(tr!("app_name"))
        .default_width(1024)
        .default_height(700)
        .build();
    dialogs::set_main_window(&window);

    // ---- ToolbarView 包装(HeaderBar + 内容) ----
    let toolbar = adw::ToolbarView::new();

    // ---- HeaderBar ----
    let header = HeaderBar::new();
    header.set_show_title_buttons(true);
    toolbar.add_top_bar(&header);

    // ---- 内容 Stack ----
    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::Crossfade);
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    toolbar.set_content(Some(&stack));

    window.set_content(Some(&toolbar));

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
        None,
        batch_cb(&ev_tx),
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
        {
            let ev = ev_tx.clone();
            Some(std::boxed::Box::new(move || {
                let _ = ev.send(UiEvent::ActionRefresh);
            }))
        },
        batch_cb(&ev_tx),
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
        None,
        batch_cb(&ev_tx),
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

    // ---- 右侧:主题 / 语言 / 关于 合并为单个菜单 ----
    let menu_root = gio::Menu::new();

    // 主题组(单选)
    let theme_items = gio::Menu::new();
    for (label, target) in [
        (i18n::theme_self_name(ThemePref::System), "system"),
        (i18n::theme_self_name(ThemePref::Light), "light"),
        (i18n::theme_self_name(ThemePref::Dark), "dark"),
    ] {
        let item = gio::MenuItem::new(Some(label), None);
        let target = glib::Variant::from(target);
        item.set_action_and_target_value(Some("win.theme"), Some(&target));
        theme_items.append_item(&item);
    }
    menu_root.append_section(Some(tr!("theme")), &theme_items);

    // 语言组(单选)
    let lang_items = gio::Menu::new();
    for (label, target) in [
        (i18n::lang_self_name(LangPref::System), "system"),
        (i18n::lang_self_name(LangPref::Zh), "zh"),
        (i18n::lang_self_name(LangPref::En), "en"),
    ] {
        let item = gio::MenuItem::new(Some(label), None);
        let target = glib::Variant::from(target);
        item.set_action_and_target_value(Some("win.lang"), Some(&target));
        lang_items.append_item(&item);
    }
    menu_root.append_section(Some(tr!("language")), &lang_items);

    // 关于
    menu_root.append(Some(tr!("about")), Some("win.about"));

    let popover = PopoverMenu::from_model(Some(&menu_root));
    let menu_btn = MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .popover(&popover)
        .build();
    menu_btn.set_tooltip_text(Some(tr!("menu")));
    header.pack_end(&menu_btn);

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

    // ---- 事件处理(主循环 timeout 轮询,避免 idle 持续 Continue 忙转吃满 CPU)----
    // 所有 ev_tx 发送方随窗口/页面销毁后,通道断开即停止本源,避免永久泄漏
    let st = state.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        loop {
            match ev_rx.try_recv() {
                Ok(ev) => {
                    let st = st.clone();
                    handle_event(&st, ev);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
        glib::ControlFlow::Continue
    });

    // ---- 菜单动作:关于 / 主题 / 语言 ----
    // 关于
    let about_action = gio::SimpleAction::new("about", None);
    {
        let st = state.clone();
        about_action.connect_activate(move |_, _| {
            let s = st.borrow();
            let win = s.window.clone();
            dialogs::show_about(&win);
        });
    }
    window.add_action(&about_action);

    // 主题(stateful action,state 即当前选中值,radio 菜单据此打勾)
    let theme_action = gio::SimpleAction::new_stateful(
        "theme",
        Some(&glib::VariantTy::STRING),
        &glib::Variant::from(match state.borrow().config.theme {
            ThemePref::System => "system",
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        }),
    );
    {
        let st = state.clone();
        theme_action.connect_change_state(move |a, v| {
            let Some(v) = v else { return };
            a.set_state(v);
            let pref = match v.str() {
                Some("light") => ThemePref::Light,
                Some("dark") => ThemePref::Dark,
                _ => ThemePref::System,
            };
            let win = st.borrow().window.clone();
            theme::apply_theme(pref, &win);
            let mut s = st.borrow_mut();
            s.config.theme = pref;
            s.config.save();
        });
    }
    window.add_action(&theme_action);

    // 语言:保存配置 + 重建界面
    let lang_action = gio::SimpleAction::new_stateful(
        "lang",
        Some(&glib::VariantTy::STRING),
        &glib::Variant::from(match state.borrow().config.language {
            LangPref::System => "system",
            LangPref::Zh => "zh",
            LangPref::En => "en",
        }),
    );
    {
        let app = app.clone();
        let state2 = state.clone();
        lang_action.connect_change_state(move |a, v| {
            let Some(v) = v else { return };
            a.set_state(v);
            let pref = match v.str() {
                Some("zh") => LangPref::Zh,
                Some("en") => LangPref::En,
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
            let worker = state2.borrow().worker.clone();
            build_window(&app, worker, cfg, true);
        });
    }
    window.add_action(&lang_action);

    // ---- 启动时加载快照 ----
    let st = state.clone();
    let worker = st.borrow().worker.clone();
    worker.request_snapshot(move |snap| {
        let s = st.borrow();
        s.page_all.borrow_mut().set_snapshot(snap.clone());
        s.page_installed.borrow_mut().set_snapshot(snap.clone());
        s.page_upgradable.borrow_mut().set_snapshot(snap);
    });

    // 纯 deb 打开场景 show=false:主窗口保持隐藏(应用不退出、对话框有 parent),不进主页面
    if show {
        window.present();
    }
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
        // 注:不再弹确认框——用户点安装/卸载即已知晓需要 root 权限,
        // pkexec 会弹出系统认证框进行授权。
        UiEvent::OpInstall(name) => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            let title = format!("{}: {name}", i18n::t("install"));
            dialogs::run_op_dialog(
                Some(&win),
                &title,
                &["install", "-y", "--no-install-recommends", &name],
                move || refresh_all(&st),
            );
        }
        UiEvent::OpUpdate(name) => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            let title = format!("{}: {name}", i18n::t("update"));
            dialogs::run_op_dialog(
                Some(&win),
                &title,
                &["install", "--only-upgrade", "-y", &name],
                move || refresh_all(&st),
            );
        }
        UiEvent::OpRemove(name) => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            let title = format!("{}: {name}", i18n::t("uninstall"));
            dialogs::run_op_dialog(Some(&win), &title, &["remove", "-y", &name], move || {
                refresh_all(&st);
            });
        }
        // 多选批量操作:按动作拼接多包参数
        UiEvent::BatchOp(action, names) => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            let (title, mut args): (String, Vec<&str>) = match action {
                BatchAction::Install => (
                    format!("{} ({})", i18n::t("batch_install"), names.len()),
                    vec!["install", "-y", "--no-install-recommends"],
                ),
                BatchAction::Remove => (
                    format!("{} ({})", i18n::t("batch_remove"), names.len()),
                    vec!["remove", "-y"],
                ),
                BatchAction::Update => (
                    format!("{} ({})", i18n::t("batch_update"), names.len()),
                    vec!["install", "--only-upgrade", "-y"],
                ),
            };
            args.extend(names.iter().map(String::as_str));
            dialogs::run_op_dialog(Some(&win), &title, &args, move || refresh_all(&st));
        }
        UiEvent::ActionUpdateSources => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            dialogs::run_op_dialog(Some(&win), tr!("update_sources"), &["update"], move || {
                refresh_all(&st);
            });
        }
        UiEvent::ActionAutoRemove => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            // 先执行 dry-run 获取要删除的包列表
            let (packages, success) = apt_worker::dry_run_autoremove();
            if !success {
                dialogs::run_op_dialog(Some(&win), tr!("auto_remove"), &["autoremove", "-y"], move || {
                    refresh_all(&st);
                });
                return;
            }
            // 弹出包列表选择对话框
            let win2 = win.clone();
            dialogs::show_autoremove_dialog(
                Some(&win),
                packages,
                move |selected| {
                    if selected.is_empty() {
                        return;
                    }
                    // 构建命令:autoremove 后跟选中的包名
                    let mut args = vec!["autoremove", "-y"];
                    let pkg_strs: Vec<&str> = selected.iter().map(|s| s.as_str()).collect();
                    args.extend_from_slice(&pkg_strs);
                    let title = tr!("auto_remove").to_string();
                    let st = st.clone();
                    dialogs::run_op_dialog(Some(&win2), &title, &args, move || {
                        refresh_all(&st);
                    });
                },
            );
        }
        UiEvent::ActionUpgradeAll => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            dialogs::run_op_dialog(Some(&win), tr!("upgrade_all"), &["upgrade", "-y"], move || {
                refresh_all(&st);
            });
        }
        UiEvent::ActionCheckUpdates => {
            let st = state.clone();
            let win = state.borrow().window.clone();
            dialogs::run_op_dialog(Some(&win), tr!("check_updates"), &["update"], move || {
                refresh_all(&st);
            });
        }
        // 手动刷新列表(已安装页刷新按钮)
        UiEvent::ActionRefresh => {
            refresh_all(state);
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
