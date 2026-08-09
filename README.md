# apt-manager

A graphical package manager for Debian and its derivatives, built with GTK4 + Rust.

English | [简体中文](README.zh-CN.md)

## Features

- Three views: All packages (`apt list`), Installed, Upgradable
- Package search + pagination (50 items per page)
- Update package lists (`apt update`)
- Install / upgrade / remove packages, `autoremove` for unused dependencies
- Theme switching (System / Light / Dark), preference remembered
- Language switching (Simplified Chinese / English / System), preference remembered
- Detail page: description, version, status badges (installed / upgradable)
- Change operations run `apt-get` via `pkexec` elevation; executed directly when root

## Tech Stack

| Component | Description |
| --- | --- |
| Rust | edition 2024 |
| GTK4 | gtk4-rs 0.11, requires GTK 4.6+ |
| glib / pango | Core libraries and text layout |
| rust-apt | Reads the system apt / dpkg database |
| tokio | Asynchronously runs apt-get and streams output |
| libc | Detects euid to decide the elevation path |

## Requirements

- A Debian-based system (Debian / Ubuntu / Deepin, etc.)
- GTK 4.6 or newer
- All CPU architectures supported: amd64, arm64, loong64 (LoongArch), riscv64, ppc64el, s390x, etc.
- Package-changing operations need `pkexec` (provided by the `pkexec` or `policykit-1` package)

## Installation

Prebuilt deb packages are provided for two architectures on the Release page:

| Arch | Notes |
| --- | --- |
| amd64 | mainstream x86_64 |
| loong64 | Loongson LoongArch64 |

```bash
sudo apt install ./apt-manager_0.1.0_amd64.deb
```

For other architectures (arm64, riscv64, ...), build the package yourself as described below.

## Building from Source

### Build Dependencies

- Rust toolchain (rustc 1.85+, edition 2024 support)
- pkg-config
- libgtk-4-dev (>= 4.6)

```bash
# Debian-based: install build dependencies
sudo apt install pkg-config libgtk-4-dev
```

### One-shot Packaging

`build.sh` at the project root builds a release binary and wraps it into a deb:

```bash
./build.sh
```

What the script does:

1. `cargo build --release`
2. strip the binary (`./build.sh --no-strip` to skip)
3. Package the binary, `.desktop` entry and `icon.png`
4. Read the version from Cargo.toml and the architecture from `dpkg --print-architecture` automatically
5. Build the deb and verify the output

Output: `dist/apt-manager_<version>_<arch>.deb`, e.g.:

```bash
# after running on loong64:
./build.sh
# => dist/apt-manager_0.1.0_loong64.deb

# install
sudo apt install ./dist/apt-manager_0.1.0_loong64.deb
```

Running `./build.sh` on any Debian-based machine produces a deb for that machine's architecture.

### Runtime Dependencies (handled automatically by the deb)

`libgtk-4-1 (>= 4.6)`, `libpango-1.0-0`, `libglib2.0-0`, `libc6`, `apt`, `dpkg`, `pkexec | policykit-1`

## License

[MIT License](LICENSE), Copyright (c) 2026 liangzhaoyuan12
