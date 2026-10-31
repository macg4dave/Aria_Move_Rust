# Installation details

Debian owns the binary, systemd unit and two dpkg conffiles in /etc/aria_move.
The RPM installs the same files (launcher in /usr/libexec/aria-move, configs as
`%config(noreplace)`), but leaves the service disabled until
`sudo systemctl enable --now aria-move`; aria2 comes from EPEL on RHEL-family systems.
debhelper generates enable/start, upgrade restart, stop/removal and daemon-reload
actions. Installation never prompts for application setup. aria2 invokes the
mover directly with its three completion arguments; no scanning daemon is used.

## RPC clients

RPC is enabled on localhost by default, with a random token generated on this
machine at install time. Read it with `sudo cat /var/lib/aria-move/rpc-secret` and
enter it in your aria2 client. The token persists across upgrades and reinstalls.
At every start the internal launcher writes a private runtime copy of aria2.conf
with this token appended, so the token never appears in the package, a conffile
or a process command line. Any `rpc-secret` line in aria2.conf is ignored.

To use your own token, write it as a single line without spaces and restart:

```bash
sudo install -o aria-move -g aria-move -m 0600 /dev/null /var/lib/aria-move/rpc-secret
printf '%s\n' 'your-token' | sudo tee /var/lib/aria-move/rpc-secret > /dev/null
sudo systemctl restart aria-move
```

For a command-line download (requires curl), replace the example URL:

```bash
token=$(sudo cat /var/lib/aria-move/rpc-secret)
curl --fail --silent --show-error http://localhost:6800/jsonrpc \
  --json "{\"jsonrpc\":\"2.0\",\"id\":\"download\",\"method\":\"aria2.addUri\",\"params\":[\"token:$token\",[\"https://example.com/file.zip\"]]}"
unset token
```

Use SSH forwarding for remote access. Do not expose unauthenticated RPC.
If port 6800 is occupied, change `rpc-listen-port` in aria2.conf and restart.
The service manages aria2; the mover itself runs once per completed download.

## Custom storage

Stop the service. Set matching incoming paths in aria2.conf (dir) and config.xml
(download_base), and completed_base in XML. Use absolute non-nested paths.
Grant the service account access to those specific directories; avoid recursive
ownership changes to shared storage. Use `sudo systemctl edit aria-move`:

```ini
[Unit]
RequiresMountsFor=/srv/downloads/incoming /srv/downloads/completed

[Service]
ReadWritePaths=/srv/downloads/incoming /srv/downloads/completed
```

Every `ReadWritePaths` entry must already exist, or systemd refuses to start the
service at all (`status=226/NAMESPACE`, journal: "Failed to set up mount
namespacing: ...: No such file or directory"). If `log_file` points outside
`/var/log/aria-move`, also list the log's directory, or the mover cannot create it.

The service filesystem is read-only except state/log directories and explicit
writable paths. Home directories are inaccessible; prefer /srv or defaults.
Run `sudo -u aria-move aria_move --check-config`, then restart. Validation checks
configuration structure and paths; actual write access is checked during moves.

## Existing aria2

Disable the bundled service with `sudo systemctl disable --now aria-move`.
Set `on-download-complete=/usr/bin/aria_move` in your existing aria2 instance.
Set both bases and a writable log location in config.xml for that instance's
account. Never use two instances against the same incoming directory/session.
There is no need for a wrapper script.

## Removal

Remove stops/disables the service; purge also deletes package conffiles.
Both retain download data, session, logs and the account. Back up files before
manually deleting /var/lib/aria-move or /var/log/aria-move or removing the account.
Reinstallation restores package files and reuses retained data. Dpkg handles
local conffile changes on upgrades with its normal policy.

Windows runs under the current account without Linux-specific paths or users.
Existing startup management should launch aria2 with the executable completion
hook; aria_move exits after each move.
