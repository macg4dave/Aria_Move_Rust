# aria_move 1.5

Move completed aria2 downloads safely from an incoming directory to a completed directory. Uses atomic renames where possible and verified copies across filesystems. Existing files are preserved; unfinished downloads and symlinks are refused.

Supported targets: Linux x86_64 (amd64), Linux ARM64 (arm64), Windows x86_64.

## Install on Debian or Ubuntu

Download your architecture's package from [Releases](https://github.com/macg4dave/Aria_Move_Rust/releases), then:

```bash
sudo apt install ./aria-move_1.5.0_amd64.deb
systemctl status aria-move
```

The package installs aria2, creates a restricted service account and enables a managed aria2 service. No wrapper scripts or hand-written units are needed.

- Incoming: `/var/lib/aria-move/incoming`
- Completed: `/var/lib/aria-move/completed`
- Configuration: `/etc/aria_move/config.xml` and `/etc/aria_move/aria2.conf`
- Move logs: `/var/log/aria-move/aria_move.log`

The service accepts authenticated RPC connections on localhost port 6800. Point your aria2 client at `http://localhost:6800/jsonrpc`; retrieve its automatically generated token with `sudo cat /var/lib/aria-move/rpc-secret`. Keep that token private. See [installation details](docs/INSTALLATION.md) for a command-line download example, custom storage and existing aria2 installations.

## First-run configuration and usage

Debian defaults need no path setup. Check configuration with `aria_move --check-config`.

Standalone users can create configuration without editing XML:

```bash
aria_move --init --download-base /absolute/incoming --completed-base /absolute/completed
aria_move --dry-run /absolute/incoming/movie.mkv
aria_move /absolute/incoming/movie.mkv
```

On Windows extract the archive to a permanent folder and run `aria_move.exe` there with Windows paths, such as `C:\Downloads\incoming`. Administrator rights are unnecessary. Set aria2's completion hook directly to the executable. It runs once per completion; it is not a Windows service.

`--init` never overwrites configuration. Standalone defaults are `~/.config/aria_move/config.xml` on Linux and `%APPDATA%\aria_move\config.xml` on Windows. An installed system config takes precedence on Linux. `--config PATH` overrides `ARIA_MOVE_CONFIG` and defaults. `--print-config` shows the selected path; `--help` lists all options.

## Service management

```bash
sudo systemctl stop aria-move
sudo systemctl start aria-move
sudo systemctl restart aria-move
systemctl status aria-move
journalctl -u aria-move -e
```

Install a newer package with `sudo apt install ./aria-move_VERSION_ARCH.deb`. Local configuration survives upgrades.

```bash
sudo apt remove aria-move
# Also remove package configuration:
sudo apt purge aria-move
```

Downloads, logs and the service account remain after removal and purge to protect your files. Reinstallation reuses them.

## Troubleshooting

- Service failed: inspect `journalctl -u aria-move -e` and run `sudo -u aria-move aria_move --check-config`.
- Permission denied: check the service account's storage and log access. Custom paths also need a systemd writable-path override.
- Nothing moved: check the completion hook, move log and adjacent .aria2 control files.
- Network filesystem lock errors: `--disable-locks` removes concurrent-move protection; use only when necessary.
- Windows access denied: close programs holding the source and retry.

## Development

Rust 1.99+ and a native C linker are required.

```bash
cargo build --release --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
```

See [packaging](docs/RELEASING.md), [validation](docs/VALIDATION.md), [installer review](docs/INSTALLER_REVIEW.md) and [architecture](docs/ARCHITECTURE.md). [MIT license](LICENSE).
