# aria_move (Rust)

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](#license) [![Build](https://img.shields.io/badge/build-cargo-blue.svg)](#development) [![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-orange.svg)](#usage)

---

## Table of Contents

- [Requirements & Build Tools](#requirements--build-tools)
- [Installation](#installation)
- [Usage](#usage)
    - [--help snapshot](#help-snapshot)
- [Integration with aria2](#aria2-integration)
    - [systemd integration](#systemd-integration)
- [Configuration](#configuration)
- [Logging](#logging)
- [Platform Feature Matrix](#platform-feature-matrix)
- [Links](#links)
- [License](#license)
- [Contributing](#contributing)

---

aria_move makes moving completed downloads effortless and safe — whether you run a single desktop client or manage a headless download server. Install in minutes, plug it into aria2 or any downloader hook, and let aria_move reliably place finished files into a curated completed directory with zero fuss.

Designed for ease-of-use and reliability: quick sensible defaults, a tiny XML config you can edit later, safe-by-default behavior (no symlink-trickery, secure log file handling on Unix), and robust fallbacks when a straight rename isn't possible.

## Why choose aria_move?

- Zero-surprise operation: safe defaults so you can run it unattended.
- Plug-and-play with aria2 (or any hook) — pass the task id, file count and source path and you're done.
- Fast and efficient: atomic renames when possible, reliable copy+rename fallback across filesystems.
- Safe for production: symlink defenses, disk-space checks (Unix), and secure log/config file handling.

## Key features (end users)

- Automatic move of completed items from download base to completed base
- Dry-run mode to preview actions without touching files
- Optional preservation of file permissions and timestamps
- Secure defaults: refuses to use log paths with symlinked ancestors on Unix
- Creates a secure template config on first run if none exists
- Cross-platform (macOS, Linux, Windows)

## Key features (for developers & integrators)

- Small, modular codebase with platform helpers for Unix/Windows separation
- Test suite covering races, symlink defenses and I/O helpers
- Structured, documented errors (AriaMoveError) for easy assertion in integration tests
- Traces and optional JSON logs for integration with log collectors
- Easy to extend: clear fs/ and platform/ boundaries to add features safely

---

## Requirements & build tools

This project is written in Rust. You need the Rust toolchain, Git, and a few native build tools (pkg-config / C toolchain / OpenSSL headers) on some platforms. Install the items below for your OS before building.

### Common (all platforms)

- **rustup** — the recommended way to install Rust (provides rustc, cargo).
- **git** — to clone the repository.
- **Build tools** — a C toolchain and `pkg-config` are required by some crates.
- **Extras for development:** `rustfmt` and `clippy` (install via rustup).

Install the Rust toolchain and developer components:

```bash
# Install rustup (one-liner)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup component add rustfmt clippy
rustc --version
cargo --version
```

### macOS (Homebrew)

```bash
xcode-select --install
brew install pkg-config openssl@3
export OPENSSL_DIR="$(brew --prefix openssl@3)"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig:$PKG_CONFIG_PATH"
```

### Debian / Ubuntu

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl git
# optional: sudo apt install -y clang
```

### Fedora / RHEL (dnf)

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config openssl-devel git
```

### Windows

Pick one toolchain:

- **MSVC (recommended)**: Install “Build Tools for Visual Studio” with “Desktop development with C++”. Then:

```powershell
rustup default stable
rustup component add rustfmt clippy
```

- **MSYS2/MinGW** (alternative):

```bash
pacman -Syu
pacman -S --needed base-devel mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkg-config mingw-w64-x86_64-openssl
# use the mingw64 shell to build
```

> Notes: If native deps complain about OpenSSL, install platform dev packages (`libssl-dev`/`openssl-devel`) and export `OPENSSL_DIR`/`PKG_CONFIG_PATH` as above.

---

## Installation

Pick the method that fits your environment.

### Easy (cross‑platform)

```bash
cargo install --path .
aria_move --version
```

### Build from source (manual)

```bash
cargo build --release
# binary: target/release/aria_move
sudo cp target/release/aria_move /usr/local/bin/    # macOS/Linux
# or copy aria_move.exe into a directory on PATH (Windows)
```

### Platform-specific notes

#### macOS

```bash
xcode-select --install
brew install pkg-config openssl@3
export OPENSSL_DIR="$(brew --prefix openssl@3)"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig:$PKG_CONFIG_PATH"
cargo install --path .   # or: cargo build --release
```

#### Debian/Ubuntu

```bash
sudo apt install -y build-essential pkg-config libssl-dev curl git
cargo install --path .   # or: cargo build --release
```

#### Fedora/RHEL

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config openssl-devel git
cargo install --path .   # or: cargo build --release
```

#### Windows

```powershell
# MSVC
rustup default stable
cargo install --path .
# MSYS2 (from mingw64 shell)
cargo install --path .
```

**Uninstall:**

```bash
cargo uninstall aria_move
```

---

## Usage

### Synopsis

```text
aria_move [TASK_ID] [NUM_FILES] [SOURCE_PATH] [FLAGS]
```

### Positional Arguments (Optional)

When integrating with aria2, these are typically passed by the download-complete hook:

| Argument     | Type   | Description                      |
|--------------|--------|----------------------------------|
| `TASK_ID`    | String | aria2 GID (informational)        |
| `NUM_FILES`  | Integer| Number of files (0 if unknown)   |
| `SOURCE_PATH`| Path   | File or directory to move        |

### Common Flags

| Flag                                | Description                                      |
|-------------------------------------|--------------------------------------------------|
| `--download-base <PATH>`            | Override download base directory                 |
| `--completed-base <PATH>`           | Override completed base directory                |
| `-d`, `--debug`                     | Set log level to debug                            |
| `--log-level <LEVEL>`               | quiet, normal, info, debug                       |
| `--print-config`                    | Show config file location and exit               |
| `--dry-run`                         | Log actions without modifying filesystem         |
| `--preserve-metadata`               | Preserve permissions, timestamps, xattrs (feature)|
| `--preserve-permissions`            | Preserve only permissions (faster)               |
| `--json`                            | Emit logs in JSON format                         |

### Examples

```bash
# Auto-resolve most recent file
aria_move

# Move a specific path (typical aria2 hook)
aria_move 7b3f1234 1 /path/to/incoming/file.iso

# Override bases
aria_move --download-base /data/incoming --completed-base /data/completed

# Dry run with JSON logs
aria_move --dry-run --json

# Show config location
aria_move --print-config
```

---

### --help snapshot (example)

```
aria_move 0.6.0
Move completed aria2 downloads safely (Rust)

USAGE:
    aria_move [OPTIONS] [TASK_ID] [NUM_FILES] [SOURCE_PATH]

ARGS:
    <TASK_ID>        Aria2 task id (optional, informational)
    <NUM_FILES>      Number of files reported by aria2 (0 = unknown)
    <SOURCE_PATH>    Source path passed by aria2

OPTIONS:
        --download-base <PATH>      Override the download base directory
        --completed-base <PATH>     Override the completed base directory
    -d, --debug                      Enable debug logging (shorthand for --log-level debug)
        --log-level <LEVEL>         Set log level: quiet, normal, info, debug
        --print-config              Print the config file location used by aria_move and exit
        --dry-run                   Show what would be done, but do not modify files/directories
    --preserve-metadata         Preserve permissions, timestamps and xattrs (when enabled); slower
    --preserve-permissions      Preserve only permissions (mode/readonly); faster than --preserve-metadata
        --json                      Emit logs in structured JSON
    -h, --help                       Print help
    -V, --version                    Print version
```

---

## Integration with aria2

aria2 exposes an on-download-complete hook you configure in `aria2.conf`. Use absolute paths; aria2 runs the hook under its own environment. A tiny wrapper script is recommended.

### Important notes

- Use absolute paths for the wrapper and the aria_move binary.
- Make the wrapper executable: `chmod +x /usr/local/bin/aria_move_hook.sh`.
- Run aria2 as the user that should own/see the download and completed directories.
- Test the wrapper manually before adding it to aria2.conf.

### Example — Unix (bash) wrapper

```bash
#!/usr/bin/env bash
# filepath: /usr/local/bin/aria_move_hook.sh
# Make executable: chmod +x /usr/local/bin/aria_move_hook.sh
exec /usr/local/bin/aria_move "$1" "$2" "$3"
```

Add to `aria2.conf`:

```text
on-download-complete=/usr/local/bin/aria_move_hook.sh
```

### Example — Windows (batch) wrapper

```bat
@echo off
REM filepath: C:\Program Files\aria_move\aria_move_hook.bat
"C:\Program Files\aria_move\aria_move.exe" %1 %2 %3
```

Add to `aria2.conf`:

```text
on-download-complete=C:\Program Files\aria_move\aria_move_hook.bat
```

### systemd integration

If aria2 runs under systemd, ensure the service user and environment are correct, and that the wrapper path is absolute.

```ini
# Example override: sudo systemctl edit aria2c.service
[Service]
User=aria2
# Ensure /usr/local/bin is on PATH (or use absolute paths in aria2.conf as above)
Environment=PATH=/usr/local/bin:/usr/bin
# Ensure aria2.conf contains the on-download-complete=/usr/local/bin/aria_move_hook.sh line
```

---

## Configuration

### Config File Location (XML)

| Platform | Default Path |
|----------|--------------|
| macOS    | `~/Library/Application Support/aria_move/config.xml` |
| Linux    | `~/.config/aria_move/config.xml` |
| Windows  | `%APPDATA%\aria_move\config.xml` |

### Override Config Location

Set the `ARIA_MOVE_CONFIG` environment variable:

```bash
# macOS/Linux
export ARIA_MOVE_CONFIG=/custom/path/to/config.xml
aria_move

# Windows (PowerShell)
$env:ARIA_MOVE_CONFIG = "C:\custom\path\to\config.xml"
aria_move
```

### First Run Behavior

If no config exists and `ARIA_MOVE_CONFIG` is unset, aria_move creates a secure template and exits. Edit it and rerun.

### Security Notes

- **macOS/Linux:** `download_base` and `completed_base` must be owned by the current user and not group/world writable (mode & 0o022 == 0).
- **Windows:** Basic readonly check only; use `icacls` to verify ACLs.
- **Log file:** On Unix, log file path is refused if any ancestor is a symlink.

---

## Logging

- Human-readable (default) or JSON (`--json`)
- Levels: quiet, normal, info, debug

```bash
aria_move --json --log-level info
aria_move -d
```

---

## Platform Feature Matrix

| Feature                               | macOS | Linux | Windows |
|---------------------------------------|:-----:|:-----:|:-------:|
| Atomic rename (files/dirs)            | ✅    | ✅    | ✅      |
| Safe copy+rename fallback             | ✅    | ✅    | ✅      |
| --preserve-metadata (perms+mtime)     | ✅    | ✅    | ✅ (mtime partial) |
| --preserve-permissions (mode/ro)      | ✅    | ✅    | ✅ (readonly) |
| Extended attributes (xattrs feature)  | ✅    | ✅    | ❌      |
| Disk space pre-flight (copy fallback) | ✅    | ✅    | ❌      |
| Directory permission validation       | ✅    | ✅    | ⚠️ (readonly only) |
| Symlink ancestor defense (log path)   | ✅    | ✅    | ❌      |
| O_NOFOLLOW log open                   | ✅    | ✅    | ❌      |
| Structured logging (JSON)             | ✅    | ✅    | ✅      |
| Auto-resolution recent window (code)  | ✅    | ✅    | ✅      |

---

## Links

- [CHANGELOG](./CHANGELOG.md) (add this file when you publish releases)
- Issues • Pull Requests (update with your repository links)

## License

MIT

## Contributing

Contributions welcome! Please open an issue or pull request. Ensure all tests pass and code is formatted/linted.