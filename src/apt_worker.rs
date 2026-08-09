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
    pub name: String,
    pub candidate_version: String,
    pub installed_version: String,
    pub summary: String,
    pub description: String,
    pub section: String,
    pub size: u64,
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
    let cache: Cache = match new_cache!() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("apt cache open failed: {e:?}");
            return;
        }
    };
    while let Ok(req) = rx.recv() {
        match req {
            Req::Snapshot(reply) => {
                let snap = build_snapshot(&cache);
                let _ = reply.send(snap);
            }
            Req::Detail { name, reply } => {
                let detail = build_detail(&cache, &name);
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
    Some(PkgDetail {
        name: pkg.name().to_string(),
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
        size: cand.as_ref().map(|v| v.size()).unwrap_or(0),
        installed: pkg.is_installed(),
        upgradable: pkg.is_upgradable(),
    })
}

// ---- 变更操作:pkexec apt-get ----

/// 异步执行 apt 变更操作,逐行回调输出,结束时回调结果。
/// 以普通用户运行时通过 pkexec 提权;root 直接执行。
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
                let mut cmd = tokio::process::Command::new(if is_root { "apt-get" } else { "pkexec" });
                if !is_root {
                    cmd.arg("apt-get");
                }
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
                let (line_tx, mut line_rx) = tokio::sync::mpsc::channel::<String>(256);
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
                loop {
                    tokio::select! {
                        line = line_rx.recv() => {
                            match line {
                                Some(l) => on_line(l),
                                None => { /* 所有输出结束,继续等子进程退出 */ }
                            }
                        }
                        status = child.wait() => {
                            match status {
                                Ok(s) => on_done(s.success()),
                                Err(e) => {
                                    on_line(format!("wait failed: {e}"));
                                    on_done(false);
                                }
                            }
                            break;
                        }
                    }
                }
            });
        })
        .expect("failed to spawn apt-op thread");
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
