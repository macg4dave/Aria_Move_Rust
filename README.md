# Aria Move

Aria Move is a small companion for [aria2](https://aria2.github.io/), the download manager. Once aria2 finishes a download, it tells Aria Move to move it into your **completed** folder. You can run both together on a Debian, Ubuntu, Fedora or RHEL-family home server, or connect Aria Move to an aria2 installation you already have.



**Quick links:** [Install](#install-on-debian-or-ubuntu) · [Fedora/RHEL](#install-on-fedora-rhel-almalinux-or-rocky-linux) · [Connect a web UI](#connect-a-web-interface) · [Change folders](#use-your-own-download-folders) · [Windows](#windows) · [Fix a problem](#troubleshooting)

## What makes it useful?

- **Automatic:** finished downloads move without you sorting them by hand.
- **Careful with your files:** existing destination files are preserved, and Aria Move refuses unfinished downloads and symbolic links.
- **Works across drives:** it uses a fast rename when possible, or a verified copy when moving between filesystems.
- **Home-server friendly:** the Debian package installs aria2 and sets up a systemd service, including a restricted account.
- **Test before moving:** a dry-run option shows what would happen without touching the files.

**Supported builds:** Linux x86-64 and ARM64 (`.deb` and `.rpm`) and Windows x86-64.

## Install on Debian or Ubuntu

**Recommended for a home server, mini PC or Raspberry Pi running 64-bit Debian/Ubuntu.** You do not need Rust, a compiler or custom startup scripts.

### 1. Download the package

Open the [Releases page](https://github.com/macg4dave/Aria_Move_Rust/releases) and download the `.deb` file for your machine:

| Your machine                                                   | Package ending |
| -------------------------------------------------------------- | -------------- |
| Typical Intel/AMD PC or server                                 | `_amd64.deb`   |
| 64-bit ARM machine, such as a Raspberry Pi running ARM64 Linux | `_arm64.deb`   |

Not sure which one you need? Run:

```bash
dpkg --print-architecture
```

If it prints `amd64`, choose the amd64 package. If it prints `arm64`, choose the arm64 package. If it prints something else, neither of these packages is the right fit.

### 2. Install it

Open a terminal in the folder containing the downloaded `.deb` file. For example:

```bash
cd ~/Downloads
sudo apt install ./aria-move_1.5.1_amd64.deb
```

The package installs aria2 as needed, creates its own service account and configures the automatic completion hook. You do **not** need to write a service file or a shell script.

> **Already running your own aria2 server?** Read [Use an existing aria2 installation](#use-an-existing-aria2-installation) before starting another one. Two instances should not share the same download folder and session.

### 3. Check that it started

```bash
systemctl status aria-move
```

Look for **`active (running)`**. Press **Q** to leave the status screen.

Check the mover configuration:

```bash
sudo -u aria-move aria_move --check-config
```

That's the basic installation done. On the standard package setup, files go here:

| Purpose               | Location                           |
| --------------------- | ---------------------------------- |
| Downloads in progress | `/var/lib/aria-move/incoming`      |
| Finished downloads    | `/var/lib/aria-move/completed`     |
| Aria Move settings    | `/etc/aria_move/config.xml`        |
| aria2 settings        | `/etc/aria_move/aria2.conf`        |
| Move activity log     | `/var/log/aria-move/aria_move.log` |

**Tip:** You can leave these defaults in place to get started and change the storage location later.

## Install on Fedora, RHEL, AlmaLinux or Rocky Linux

The `.rpm` packages set up the same service as the Debian package: a restricted `aria-move` account, `aria-move.service`, authenticated localhost RPC and the completion hook. They need glibc 2.34 or newer (RHEL/AlmaLinux/Rocky 9+ or a current Fedora).

| Your machine                     | Package ending  |
| -------------------------------- | --------------- |
| Typical Intel/AMD PC or server   | `.x86_64.rpm`   |
| 64-bit ARM machine               | `.aarch64.rpm`  |

Run `uname -m` if unsure. On RHEL, AlmaLinux and Rocky Linux, aria2 comes from EPEL, so enable it first (Fedora already has aria2):

```bash
sudo dnf install epel-release        # RHEL-family only
sudo dnf install ./aria-move-1.5.1-1.el9.x86_64.rpm
```

As usual for RPM packages, the service is installed but not started. Start it and enable it at boot:

```bash
sudo systemctl enable --now aria-move
```

Everything else in this guide (folders, web UI, token, commands) is the same as on Debian. To update, install the newer `.rpm` with `sudo dnf install ./FILE.rpm`; your edited settings in `/etc/aria_move` are kept. `sudo dnf remove aria-move` keeps your downloads, token and logs.

## Connect a web interface

A web interface makes it easier to add downloads from your browser. Aria Move does **not** include one, but the bundled aria2 service provides an RPC connection that compatible aria2 clients and web interfaces can use.

The default connection settings are:

| Setting            | Value                            |
| ------------------ | -------------------------------- |
| Server / host      | `localhost` (on the same server) |
| Port               | `6800`                           |
| Protocol           | `http`                           |
| RPC path           | `/jsonrpc`                       |
| Full local address | `http://localhost:6800/jsonrpc`  |
| Authentication     | Secret token                     |

### Get your secret token

On the Debian/Ubuntu server, run:

```bash
sudo cat /var/lib/aria-move/rpc-secret
```

Copy the value into your web interface's **RPC secret**, **token** or similarly named field. The token is generated automatically and persists across normal restarts and reinstalls.

**Important:** Keep this token private. It gives access to the aria2 download manager.

### Connecting from another computer

The bundled service listens only on the server itself by default. This is intentional: `localhost` on your laptop is **not** the same as `localhost` on your server.

For secure remote access, use SSH port forwarding from your other computer:

```bash
ssh -L 6800:127.0.0.1:6800 your-user@your-server
```

Replace `your-user` and `your-server` with your SSH login and the server's hostname or IP address. Leave that SSH connection open, then set your browser-based aria2 client to `http://localhost:6800/jsonrpc` and enter the secret token.

**Tip:** If your own computer already uses port 6800, forward another local port, for example `ssh -L 16800:127.0.0.1:6800 your-user@your-server`, and connect the client to `http://localhost:16800/jsonrpc` instead.

Do not expose the RPC port directly to the internet.

## Try a download

With your preferred aria2 client connected, add a download. aria2 saves it in `incoming` while it is downloading. When it finishes, the completion hook calls Aria Move, which moves the finished file into `completed`.

To look at the folders on your server:

```bash
sudo ls -lah /var/lib/aria-move/incoming
sudo ls -lah /var/lib/aria-move/completed
```

To see recent moves:

```bash
sudo tail -n 30 /var/log/aria-move/aria_move.log
```

To watch the log as new downloads finish, use:

```bash
sudo tail -f /var/log/aria-move/aria_move.log
```

Press **Ctrl+C** to stop following the log.

## Use your own download folders

Want downloads on a larger disk, NAS mount or `/srv/downloads`? Both programs need to agree on the incoming folder. The service account also needs access to it.

For example, suppose you want:

```text
/srv/downloads/
├── incoming/
└── completed/
```

### 1. Stop the service

```bash
sudo systemctl stop aria-move
```

### 2. Create the folders

```bash
sudo mkdir -p /srv/downloads/incoming /srv/downloads/completed
```

If these folders belong only to Aria Move, give its service account ownership:

```bash
sudo chown aria-move:aria-move /srv/downloads/incoming /srv/downloads/completed
```

**Tip:** On a shared disk or NAS, **do not blindly change ownership of the whole drive**. Give the `aria-move` account access to only the directories it needs. Network mounts may need their own permission configuration.

### 3. Tell aria2 where to download

```bash
sudo nano /etc/aria_move/aria2.conf
```

Set the `dir` entry to:

```ini
dir=/srv/downloads/incoming
```

press **Ctrl+O**, **Enter**, then **Ctrl+X** to save and exit.

### 4. Tell Aria Move where to move files

```bash
sudo nano /etc/aria_move/config.xml
```

Change the incoming and completed entries. For example:

```xml
<config>
  <download_base>/srv/downloads/incoming</download_base>
  <completed_base>/srv/downloads/completed</completed_base>
  <log_file>/var/log/aria-move/aria_move.log</log_file>
  <log_level>warn</log_level>
</config>
```

Aria Move writes to two independent logs:

- **System log** – every application warning, error and crash (a failed move,
  a file it cannot read or write, a bad configuration, a log file it cannot
  open), whatever `log_level` says. On Linux and macOS this is syslog
  (`journalctl -t aria_move` on systemd); on Windows it is the Application
  event log, source `aria_move`. The same messages are printed to stderr.
  Successful moves and diagnostics never go here.
- **Log file** (`log_file`) – controlled by `log_level`:

| `log_level` | Log file contents |
| --- | --- |
| `verbose` | Everything: diagnostics, completed moves, warnings and errors |
| `normal` | Completed moves, warnings and errors |
| `warn` (default) | Warnings and errors only |

The old `debug`, `trace`, `info` and `detailed` names are aliases for `verbose`;
`quiet`, `error`, `none` and `warning` are aliases for `warn`. `--json` writes the
log file as JSON lines. Errors that happen before the configuration is loaded
can only reach the system log, because the log file's path is not known yet.

You can keep other valid settings that were already in the file. Use **absolute paths**, not relative paths. Keep the incoming and completed folders separate; don't put one inside the other.

### 5. Allow the service to use those folders

The bundled service has filesystem protections, so changing permissions alone is not enough. Create a systemd override:

```bash
sudo systemctl edit aria-move
```

Paste:

```ini
[Unit]
RequiresMountsFor=/srv/downloads/incoming /srv/downloads/completed

[Service]
ReadWritePaths=/srv/downloads/incoming /srv/downloads/completed
```

Save and exit the editor. `RequiresMountsFor` helps with mounted drives; `ReadWritePaths` lets the protected service write to the new locations. These settings do not grant ordinary Linux file permissions, which must also be correct.

Every `ReadWritePaths` entry must already exist; a missing or misspelled one stops the whole service from starting with `status=226/NAMESPACE`. If `log_file` is outside `/var/log/aria-move`, add the log file's directory too.

### 6. Check and restart

```bash
sudo systemctl daemon-reload
sudo -u aria-move aria_move --check-config
sudo systemctl start aria-move
systemctl status aria-move
```

Try a small download and confirm it appears in your new completed folder.

## Everyday commands

All the following commands apply to the bundled Debian/Ubuntu service.

| What you want                   | Command                                      |
| ------------------------------- | -------------------------------------------- |
| Check whether it is running     | `systemctl status aria-move`                 |
| Restart after changing settings | `sudo systemctl restart aria-move`           |
| View recent service messages    | `journalctl -u aria-move -e`                 |
| Watch service messages live     | `journalctl -u aria-move -f`                 |
| View mover errors               | `journalctl -t aria_move -e`                 |
| Check mover settings            | `sudo -u aria-move aria_move --check-config` |
| Show available options          | `aria_move --help`                           |

**Tip:** If you changed `aria2.conf` or `config.xml`, restart the service to apply the changes.

## Test a move without changing files

You can run Aria Move yourself, without waiting for aria2. Use `--dry-run` to preview a move:

```bash
aria_move --dry-run /path/to/incoming/example.zip
```

Then, if the result is correct:

```bash
aria_move /path/to/incoming/example.zip
```

The example path must refer to an existing item **inside your configured incoming folder**. The user running the command must have permission to access the files and destination.

When running outside the packaged environment, you can supply both folders explicitly:

```bash
aria_move \
  --download-base /path/to/incoming \
  --completed-base /path/to/completed \
  --dry-run /path/to/incoming/example.zip
```

**How folders behave:** For a normal one-file download, Aria Move moves the selected file. For a multi-file aria2 download, place that download in its own top-level folder inside `incoming`; when aria2 supplies its multi-file completion information, Aria Move can move that folder together. It does not simply move every other file in the incoming folder.

## Use an existing aria2 installation

Already have aria2 configured with your own web UI, RPC password, or download directories? You can use Aria Move without running the bundled aria2 service.

On Debian/Ubuntu, install the `.deb` as above, then disable the bundled service so it doesn't compete with your existing installation:

```bash
sudo systemctl disable --now aria-move
```

Find the configuration file used by your existing aria2 instance and add or update this line:

```ini
on-download-complete=/usr/bin/aria_move
```

Next, edit Aria Move's configuration:

```bash
sudo nano /etc/aria_move/config.xml
```

Make `download_base` match your existing aria2 download directory, and set `completed_base` to your intended finished-download folder. If you enable file logging, choose a log file location that the **existing aria2 service's account** can write to.

Check the configuration and restart **your existing aria2 service**, not the bundled `aria-move` one.

**Important:** Only one aria2 instance should manage a given incoming folder/session. Aria Move runs when the completion hook is called; it does not need its own continuously running mover daemon.

## Windows

Windows users can run the Aria Move executable alongside an existing aria2 installation. The Windows package does **not** install a Windows service or a download manager.

1. Download the Windows x86-64 archive from [Releases](https://github.com/macg4dave/Aria_Move_Rust/releases).
2. Extract it into a permanent folder, for example `C:\Tools\AriaMove`.
3. Open PowerShell and navigate to that folder.
4. Create configuration using the example below.

```powershell
cd C:\Tools\AriaMove
.\aria_move.exe --init --download-base "C:\Downloads\incoming" --completed-base "C:\Downloads\completed"
```

Create the corresponding folders if they do not exist. `--init` creates the config but will not overwrite an existing one. To see which config is being used:

```powershell
.\aria_move.exe --print-config
```

Check it:

```powershell
.\aria_move.exe --check-config
```

Preview moving a finished file:

```powershell
.\aria_move.exe --dry-run "C:\Downloads\incoming\example.zip"
```

In your existing aria2 configuration, point the completion hook at the executable using its **actual full path**:

```ini
on-download-complete=C:\Tools\AriaMove\aria_move.exe
```

Restart aria2 after changing its configuration. aria2 invokes Aria Move once per completed download. No administrator rights are normally required to run the mover under your user account.

**Windows errors** go to the Application event log under the source `aria_move`. Until that source is registered, Event Viewer shows each message under "the following information was included with the event". To register it once, run in an elevated **Windows PowerShell 5.1** prompt:

```powershell
New-EventLog -LogName Application -Source aria_move
```

**Windows configuration:** The default user config is at `%APPDATA%\aria_move\config.xml`. If a file already exists, `--init` will refuse to replace it. Edit the existing config or use `--config` to select another config file.

## Other Linux installations / build from source

If there isn't a package for your operating system, advanced users can build the executable from source. You need Rust 1.99 or later and a working native C linker.

```bash
git clone https://github.com/macg4dave/Aria_Move_Rust.git
cd Aria_Move_Rust
cargo build --release --locked
```

The compiled Linux executable is in `target/release/aria_move`. Set up a standalone configuration with your own absolute folder paths:

```bash
./target/release/aria_move --init \
  --download-base /home/your-user/Downloads/incoming \
  --completed-base /home/your-user/Downloads/completed
```

Run `./target/release/aria_move --check-config`, then configure your existing aria2 to use the full path to that executable as its `on-download-complete` hook. A source build does **not** automatically set up the packaged systemd service or RPC server.

## Updates and removal

### Update on Debian/Ubuntu

Download the newer `.deb` from [Releases](https://github.com/macg4dave/Aria_Move_Rust/releases) and install it over the existing package:

```bash
sudo apt install ./aria-move_VERSION_ARCH.deb
```

Replace `VERSION_ARCH` with the actual filename. Local package configuration is handled by Debian's normal package-upgrade rules; read any prompts before replacing a changed config file.

### Uninstall

Remove the package:

```bash
sudo apt remove aria-move
```

Or remove the package and its packaged configuration:

```bash
sudo apt purge aria-move
```

**Your downloads are not deleted** by package removal or purge. The installer also retains state, logs and the service account to avoid losing data. Only delete retained directories manually after checking they contain nothing you still need.

## Troubleshooting

### The service isn't running

```bash
systemctl status aria-move
sudo journalctl -u aria-move -n 50 --no-pager
sudo journalctl -t aria_move -n 50 --no-pager
sudo -u aria-move aria_move --check-config
```

Read the first error message. A missing folder, incorrect configuration or another process using aria2's port may prevent startup.

### The download finishes, but doesn't move

Check the move log:

```bash
sudo tail -n 50 /var/log/aria-move/aria_move.log
```

Make sure aria2 has the `on-download-complete` setting pointing to the correct executable, and that `download_base` matches aria2's incoming directory. Aria Move intentionally refuses files that still have aria2 control files indicating an incomplete download.

### Permission denied

Check whether the service account can access the actual download storage. On Debian/Ubuntu the bundled account is called `aria-move`. If you changed folders, follow **all** the steps under [Use your own download folders](#use-your-own-download-folders), including the systemd override. For a NAS, check mount permissions as well.

Avoid using `chmod -R 777` as a shortcut.

### My web UI cannot connect

First check the service:

```bash
systemctl status aria-move
```

Then check the client's host, port, RPC path and secret token. The bundled RPC server uses **localhost:6800**, not a publicly listening address. If your web UI runs elsewhere, use the [SSH forwarding instructions](#connecting-from-another-computer). If port 6800 is already in use, change `rpc-listen-port` in `/etc/aria_move/aria2.conf` and restart the service.

### I see an HTTP 400 error at `/jsonrpc`

That can simply mean you opened the JSON-RPC endpoint as if it were a normal web page. It expects a properly formatted RPC request; it **does not display a web interface**. Connect with a compatible aria2 client instead.

### A destination file already exists

Aria Move preserves existing files rather than replacing them. Check the completed folder and resolve the name conflict before retrying.

### Network drive or file lock errors

Some network filesystems do not support the locking behaviour Aria Move expects. The `--disable-locks` option exists, but removes protection against concurrent moves. Use it only when you understand the risk and cannot fix the filesystem behaviour.

### Still stuck?

Open an [issue on GitHub](https://github.com/macg4dave/Aria_Move_Rust/issues) and include your operating system, how you installed Aria Move, the exact error, and relevant **redacted** log lines. **Never include your RPC token or private credentials.**

## Useful reference

| Option                                              | What it does                                                         |
| --------------------------------------------------- | -------------------------------------------------------------------- |
| `--help`                                            | Show all available commands and options                              |
| `--version`                                         | Show installed version                                               |
| `--print-config`                                    | Show the active configuration path                                   |
| `--check-config`                                    | Check the settings without moving files                              |
| `--init --download-base PATH --completed-base PATH` | Create a standalone configuration (won't overwrite an existing file) |
| `--dry-run`                                         | Preview a move                                                       |
| `--source-path PATH`                                | Explicitly select an item to move                                    |
| `--config PATH`                                     | Use a particular XML configuration file                              |
| `--debug`                                           | Enable detailed logging                                              |
| `--preserve-metadata`                               | Preserve more file metadata when copying across filesystems          |

More technical information is available in [installation notes](docs/INSTALLATION.md), [architecture](docs/ARCHITECTURE.md) and [release/build notes](docs/RELEASING.md).

## Licence

Aria Move is released under the [MIT Licence](LICENSE).
