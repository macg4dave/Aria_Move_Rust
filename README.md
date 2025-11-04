# aria_move — README snippet

Config file and usage (quick reference)
- Default config path (platform-aware):
  - Linux/XDG: $XDG_CONFIG_HOME/aria_move/config.xml (or $HOME/.config/aria_move/config.xml)
  - macOS: $HOME/Library/Application Support/aria_move/config.xml
- You can override the config file location with the ARIA_MOVE_CONFIG environment variable.

Example config (XML)
<config>
  <download_base>/path/to/incoming</download_base>
  <completed_base>/path/to/completed</completed_base>
  <log_level>info</log_level>
</config>

Common CLI usage
- Print config file location (no side effects):
  aria_move --print-config

- Create a template config (auto-created on first run when no ARIA_MOVE_CONFIG is set):
  Run aria_move with no other flags; if no config exists a template will be created and the path printed.

- Dry-run (show what would be done, without touching the filesystem):
  aria_move --dry-run /path/to/source

Notes
- The tool prefers atomic renames; if renaming across filesystems fails it falls back to copy+remove.
- The tool detects files that are likely still being written (common temp suffixes and a size-stability probe) and will refuse to move them unless they appear stable.
- For scripting/automation, check exit codes and logs. Enable debug logging with `--log-level debug` or `-d`.