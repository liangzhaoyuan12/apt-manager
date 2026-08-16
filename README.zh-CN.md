# apt-manager

一个基于 GTK4 + Rust 的 Debian 及其衍生系统软件包管理器图形界面。

[English](README.md) | 简体中文

## 功能特性

- 三个视图:所有软件(apt list)、已安装、可更新
- 软件搜索 + 分页(每页 50 条)
- 更新软件源(apt update)
- 安装 / 更新 / 卸载软件,autoremove 自动清理无用依赖
- 主题切换(跟随系统 / 浅色 / 深色),记忆偏好
- 语言切换(简体中文 / English / 跟随系统),记忆偏好
- 软件详情页:应用名 / 包名 / 版本 / 架构 / 维护者 / 更新时间 / 安装后占用 / 安装包大小 / 下载地址 / 主页 / 桌面文件 / 执行命令 / 依赖 / 描述,含状态徽章(已安装 / 可更新)
- 变更操作通过 `pkexec` 提权执行 `apt-get`,root 用户直接执行

## 技术栈

| 组件 | 说明 |
| --- | --- |
| Rust | edition 2024 |
| GTK4 | gtk4-rs 0.11,最低 GTK 4.6 |
| glib / pango | 基础库与文本布局 |
| rust-apt | 读取系统 apt / dpkg 数据库 |
| tokio | 异步执行 apt-get 并流式回传输出 |
| libc | 检测 euid,决定提权方式 |

## 系统要求

- Debian 系系统(Debian / Ubuntu / Deepin 等)
- GTK 4.6 及以上
- 支持所有 CPU 架构:amd64、arm64、loong64(龙芯)、riscv64、ppc64el、s390x 等
- 软件变更操作需要 `pkexec`(policykit-1 或 pkexec 包)

## 安装

Release 目前提供两种架构的预编译 deb:

| 架构 | 说明 |
| --- | --- |
| amd64 | 主流 x86_64 |
| loong64 | 龙芯 LoongArch64 |

```bash
sudo apt install ./apt-manager_0.1.0_amd64.deb
```

其他架构(arm64、riscv64 等)请按下一节自行编译打包。

## 从源码编译打包

### 编译依赖

- Rust 工具链(rustc 1.85+,支持 edition 2024)
- pkg-config
- libgtk-4-dev(>= 4.6)

```bash
# Debian 系安装编译依赖
sudo apt install pkg-config libgtk-4-dev
```

### 一键打包

项目根目录提供 `build.sh`,自动完成 release 构建并封装成 deb:

```bash
./build.sh
```

脚本自动完成:

1. `cargo build --release`
2. strip 二进制(可用 `./build.sh --no-strip` 跳过)
3. 打包二进制、`.desktop` 入口、`icon.png` 图标
4. 自动读取版本(Cargo.toml)与当前架构(`dpkg --print-architecture`)
5. 生成 deb 并校验输出

产物:`dist/apt-manager_<版本>_<架构>.deb`,例如:

```bash
# loong64 上执行后:
./build.sh
# => dist/apt-manager_0.1.0_loong64.deb

# 安装
sudo apt install ./dist/apt-manager_0.1.0_loong64.deb
```

在任意架构的 Debian 系机器上执行 `./build.sh` 都会得到对应架构的 deb。

### 运行依赖(deb 自动处理)

`libgtk-4-1 (>= 4.6)`、`libpango-1.0-0`、`libglib2.0-0`、`libc6`、`apt`、`dpkg`、`pkexec | policykit-1`

## 开源协议

[MIT License](LICENSE),版权所有 © 2026 liangzhaoyuan12
