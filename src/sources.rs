use std::fs;

/// 检测系统是否是 Debian
pub fn is_debian() -> bool {
    // 方法1: 检查 /etc/os-release
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("ID=") {
                let value = value.trim().trim_matches('"');
                if value == "debian" {
                    return true;
                }
            }
            // 也检查 ID_LIKE(如 deepin 基于 debian)
            if let Some(value) = line.strip_prefix("ID_LIKE=") {
                let value = value.trim().trim_matches('"');
                if value.contains("debian") {
                    return true;
                }
            }
        }
    }
    false
}

/// 源配置中每个仓库的状态
#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub uri: String,
    pub distribution: String,
    pub components: Vec<String>,
    pub has_non_free: bool,
    pub has_non_free_firmware: bool,
}

/// 解析 sources.list 文件,返回各仓库的 non-free 状态
pub fn parse_sources() -> Vec<RepoStatus> {
    let mut repos = Vec::new();

    // 读取主 sources.list
    if let Ok(content) = fs::read_to_string("/etc/apt/sources.list") {
        parse_sources_content(&content, &mut repos);
    }

    // 读取 sources.list.d/ 下的 .sources 和 .list 文件
    if let Ok(entries) = fs::read_dir("/etc/apt/sources.list.d") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sources")
                || path.extension().and_then(|e| e.to_str()) == Some("list")
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    parse_sources_content(&content, &mut repos);
                }
            }
        }
    }

    repos
}

/// 解析 sources.list 内容
fn parse_sources_content(content: &str, repos: &mut Vec<RepoStatus>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // 跳过空行和注释
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("deb ") {
            // 只处理 deb 类型(deb-src 跳过)
            if !trimmed.starts_with("deb ") {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // 格式: deb http://... distribution component1 component2 ...
            let uri = parts[1].to_string();
            let distribution = parts[2].to_string();
            let components: Vec<String> = parts[3..].iter().map(|s| s.to_string()).collect();

            let has_non_free = components.iter().any(|c| c == "non-free");
            let has_non_free_firmware = components.iter().any(|c| c == "non-free-firmware");

            repos.push(RepoStatus {
                uri,
                distribution,
                components,
                has_non_free,
                has_non_free_firmware,
            });
        }
    }
}

/// 写入 sources.list 并刷新源
/// 返回 (是否成功, 输出信息)
pub fn apply_sources(content: &str) -> (bool, String) {
    // 备份原文件
    let backup_path = "/etc/apt/sources.list.bak";
    if std::path::Path::new("/etc/apt/sources.list").exists() {
        if let Err(e) = fs::copy("/etc/apt/sources.list", backup_path) {
            return (false, format!("备份失败: {e}"));
        }
    }

    // 写入新内容(需要 root 权限,通过 pkexec)
    match std::process::Command::new("pkexec")
        .args(["tee", "/etc/apt/sources.list"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                if let Err(e) = stdin.write_all(content.as_bytes()) {
                    return (false, format!("写入失败: {e}"));
                }
            }
            // 等待 tee 完成
            match child.wait() {
                Ok(status) if status.success() => {
                    // 刷新源
                    match std::process::Command::new("pkexec")
                        .args(["apt-get", "update"])
                        .output()
                    {
                        Ok(out) => {
                            let msg = String::from_utf8_lossy(&out.stdout);
                            (true, msg.to_string())
                        }
                        Err(e) => (false, format!("刷新源失败: {e}")),
                    }
                }
                Ok(_) => (false, "写入失败".to_string()),
                Err(e) => (false, format!("执行失败: {e}")),
            }
        }
        Err(e) => (false, format!("启动失败: {e}")),
    }
}
