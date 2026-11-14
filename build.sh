#!/usr/bin/env bash
# apt-manager 一键构建 + deb 打包脚本
# 用法: ./build.sh           # cargo build --release 后打包成 deb
#       ./build.sh --no-strip # 跳过 strip(调试用)
set -euo pipefail

# ---------- 元信息 ----------
PKG_NAME="apt-manager"
APP_ID="com.liangzhaoyuan12.aptmanager"
MAINTAINER="liangzhaoyuan12 <liangzhaoyuan12@outlook.com>"
HOMEPAGE="https://github.com/liangzhaoyuan12/apt-manager"
DESC_SHORT="这是一个用户管理Debian及其类衍生系统的包管理器GUI软件"
DESC_LONG="基于 GTK4 的软件包管理器图形界面,支持:所有软件/已安装/可更新三个视图、\
软件搜索与分页、更新软件源、安装/更新/卸载软件、autoremove 清理,以及主题与语言切换。"
STRIP_BIN=1
[ "${1:-}" = "--no-strip" ] && STRIP_BIN=0

# ---------- 路径 ----------
cd "$(dirname "$0")"                 # 脚本所在目录,保证从任何地方执行都对
PROJECT_DIR="$(pwd)"
DIST_DIR="$PROJECT_DIR/dist"
BIN_PATH="$PROJECT_DIR/target/release/$PKG_NAME"

# ---------- 读取版本 / 架构 ----------
VERSION="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' Cargo.toml)"
[ -n "$VERSION" ] || { echo "错误:无法从 Cargo.toml 读取版本号" >&2; exit 1; }
ARCH="$(dpkg --print-architecture 2>/dev/null || uname -m)"
DEB_FILE="$DIST_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"

echo "==> 版本: $VERSION"
echo "==> 架构: $ARCH"
echo "==> 输出: $DEB_FILE"

# ---------- 1. cargo release 构建 ----------
command -v cargo >/dev/null || { echo "错误:未找到 cargo" >&2; exit 1; }
echo "==> cargo build --release ..."
cargo build --release

[ -x "$BIN_PATH" ] || { echo "错误:构建产物不存在: $BIN_PATH" >&2; exit 1; }
if [ "$STRIP_BIN" = "1" ]; then
    echo "==> strip 二进制 ..."
    strip -s "$BIN_PATH"
fi

# ---------- 1.5 自动探测二进制实际链接的 libapt-pkg，注入 Depends ----------
# rust-apt 在编译期链接系统 libapt-pkg，SONAME 由构建机当时的版本决定。
# 不能写死版本号: 构建机/目标机 libapt-pkg 大版本(6.0/7.0)不一致就会"能装不能跑"。
APT_PKG_SO="$(objdump -p "$BIN_PATH" 2>/dev/null | awk '/NEEDED/{print $2}' | grep -E '^libapt-pkg\.so\.' | head -1)"
if [ -z "$APT_PKG_SO" ]; then
    echo "警告: 未在二进制中发现 libapt-pkg 依赖" >&2
    APT_PKG_DEP=""
else
    # SONAME libapt-pkg.so.6.0 -> 包名 libapt-pkg6.0
    APT_PKG_DEP="$(printf '%s' "$APT_PKG_SO" | sed -E 's/\.so\././')"
    echo "==> 检测到 libapt-pkg 依赖: $APT_PKG_SO (对应包: $APT_PKG_DEP)"
fi

# ---------- 1.6 构建期防线: 二进制若有未解析的共享库，直接中止 ----------
# 能拦住"构建机缺库 / SONAME 对不上"导致的能装不能跑。
if ldd "$BIN_PATH" 2>/dev/null | grep -q "not found"; then
    echo "错误: 二进制存在未解析的共享库，停止打包:" >&2
    ldd "$BIN_PATH" | grep "not found" >&2
    exit 1
fi

# ---------- 1.7 动态拼装 Depends (跟随二进制实际链接的库) ----------
DEPS="libgtk-4-1 (>= 4.6), libadwaita-1-0, libpango-1.0-0, libglib2.0-0, libc6"
[ -n "$APT_PKG_DEP" ] && DEPS="$DEPS, $APT_PKG_DEP"
DEPS="$DEPS, apt, dpkg, sudo"

# ---------- 2. 组装 deb 目录 ----------
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# 2.1 二进制
install -Dm755 "$BIN_PATH" "$STAGE/usr/bin/$PKG_NAME"

# 2.2 .desktop 入口
# 文件名必须与 application_id 一致(com.liangzhaoyuan12.aptmanager.desktop),
# 否则 KDE Plasma Wayland 的 dock 通过 app-id 匹配不到 desktop 文件,图标显示为 Wayland 默认。
mkdir -p "$STAGE/usr/share/applications" "$STAGE/DEBIAN"
cat > "$STAGE/usr/share/applications/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Apt Manager
Name[zh_CN]=APT 管理器
GenericName=Package Manager
GenericName[zh_CN]=软件包管理器
Comment=$DESC_SHORT
Comment[zh_CN]=$DESC_SHORT
# Exec 必须带 %U:没有该占位符时,应用管理器(文件管理器打开方式)不会传文件参数
Exec=$PKG_NAME %U
Icon=$APP_ID
Terminal=false
Categories=System;PackageManager;Utility;
Keywords=apt;dpkg;package;install;update;
StartupNotify=true
# 窗口 WM_CLASS 是二进制名,dock 按它归组显示图标
StartupWMClass=$PKG_NAME
# 作为 .deb 文件打开方式(双击 deb 用 apt-manager 安装)
MimeType=application/vnd.debian.binary-package;application/x-deb;
EOF

# 2.3 应用图标 (从项目根目录 icon.png,512x512)
if [ -f "$PROJECT_DIR/icon.png" ]; then
    install -Dm644 "$PROJECT_DIR/icon.png" \
        "$STAGE/usr/share/icons/hicolor/512x512/apps/$APP_ID.png"
else
    echo "警告: 未找到 icon.png,跳过图标" >&2
fi

# 2.4 控制信息
cat > "$STAGE/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: $MAINTAINER
Homepage: $HOMEPAGE
Depends: $DEPS
Description: $DESC_SHORT
 $DESC_LONG
EOF

# 2.5 升级/卸载时清理旧版遗留的 desktop 文件
# 0.6.0 及以前用 apt-manager.desktop,更早版本用反向域名 com.liangzhaoyuan12.aptmanager.desktop(误)
cat > "$STAGE/DEBIAN/prerm" <<'EOF'
#!/bin/sh
rm -f /usr/share/applications/apt-manager.desktop
rm -f /usr/share/applications/com.liangzhaoyuan12.aptmanager.desktop
exit 0
EOF
chmod 755 "$STAGE/DEBIAN/prerm"

# ---------- 3. 打包 ----------
mkdir -p "$DIST_DIR"
echo "==> dpkg-deb 打包 ..."
dpkg-deb --build --root-owner-group "$STAGE" "$DEB_FILE"

# ---------- 4. 校验并输出 ----------
echo "==> 校验包信息:"
dpkg-deb --info "$DEB_FILE"
echo "==> 内容清单:"
dpkg-deb --contents "$DEB_FILE"
echo
echo "打包完成: $DEB_FILE"
echo "安装: sudo apt install $DEB_FILE   (或 sudo dpkg -i $DEB_FILE && sudo apt -f install)"
