use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use rust_apt::cache::PackageSort;
use rust_apt::{new_cache, Cache};

/// 包列表中的单个条目(快照)
#[derive(Debug, Clone)]
pub struct PkgInfo {
    pub name: String,
    pub version: String, // 候选版本
    #[allow(dead_code)]
    pub installed_version: String,
    pub summary: String,
    pub installed: bool,
    pub upgradable: bool,
}

/// 包详情(详情页用)
#[derive(Debug, Clone)]
pub struct PkgDetail {
    pub name: String,             // 包名
    pub display_name: String,     // 应用名(desktop 文件 Name,未找到时回退包名)
    pub candidate_version: String,
    pub installed_version: String,
    pub summary: String,
    pub description: String,
    pub section: String,
    pub installed_size: u64,      // 安装后占用(字节)
    pub package_size: u64,        // deb 包大小(字节)
    pub maintainer: String,
    pub architecture: String,
    pub homepage: String,
    pub download_url: String,     // 完整下载地址
    pub date: String,             // 更新时间(已安装包取 dpkg info 文件 mtime)
    pub desktop_file: String,     // 桌面文件路径
    pub exec_command: String,     // 执行命令(desktop Exec)
    pub exec_path: String,        // 执行路径(Exec 第一个词)
    pub depends: String,          // 依赖(原始 Depends 字段)
    pub installed: bool,
    pub upgradable: bool,
}

/// 后台工作线程请求
enum Req {
    Snapshot(Sender<Vec<PkgInfo>>),
    Detail {
        name: String,
        reply: Sender<Option<PkgDetail>>,
    },
}

/// 与 UI 线程通信的句柄
pub struct AptWorker {
    tx: Sender<Req>,
}

impl AptWorker {
    /// 启动后台线程并立即开始加载快照
    pub fn start() -> Self {
        let (tx, rx) = channel::<Req>();
        thread::Builder::new()
            .name("apt-worker".into())
            .spawn(move || worker_loop(rx))
            .expect("failed to spawn apt worker thread");
        AptWorker { tx }
    }

    /// 异步请求包快照,完成后在主线程回调。
    /// glib 0.22 无 MainContext::channel:std mpsc + 主循环 timeout 轮询。
    pub fn request_snapshot(&self, cb: impl FnOnce(Vec<PkgInfo>) + 'static) {
        let (reply_tx, reply_rx) = channel::<Vec<PkgInfo>>();
        let _ = self.tx.send(Req::Snapshot(reply_tx));
        let mut cb = Some(cb);
        glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
            match reply_rx.try_recv() {
                Ok(data) => {
                    if let Some(cb) = cb.take() {
                        cb(data);
                    }
                    glib::ControlFlow::Break
                }
                Err(_) => glib::ControlFlow::Continue,
            }
        });
    }

    /// 异步请求包详情,完成后在主线程回调
    pub fn request_detail(&self, name: String, cb: impl FnOnce(Option<PkgDetail>) + 'static) {
        let (reply_tx, reply_rx) = channel::<Option<PkgDetail>>();
        let _ = self.tx.send(Req::Detail {
            name,
            reply: reply_tx,
        });
        let mut cb = Some(cb);
        glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
            match reply_rx.try_recv() {
                Ok(data) => {
                    if let Some(cb) = cb.take() {
                        cb(data);
                    }
                    glib::ControlFlow::Break
                }
                Err(_) => glib::ControlFlow::Continue,
            }
        });
    }
}

fn worker_loop(rx: Receiver<Req>) {
    // cache 惰性创建;每次 Snapshot 都重建,否则 apt-get 卸载/安装/update 后
    // 内存 cache 仍是旧状态,列表刷新拿到的是过期数据。
    let mut cache: Option<Cache> = None;
    while let Ok(req) = rx.recv() {
        match req {
            Req::Snapshot(reply) => {
                match new_cache!() {
                    Ok(c) => {
                        let snap = build_snapshot(&c);
                        cache = Some(c);
                        let _ = reply.send(snap);
                    }
                    Err(e) => {
                        eprintln!("apt cache rebuild failed: {e:?}");
                        let _ = reply.send(Vec::new());
                    }
                }
            }
            Req::Detail { name, reply } => {
                if cache.is_none() {
                    match new_cache!() {
                        Ok(c) => cache = Some(c),
                        Err(e) => {
                            eprintln!("apt cache open failed: {e:?}");
                            let _ = reply.send(None);
                            continue;
                        }
                    }
                }
                let c = cache.as_ref().expect("cache initialized above");
                let detail = build_detail(c, &name);
                let _ = reply.send(detail);
            }
        }
    }
}

/// 全量快照:69k 包带摘要约 7 秒,后台线程执行
fn build_snapshot(cache: &Cache) -> Vec<PkgInfo> {
    let mut out = Vec::with_capacity(70000);
    for pkg in cache.packages(&PackageSort::default().names()) {
        let name = pkg.name().to_string();
        let candidate_version = pkg.candidate().map(|v| v.version().to_string()).unwrap_or_default();
        let installed_version = pkg
            .installed()
            .map(|v| v.version().to_string())
            .unwrap_or_default();
        let summary = pkg.candidate().and_then(|v| v.summary()).unwrap_or_default();
        out.push(PkgInfo {
            name,
            version: candidate_version,
            installed_version,
            summary,
            installed: pkg.is_installed(),
            upgradable: pkg.is_upgradable(),
        });
    }
    out
}

fn build_detail(cache: &Cache, name: &str) -> Option<PkgDetail> {
    let pkg = cache.get(name)?;
    let cand = pkg.candidate();
    let inst = pkg.installed();
    let pkg_name = pkg.name().to_string();

    // 任意索引字段:候选版本优先,回退已安装版本
    let rec = |field: &str| {
        cand.as_ref()
            .and_then(|v| v.get_record(field))
            .or_else(|| inst.as_ref().and_then(|v| v.get_record(field)))
    };

    // 桌面文件信息(仅已安装包有本地文件)
    let zh = crate::i18n::current_is_zh();
    let mut desktop_file = String::new();
    let mut display_name = pkg_name.clone();
    let mut exec_command = String::new();
    if let Some(list) = installed_file_list(&pkg_name) {
        if let Some(df) = find_desktop_file(&list) {
            if let Some((n, e)) = parse_desktop(&df, zh) {
                desktop_file = df;
                display_name = n;
                exec_command = e;
            }
        }
    }

    // 更新时间:Deepin 仓库索引无 Date 字段,已安装包用 dpkg info 文件 mtime 近似
    let date = inst
        .as_ref()
        .and_then(|_| fmt_file_mtime(&format!("/var/lib/dpkg/info/{pkg_name}.list")))
        .unwrap_or_default();

    Some(PkgDetail {
        name: pkg_name,
        display_name,
        candidate_version: cand.as_ref().map(|v| v.version().to_string()).unwrap_or_default(),
        installed_version: inst.as_ref().map(|v| v.version().to_string()).unwrap_or_default(),
        summary: cand
            .as_ref()
            .and_then(|v| v.summary())
            .or_else(|| inst.as_ref().and_then(|v| v.summary()))
            .unwrap_or_default(),
        description: cand
            .as_ref()
            .and_then(|v| v.description())
            .or_else(|| inst.as_ref().and_then(|v| v.description()))
            .unwrap_or_default(),
        section: cand.as_ref().and_then(|v| v.section().ok()).unwrap_or("").to_string(),
        // rust-apt installed_size() 已把 Installed-Size(KB) 转成字节
        installed_size: inst
            .as_ref()
            .map(|v| v.installed_size())
            .or_else(|| cand.as_ref().map(|v| v.installed_size()))
            .unwrap_or(0),
        package_size: cand.as_ref().map(|v| v.size()).unwrap_or(0),
        maintainer: rec("Maintainer").unwrap_or_default(),
        architecture: cand.as_ref().map(|v| v.arch().to_string()).unwrap_or_default(),
        homepage: rec("Homepage").unwrap_or_default(),
        download_url: cand.as_ref().and_then(|v| v.uris().next()).unwrap_or_default(),
        date,
        desktop_file,
        exec_command: exec_command.clone(),
        exec_path: exec_command.split_whitespace().next().unwrap_or("").to_string(),
        depends: rec("Depends").unwrap_or_default(),
        installed: pkg.is_installed(),
        upgradable: pkg.is_upgradable(),
    })
}

/// 已安装包的文件清单(dpkg -L 等价:读 /var/lib/dpkg/info/<name>.list)
fn installed_file_list(name: &str) -> Option<Vec<String>> {
    let path = format!("/var/lib/dpkg/info/{name}.list");
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.lines().map(|l| l.trim_end().to_string()).collect())
}

/// 在包文件清单里找 /usr/share/applications/ 下的第一个 .desktop
fn find_desktop_file(list: &[String]) -> Option<String> {
    list.iter()
        .find(|p| p.starts_with("/usr/share/applications/") && p.ends_with(".desktop"))
        .cloned()
}

/// 解析 desktop 文件,返回 (显示名, Exec 命令)。
/// 显示名按当前语言取 Name[zh_CN],否则回退 Name。
fn parse_desktop(path: &str, zh: bool) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut name_plain = String::new();
    let mut name_zh = String::new();
    let mut exec = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "Exec" => exec = v.to_string(),
            "Name" if name_plain.is_empty() => name_plain = v.to_string(),
            "Name[zh_CN]" if name_zh.is_empty() => name_zh = v.to_string(),
            _ => {}
        }
    }
    let name = if zh && !name_zh.is_empty() {
        name_zh
    } else {
        name_plain
    };
    if name.is_empty() {
        None
    } else {
        Some((name, exec))
    }
}

/// 文件 mtime 格式化为 "YYYY-MM-DD HH:MM:SS"(本地时区)
fn fmt_file_mtime(path: &str) -> Option<String> {
    let secs = std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as libc::time_t;
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let mut t = secs;
        if libc::localtime_r(&mut t, &mut tm).is_null() {
            return None;
        }
        let mut buf = [0u8; 64];
        let fmt = b"%Y-%m-%d %H:%M:%S\0";
        let n = libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            fmt.as_ptr() as *const libc::c_char,
            &tm,
        );
        if n == 0 {
            return None;
        }
        Some(String::from_utf8_lossy(&buf[..n]).to_string())
    }
}

// ---- 变更操作:pkexec apt-get ----

/// 异步执行 apt 变更操作,逐行回调输出,结束时回调结果。
/// root 直接执行 apt-get;非 root 通过 pkexec 提权(pkexec 弹系统认证框)。
pub fn run_apt_op(
    args: Vec<String>,
    on_line: impl Fn(String) + Send + 'static,
    on_done: impl FnOnce(bool) + Send + 'static,
) {
    thread::Builder::new()
        .name("apt-op".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()
                .expect("tokio runtime");
            runtime.block_on(async move {
                let is_root = unsafe { libc::geteuid() == 0 };

                let mut cmd = if is_root {
                    tokio::process::Command::new("apt-get")
                } else {
                    // pkexec:PolicyKit 认证,弹系统级认证对话框
                    let mut c = tokio::process::Command::new("pkexec");
                    c.arg("apt-get");
                    c
                };
                cmd.args(&args);
                let mut child = match cmd
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        on_line(format!("spawn failed: {e}"));
                        on_done(false);
                        return;
                    }
                };

                // 统一输出行通道(stdout/stderr 各自线程读)
                let (line_tx, line_rx) = tokio::sync::mpsc::channel::<String>(256);
                if let Some(stdout) = child.stdout.take() {
                    let tx = line_tx.clone();
                    tokio::spawn(async move {
                        read_lines(stdout, tx).await;
                    });
                }
                if let Some(stderr) = child.stderr.take() {
                    let tx = line_tx.clone();
                    tokio::spawn(async move {
                        read_lines(stderr, tx).await;
                    });
                }
                drop(line_tx);
                // 等待退出并转发输出。输出 EOF 后停止轮询 line 分支,
                // 否则输出结束但子进程未退出时 recv() 立即返回 None,select 无
                // await 点无限空转,占满一个 CPU 核(窗口关闭也无法终止它)。
                let status = await_child_output(&mut child, line_rx, &on_line).await;
                match status {
                    Ok(s) => on_done(s.success()),
                    Err(e) => {
                        on_line(format!("wait failed: {e}"));
                        on_done(false);
                    }
                }
            });
        })
        .expect("failed to spawn apt-op thread");
}

/// 等待子进程退出,同时把 stdout/stderr 行转发给 on_line。
/// 关键:输出流全部关闭后(EOF,line_rx 所有 sender drop)不再 poll line_rx,
/// 否则 recv() 每次立即返回 None,tokio::select! 会忙转占满一个核。
async fn await_child_output(
    child: &mut tokio::process::Child,
    mut line_rx: tokio::sync::mpsc::Receiver<String>,
    on_line: &(dyn Fn(String) + Send),
) -> std::io::Result<std::process::ExitStatus> {
    let mut output_closed = false;
    loop {
        tokio::select! {
            line = line_rx.recv(), if !output_closed => {
                match line {
                    Some(l) => on_line(l),
                    None => output_closed = true,
                }
            }
            status = child.wait() => return status,
        }
    }
}

async fn read_lines<R: tokio::io::AsyncRead + Unpin>(
    stream: R,
    tx: tokio::sync::mpsc::Sender<String>,
) {
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if tx.send(line).await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_desktop_prefers_applications_dir() {
        let list = vec![
            "/usr/share/doc".to_string(),
            "/usr/share/applications/deepin-music.desktop".to_string(),
        ];
        assert_eq!(
            find_desktop_file(&list).as_deref(),
            Some("/usr/share/applications/deepin-music.desktop")
        );
        assert_eq!(find_desktop_file(&[]), None);
    }

    #[test]
    fn desktop_parse_zh_and_exec() {
        let (name, exec) = parse_desktop("/usr/share/applications/deepin-music.desktop", true)
            .expect("desktop file");
        assert_eq!(name, "音乐");
        assert_eq!(exec, "deepin-music %F");
    }

    #[test]
    fn desktop_parse_en_fallback() {
        let (name, _) = parse_desktop("/usr/share/applications/deepin-music.desktop", false)
            .expect("desktop file");
        assert_eq!(name, "Deepin Music");
    }

    #[test]
    fn mtime_is_local_datetime() {
        let s = fmt_file_mtime("/var/lib/dpkg/info/deepin-music.list").expect("mtime");
        assert_eq!(s.len(), 19);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[13..14], ":");
    }

    /// 端到端:build_detail 全字段(依赖本机 apt 缓存与已安装的 deepin-music)
    #[test]
    fn detail_all_fields() {
        let cache = new_cache!().expect("apt cache");
        let d = build_detail(&cache, "deepin-music").expect("detail");
        assert_eq!(d.name, "deepin-music");
        assert_eq!(d.display_name, "音乐");
        assert_eq!(d.candidate_version, "7.0.61");
        assert_eq!(d.installed_version, "7.0.61");
        assert_eq!(d.architecture, "loong64");
        assert!(d.maintainer.contains("packages@deepin.com"));
        assert!(d.homepage.contains("deepin.org"));
        assert!(d.depends.contains("libc6 (>= 2.38)"));
        assert!(d.download_url.contains("deepin-music_7.0.61_loong64.deb"));
        assert_eq!(d.desktop_file, "/usr/share/applications/deepin-music.desktop");
        assert_eq!(d.exec_command, "deepin-music %F");
        assert_eq!(d.exec_path, "deepin-music");
        assert_eq!(d.installed_size, 27_776_000); // 26.49 MiB
        assert_eq!(d.package_size, 21_845_500); // 20.8 MiB
        assert!(!d.date.is_empty());
        assert!(d.installed);
    }
}
