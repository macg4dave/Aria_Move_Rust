# aria_move

[![Release](https://img.shields.io/github/v/release/macg4dave/Aria_Move_Rust?display_name=tag&sort=semver)](https://github.com/macg4dave/Aria_Move_Rust/releases)
[![Downloads](https://img.shields.io/github/downloads/macg4dave/Aria_Move_Rust/total)](https://github.com/macg4dave/Aria_Move_Rust/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](#license)
[![CI](https://github.com/macg4dave/Aria_Move_Rust/actions/workflows/ci.yml/badge.svg)](https://github.com/macg4dave/Aria_Move_Rust/actions/workflows/ci.yml)

**Move completed downloads safely and effortlessly.**

A robust, cross-platform tool that relocates finished downloads from a staging area to a completed directory — designed for aria2 hooks but works standalone. Fast atomic renames when possible, reliable copy-and-cleanup fallback across filesystems, with security checks baked in.

---

## Features

- **Zero-config start** — creates a secure template config on first run
- **Atomic operations** — instant rename when source and dest are on same filesystem
- **Cross-filesystem fallback** — reliable copy+verify+cleanup when rename isn't possible
- **Security first** — symlink defense, permission validation, disk space checks (Unix)
- **Dry-run mode** — preview actions without touching files
- **Flexible logging** — human-readable or JSON, multiple log levels
- **Cross-platform** — Linux (x86_64, aarch64), macOS (universal2), Windows (x86_64)

---

## Quick start

### Download

Pick your platform from the [latest release](https://github.com/macg4dave/Aria_Move_Rust/releases/latest):

- **macOS** (universal2: Apple Silicon + Intel)
- **Linux** (x86_64 or aarch64)
- **Windows** (x86_64)

### Install

1. Extract the archive
2. Move the binary to a directory on your PATH:
   - **Linux/macOS**: `/usr/local/bin` or `~/.local/bin`
   - **Windows**: any folder in PATH (e.g., `C:\Tools`)
3. Make executable (Linux/macOS): `chmod +x aria_move`

### Verify

```bash
aria_move --version
```

### Basic usage

```bash
# Dry-run first (safe)
aria_move --dry-run /path/to/downloads/MyFolder

# Actually move it
# Debug mode
aria_move --log-level debug /path/to/file.iso
```

---

## Integration with aria2

Point aria2's `on-download-complete` hook to a tiny wrapper script:

### Unix/Linux/macOS

Create `/usr/local/bin/aria_move_hook.sh`:

```bash
#!/usr/bin/env bash
exec /usr/local/bin/aria_move "$1" "$2" "$3"
```

Make executable: `chmod +x /usr/local/bin/aria_move_hook.sh`

Add to `aria2.conf`:

```ini
on-download-complete=/usr/local/bin/aria_move_hook.sh
```

### Windows (PowerShell wrapper)

Create `C:\Tools\aria_move_hook.ps1` (PowerShell):

```powershell
# C:\Tools\aria_move_hook.ps1
param($gid, $numFiles, $sourcePath)

# Optional: point to a system-wide config (create as Administrator)
$env:ARIA_MOVE_CONFIG = 'C:\ProgramData\aria_move\config.xml'

# Invoke the aria_move executable with the arguments passed by aria2
& 'C:\Tools\aria_move.exe' $gid $numFiles $sourcePath
```

If aria2 expects a batch/script path, create a simple shim `C:\Tools\aria_move_hook.bat` that invokes PowerShell:

```bat
@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Tools\aria_move_hook.ps1" %1 %2 %3
```

Add to `aria2.conf` (use the .bat shim or point directly to the .ps1):

```ini
on-download-complete=C:\Tools\aria_move_hook.bat
```

### Running under systemd (non-interactive first run)

If `aria_move` is launched only via a systemd service user (e.g. `aria2`) the automatic first-run template may not appear because you never invoke the binary interactively as that user. Pre-create a config in a root-managed path and point the wrapper to it.

1. Create config directory and file:
   ```bash
   sudo mkdir -p /etc/aria_move
   sudo tee /etc/aria_move/config.xml > /dev/null <<'EOF'
<config>
  <download_base>/data/incoming</download_base>
  <completed_base>/data/completed</completed_base>
  <log_level>normal</log_level>
  <log_file>/var/log/aria_move/aria_move.log</log_file>
</config>
EOF
   sudo mkdir -p /var/log/aria_move
   sudo chown -R aria2:aria2 /etc/aria_move /var/log/aria_move /data/incoming /data/completed
   ```
2. Modify wrapper to export the config path before exec:
   ```bash
   # /usr/local/bin/aria_move_hook.sh
   #!/usr/bin/env bash
   export ARIA_MOVE_CONFIG=/etc/aria_move/config.xml
   exec /usr/local/bin/aria_move "$1" "$2" "$3"
   ```
3. Ensure `aria2.conf` uses the wrapper:
   ```ini
   on-download-complete=/usr/local/bin/aria_move_hook.sh
   ```
4. Validate as service user:
   ```bash
   sudo -u aria2 ARIA_MOVE_CONFIG=/etc/aria_move/config.xml /usr/local/bin/aria_move --print-config
   ```

There is no CLI flag for a config path; the environment variable is the supported override. The service user must have read access to the config and write access (if logging to file). If unreadable, defaults are used and moves may be refused due to missing base directories.

```

Add to `aria2.conf`:

```ini
on-download-complete=C:\Tools\aria_move_hook.bat
```

**Important**: Use absolute paths. Test the wrapper manually before enabling in aria2.

---

## Running aria2c under systemd (Linux)

Run aria2 as a service for reliability and auto-start on boot.

### 1) Create configuration and state

Paths used below (customize to your environment):

- Config: `/etc/aria2/aria2.conf`
- Session file: `/var/lib/aria2/aria2.session`
- Logs: `/var/log/aria2/aria2.log`
- Download dir (staging): `/data/incoming`
- Completed dir (final): `/data/completed`

Create dirs and an empty session file:

```bash
sudo mkdir -p /etc/aria2 /var/lib/aria2 /var/log/aria2 /data/incoming /data/completed
sudo touch /var/lib/aria2/aria2.session
```

Minimal `/etc/aria2/aria2.conf`:

```ini
dir=/data/incoming
continue=true

# Session persistence
save-session=/var/lib/aria2/aria2.session
input-file=/var/lib/aria2/aria2.session
save-session-interval=60

# RPC (optional for remote control)
enable-rpc=true
rpc-listen-all=false
rpc-secret=change_me

# Hook: call aria_move via wrapper
on-download-complete=/usr/local/bin/aria_move_hook.sh

# Logging
log=/var/log/aria2/aria2.log
log-level=notice
```

Ensure your wrapper exists and is executable at `/usr/local/bin/aria_move_hook.sh`.

### 2) Create systemd unit

`/etc/systemd/system/aria2c.service`:

```ini
[Unit]
Description=Aria2c download manager
Requires=network.target
After=dhcpcd.service

[Service]
Type=simple
User=aria2
Group=aria2
RemainAfterExit=yes
ExecStart=/usr/bin/aria2c --console-log-level=warn --conf-path=/etc/aria2/aria.conf
ExecReload=/usr/bin/kill -HUP $MAINPID
RestartSec=1min
Restart=on-failure

[Install]
WantedBy=multi-user.target

```

Create the `aria2` user/group and set ownership:

```bash
sudo useradd --system --create-home --home-dir /var/lib/aria2 --shell /usr/sbin/nologin aria2 || true
sudo chown -R aria2:aria2 /var/lib/aria2 /var/log/aria2 /data/incoming /data/completed
sudo chown root:root /etc/aria2
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now aria2c
sudo systemctl status aria2c --no-pager
```

Tail logs:

```bash
journalctl -u aria2c -f
```

Notes:

- The hook path in `aria2.conf` must be absolute and executable.
- Ensure the service user has read/write permissions to the download and completed directories.
- If you customize directories, update both `aria2.conf` and your `aria_move` configuration accordingly.

---

## Configuration

On first run (without a config file), aria_move creates a template at:

- **macOS**: `~/Library/Application Support/aria_move/config.xml`
- **Linux**: `~/.config/aria_move/config.xml`
- **Windows**: `%APPDATA%\aria_move\config.xml`

Edit the file to set your `download_base` and `completed_base` directories, then re-run.

**Override location**: set `ARIA_MOVE_CONFIG` environment variable:

```bash
export ARIA_MOVE_CONFIG=/custom/path/config.xml
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| **Permission denied** (Linux/macOS) | Run `chmod +x aria_move` and ensure install location is on PATH |
| **macOS Gatekeeper block** | Run `xattr -d com.apple.quarantine ./aria_move` or right-click → Open |
| **"Refusing to use log path with symlink"** (Unix) | Choose a log directory without symlinks in its path |
| **"Not enough free space"** | Free space check happens before cross-device copy; ensure destination has room |
| **Windows "Access denied"** | Close any programs viewing the file; retry |
| **Need more logs** | Use `--log-level debug` or `--json` |

---

## Command reference

```
aria_move [OPTIONS] [TASK_ID] [NUM_FILES] [SOURCE_PATH]
```

### Common options

| Flag | Description |
|------|-------------|
| `--download-base <PATH>` | Override download base directory |
| `--completed-base <PATH>` | Override completed base directory |
| `--dry-run` | Show what would happen without modifying files |
| `--log-level <LEVEL>` | Set log level: quiet, normal, info, debug |
| `-d, --debug` | Shortcut for `--log-level debug` |
| `--json` | Output logs in JSON format |
| `--preserve-metadata` | Preserve permissions, timestamps, xattrs (slower) |
| `--preserve-permissions` | Preserve only permissions (faster) |
| `--print-config` | Show config file path and exit |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

---

## Platform feature matrix

| Feature | macOS | Linux | Windows |
|---------|:-----:|:-----:|:-------:|
| Atomic rename | ✅ | ✅ | ✅ |
| Copy+cleanup fallback | ✅ | ✅ | ✅ |
| Preserve metadata | ✅ | ✅ | ⚠️ |
| Extended attributes (xattrs) | ✅ | ✅ | ❌ |
| Disk space check | ✅ | ✅ | ❌ |
| Symlink defense | ✅ | ✅ | ❌ |

---

## Building from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- Git
- C toolchain + pkg-config (Linux/macOS)
- OpenSSL dev headers (Linux: `libssl-dev`, Fedora: `openssl-devel`)

### Build steps

```bash
git clone https://github.com/macg4dave/Aria_Move_Rust.git
cd Aria_Move_Rust
cargo build --release
# Binary at: target/release/aria_move
```

### Install from source

```bash
cargo install --path .
```

### Quality checks

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

---

## Development setup

### Quick setup by platform

<details>
<summary><b>macOS</b></summary>

```bash
xcode-select --install
brew install pkg-config openssl@3
export OPENSSL_DIR="$(brew --prefix openssl@3)"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup component add rustfmt clippy
```

</details>

<details>
<summary><b>Ubuntu/Debian</b></summary>

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup component add rustfmt clippy
```

</details>

<details>
<summary><b>Fedora/RHEL</b></summary>

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config openssl-devel git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup component add rustfmt clippy
```

</details>

<details>
<summary><b>Windows (MSVC)</b></summary>

1. Install [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) with "Desktop development with C++"
2. Install Rust:

```powershell
irm https://win.rustup.rs -UseBasicParsing | iex
rustup component add rustfmt clippy
```

</details>

### VS Code setup

Recommended extensions:

- `rust-lang.rust-analyzer`
- `vadimcn.vscode-lldb`
- `tamasfe.even-better-toml`

The repository includes `.vscode/tasks.json` for build/test/clippy tasks.

---

## Release process

Releases are automated via:

- **release-please** — creates release PRs with changelogs from conventional commits
- **cargo-dist** — builds cross-platform binaries and uploads to GitHub Releases

### Typical flow

1. Merge PRs with conventional commit titles (`feat:`, `fix:`, `chore:`, etc.)
2. Release Please opens a release PR automatically
3. Review and merge the release PR
4. GitHub Release is created with artifacts for all platforms

### Manual artifact rebuild

To rebuild artifacts for an existing tag:

1. Go to **Actions** → **Release Artifacts**
2. Click **Run workflow**
3. Enter the tag name (e.g., `v1.0.0`)

---

## License

[MIT](LICENSE)

---

## Contributing

Contributions welcome! Please:

- Use conventional commit format (`feat:`, `fix:`, `docs:`, etc.)
- Run `cargo fmt --all` and `cargo clippy --all-targets`
- Ensure `cargo test` passes
- Open an issue or PR with your changes

---

**Questions?** Open an [issue](https://github.com/macg4dave/Aria_Move_Rust/issues) or check the [discussions](https://github.com/macg4dave/Aria_Move_Rust/discussions).
