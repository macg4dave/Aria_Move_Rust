# aria_move (Rust)

[![Release](https://img.shields.io/github/v/release/macg4dave/Aria_Move_Rust?display_name=tag&sort=semver)](https://github.com/macg4dave/Aria_Move_Rust/releases)
[![Downloads](https://img.shields.io/github/downloads/macg4dave/Aria_Move_Rust/total)](https://github.com/macg4dave/Aria_Move_Rust/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](#license)
[![CI](https://github.com/macg4dave/Aria_Move_Rust/actions/workflows/ci.yml/badge.svg)](https://github.com/macg4dave/Aria_Move_Rust/actions/workflows/ci.yml)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-orange.svg)](#usage)

---

## Table of Contents

- [Requirements & Build Tools](#requirements--build-tools)
- [Windows 11 + VS Code Setup](#windows-11--vs-code-setup)
 - [Fedora (Workstation/Server) + VS Code Setup](#fedora-workstationserver--vs-code-setup)
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
- [Releases](#releases)

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

## Releases

This repository automates releases with:

- Release PRs and changelogs powered by release-please
- Cross-platform binaries built and uploaded by cargo-dist

Typical flow:

1. Merge PRs with Conventional Commit-style titles (e.g., `feat: add resume support`).
2. A Release PR is opened automatically; review and merge it when ready.
3. The GitHub Release is created; the Release workflow builds and uploads binaries for Linux, macOS, and Windows.

Manual runs:

- To refresh the Release PR manually, run the “Release Please” workflow from the Actions tab.
- To build artifacts for an existing tag (re-run), dispatch the “Release Artifacts” workflow and provide the tag.

Local dry-run:

```
./scripts/release-dry-run.sh
```

Artifacts will appear in `target/dist` (no upload).

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

## Windows 11 + VS Code Setup

This section gives end-to-end steps to build, run, test, and debug `aria_move` on Windows 11 using Visual Studio Code and the MSVC toolchain. Follow it if you're new to Rust or want a reproducible dev setup.

### 1. Overview

You will install: Rust (via rustup), VS Code, C++ Build Tools (MSVC), recommended VS Code extensions, then build and test the project. No extra OpenSSL / pkg-config dependencies are required on Windows for current `Cargo.toml`.

### 2. Prerequisites

- Windows 11 (x64) with latest updates.
- Administrator rights (for installing build tools).
- Stable Internet connection.

### 3. Install MSVC Build Tools

If you do NOT already have Visual Studio installed:

1. Download "Build Tools for Visual Studio" from: https://visualstudio.microsoft.com/downloads/
2. Run the installer, select: "Desktop development with C++" workload.
3. Finish install (accept defaults). This provides the MSVC linker & libraries Rust needs.

You do NOT need the full Visual Studio IDE unless you want it; the build tools are enough.

### 4. Install Rust (rustup)

Open an elevated PowerShell (Win+X then choose Windows Terminal (Admin)) and run:

```powershell
irm https://win.rustup.rs -UseBasicParsing | Invoke-Expression
rustup default stable
rustup component add rustfmt clippy
rustc --version
cargo --version
```

If prompted for toolchain choice, pick "1) Default installation" (MSVC stable).

### 5. Install Visual Studio Code & Extensions

Download VS Code: https://code.visualstudio.com/

Recommended extensions (search in Extensions sidebar / Ctrl+Shift+X):

- Rust Analyzer (rust-lang.rust-analyzer)
- CodeLLDB (vadimcn.vscode-lldb) — optional for cross-platform debugging examples (MSVC debugging works via built-in C++ or use WinDbg style; for Rust typical workflows CodeLLDB is fine, though it uses LLDB backend)
- Even Better TOML (tamasfe.even-better-toml)
- Error Lens (usernamehw.errorlens) — surface errors inline
- EditorConfig (EditorConfig.EditorConfig) — if you standardize formatting across editors

### 6. Clone the Repository

In PowerShell:

```powershell
git clone https://github.com/macg4dave/Aria_Move_Rust.git
cd Aria_Move_Rust
code .
```

VS Code will open the workspace; Rust Analyzer begins indexing.

### 7. Verify Environment

Inside the VS Code integrated terminal (PowerShell):

```powershell
rustup show
rustup toolchain list
cargo check
```

You should see the stable-x86_64-pc-windows-msvc toolchain and `cargo check` succeed.

### 8. Build & Run

Release build (optimized):

```powershell
cargo build --release
.
dir target\release\aria_move.exe
```

Run help:

```powershell
cargo run -- --help
```

Run with a dummy path (dry-run recommended during testing):

```powershell
cargo run -- --dry-run 1234 1 C:\temp\example.file
```

### 9. Tests & Quality Gates

Run unit & integration tests:

```powershell
cargo test
```

Lint (Clippy) and format:

```powershell
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

If formatting check fails, run:

```powershell
cargo fmt --all
```

### 10. Optional VS Code Tasks

You can add a `.vscode/tasks.json` for one-click builds. Create the folder/file and add:

```jsonc
{
    "version": "2.0.0",
    "tasks": [
        { "label": "Cargo Build", "type": "shell", "command": "cargo", "args": ["build"], "group": "build" },
        { "label": "Cargo Test", "type": "shell", "command": "cargo", "args": ["test"], "group": "test" },
        { "label": "Cargo Clippy", "type": "shell", "command": "cargo", "args": ["clippy", "--all-targets", "--all-features"], "problemMatcher": [] }
    ]
}
```

Invoke with: Terminal > Run Task... (or Ctrl+Shift+B for the first build task).

### 11. Debugging in VS Code

For simple argument debugging create `.vscode/launch.json`:

```jsonc
{
    "version": "0.2.0",
    "configurations": [
        {
            "name": "Debug aria_move (args)",
            "type": "lldb",            // Use "cppvsdbg" if you prefer MSVC debugger
            "request": "launch",
            "program": "${workspaceFolder}/target/debug/aria_move.exe",
            "args": ["--dry-run", "abcd", "1", "C:/temp/example.bin"],
            "cwd": "${workspaceFolder}",
            "environment": [],
            "console": "integratedTerminal"
        }
    ]
}
```

Start by first building a debug binary:

```powershell
cargo build
```

Then press F5.

### 12. Feature Flags

Current features (`Cargo.toml`): `test-helpers`, `xattrs`.

- `xattrs` is not supported on Windows (table shows ❌). Avoid enabling it here.
- Run tests with features if desired (non-Windows ones will skip / fail accordingly):

```powershell
cargo test --features test-helpers
```

### 13. Configuration File Location

Default path: `%APPDATA%\aria_move\config.xml`.

Open it quickly:

```powershell
code $env:APPDATA\aria_move\config.xml
```

Override config path for a session:

```powershell
$env:ARIA_MOVE_CONFIG = "C:\custom\config\aria_move.xml"
cargo run -- --print-config
```

### 14. Common Issues & Fixes

| Symptom | Cause | Fix |
|---------|-------|-----|
| `link.exe` not found | MSVC build tools missing | Install Build Tools (section 3) |
| Slow first build | Rust crate compilation | Subsequent builds are incremental |
| ExecutionPolicy blocks rustup | Restricted PowerShell policy | Run elevated: `Set-ExecutionPolicy RemoteSigned -Scope CurrentUser` |
| Antivirus slows build | Real-time scanning of `target/` | Add a safe exclusion for the project dir |
| Path length errors | Long path disabled | Enable long paths: Group Policy or `reg add HKLM\SYSTEM\CurrentControlSet\Control\FileSystem /v LongPathsEnabled /t REG_DWORD /d 1 /f` |
| Rust Analyzer stuck | Workspace indexing | Run `cargo check`; ensure no modal prompts hidden |

### 15. Troubleshooting Commands

```powershell
# Show full build invocation and environment
cargo build -vv

# Clean out old artifacts
cargo clean

# Confirm the active toolchain host triple
rustc -vV

# List outdated dependencies (install cargo-edit first if needed)
cargo install cargo-outdated
cargo outdated
```

### 16. Updating Toolchain

```powershell
rustup update
cargo update    # update Cargo.lock versions within constraints
```

### 17. Automating Checks (Pre-Push)

Optional script `scripts\pre_push.ps1` (create if desired):

```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Run manually before pushing changes:

```powershell
pwsh scripts/pre_push.ps1
```

---

This completes a full Windows 11 + VS Code environment setup. You can now modify code in VS Code, use Rust Analyzer for inline diagnostics, run tasks, and debug with your chosen adapter.

---

## Fedora (Workstation/Server) + VS Code Setup

End-to-end steps to build, test, and debug `aria_move` on Fedora using VS Code. Tested on Fedora 40/41 Workstation, but applies to recent Fedora or RHEL/CentOS Stream derivatives (replace `dnf` with `yum` where needed).

### 1. Overview

You will install: system build toolchain, Rust (rustup), Visual Studio Code, recommended extensions, then run builds/tests. OpenSSL dev headers and pkg-config are required by crates; Fedora provides them via `openssl-devel` and `pkgconf-pkg-config`.

### 2. Prerequisites

- Fedora (Workstation or Server) with latest updates
- User with sudo rights
- Internet access

### 3. System Packages

Install development tools and native deps:

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y git curl pkgconf-pkg-config openssl-devel
```

Optional extras:

```bash
sudo dnf install -y lldb # if you prefer system LLDB for debugging
```

### 4. Install Rust (rustup)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
rustup component add rustfmt clippy
rustc --version
cargo --version
```

### 5. Install Visual Studio Code

Add Microsoft repository (one-time):

```bash
sudo rpm --import https://packages.microsoft.com/keys/microsoft.asc
sudo sh -c 'echo -e "[code]\nname=Visual Studio Code\nbaseurl=https://packages.microsoft.com/yumrepos/vscode\nenabled=1\ngpgcheck=1\ngpgkey=https://packages.microsoft.com/keys/microsoft.asc" > /etc/yum.repos.d/vscode.repo'
sudo dnf check-update || true
sudo dnf install -y code
```

### 6. Recommended VS Code Extensions

Search / install:

- Rust Analyzer (`rust-lang.rust-analyzer`)
- CodeLLDB (`vadimcn.vscode-lldb`)
- Even Better TOML (`tamasfe.even-better-toml`)
- Error Lens (`usernamehw.errorlens`)
- EditorConfig (`EditorConfig.EditorConfig`)

`./.vscode/extensions.json` lists these as recommendations.

### 7. Clone Repository

```bash
git clone https://github.com/macg4dave/Aria_Move_Rust.git
cd Aria_Move_Rust
code .
```

Rust Analyzer will index the workspace automatically.

### 8. Verify Toolchain

```bash
rustup show
rustup toolchain list
cargo check
```

Expect `stable-x86_64-unknown-linux-gnu` and a successful `cargo check`.

### 9. Build & Run

Debug build:

```bash
cargo build
./target/debug/aria_move --help
```

Release build:

```bash
cargo build --release
./target/release/aria_move --dry-run example-task 1 /tmp/example.file
```

### 10. Run Tests & Quality Gates

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

If formatting fails:

```bash
cargo fmt --all
```

### 11. VS Code Tasks

Convenient tasks are provided in `./.vscode/tasks.json` (Build Debug, Build Release, Test, Clippy, Fmt Check). Run via: Terminal > Run Task…

### 12. Debugging (CodeLLDB)

Launch configs in `./.vscode/launch.json` include a direct binary run with sample arguments. Build first:

```bash
cargo build
```

Then press F5 and select `Debug aria_move (Linux/Fedora)`.

### 13. Features

Optional features in `Cargo.toml`:

- `xattrs` (Linux/macOS only)
- `test-helpers` (adds tempfile for tests)

Enable when testing:

```bash
cargo test --features xattrs,test-helpers
```

### 14. Configuration Path

Default (Linux): `~/.config/aria_move/config.xml`

Override for a single run:

```bash
ARIA_MOVE_CONFIG=/custom/path/config.xml ./target/debug/aria_move --print-config
```

### 15. Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| OpenSSL not found | Missing dev headers | Install `openssl-devel` |
| pkg-config errors | `pkgconf-pkg-config` absent | Install package listed in step 3 |
| Clippy warns | Style / lint issues | Address, re-run `cargo clippy -- -D warnings` |
| Slow first build | Crate compilation | Subsequent incremental builds faster |
| Rust Analyzer lag | Indexing / no `cargo check` yet | Run `cargo check` manually |

### 16. Updating

```bash
rustup update
cargo update
```

### 17. Pre-Push Local Check

Optional script (create `scripts/pre_push.sh`):

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Run manually:

```bash
./scripts/pre_push.sh
```

---

This completes a full Fedora + VS Code environment setup. You can now develop, test, lint, and debug `aria_move` efficiently on Linux.

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
# Move a specific path (typical aria2 hook) - always provide the explicit path
aria_move 7b3f1234 1 /path/to/incoming/file.iso

# Override bases
aria_move --download-base /data/incoming --completed-base /data/completed

# Dry run with JSON logs
aria_move --dry-run --json

# Show config location
aria_move --print-config

# Move a directory by bare name (from download_base)
# If your config has download_base=/home/user/incoming and a folder named "New folder" exists there:
aria_move 'New folder'

# Move a directory by full path
aria_move /home/user/incoming/New\ folder
```

---

### --help snapshot (example)

```
aria_move 1.0.0
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
- **Source paths:** Symlink sources (file or directory) are refused. Even if a symlink ultimately points to a regular file or directory, aria_move aborts rather than follow it. This prevents time‑of‑check/time‑of‑use attacks and path “tricks” in hooks. Bare filename resolution under `download_base` also rejects symlinks. Race tests simulate a concurrent symlink appearing at a destination; if detected, the outcome is marked inconclusive rather than followed.

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
| Auto-resolution (newest file)         | ❌    | ❌    | ❌      |

---

## Links

- [CHANGELOG](./CHANGELOG.md) (add this file when you publish releases)
- Issues • Pull Requests (update with your repository links)

## License

MIT

## Contributing

Contributions welcome! Please open an issue or pull request. Ensure all tests pass and code is formatted/linted.