<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>aria_move (Rust) - Documentation</title>

    <!-- Added: styles and script to provide a "Copy" button for each code block -->
    <style>
      .code-container { position: relative; }
      .copy-btn {
        position: absolute;
        top: 6px;
        right: 6px;
        padding: 4px 8px;
        font-size: 12px;
        border: 0;
        background: #2d3748;
        color: #fff;
        border-radius: 4px;
        cursor: pointer;
        opacity: 0.9;
      }
      .copy-btn:active { transform: translateY(1px); }
      .copy-success { background: #2f855a; }
      pre { overflow: auto; padding: 12px; background: #f6f8fa; border-radius: 6px; }
    </style>
    <script>
      document.addEventListener("DOMContentLoaded", function () {
        try {
          document.querySelectorAll("pre > code").forEach(function (codeBlock) {
            var pre = codeBlock.parentNode;
            // wrap pre with container to position button
            var container = document.createElement("div");
            container.className = "code-container";
            pre.parentNode.insertBefore(container, pre);
            container.appendChild(pre);

            var btn = document.createElement("button");
            btn.className = "copy-btn";
            btn.type = "button";
            btn.innerText = "Copy";

            btn.addEventListener("click", function () {
              var text = codeBlock.innerText;
              if (navigator.clipboard && navigator.clipboard.writeText) {
                navigator.clipboard.writeText(text).then(function () {
                  btn.classList.add("copy-success");
                  btn.innerText = "Copied";
                  setTimeout(function () {
                    btn.classList.remove("copy-success");
                    btn.innerText = "Copy";
                  }, 1500);
                }, function () {
                  fallbackCopy(text, btn);
                });
              } else {
                fallbackCopy(text, btn);
              }
            });

            container.insertBefore(btn, pre);
          });

          function fallbackCopy(text, btn) {
            var textarea = document.createElement("textarea");
            textarea.value = text;
            textarea.style.position = "fixed";
            textarea.style.left = "-9999px";
            document.body.appendChild(textarea);
            textarea.select();
            try {
              document.execCommand("copy");
              btn.classList.add("copy-success");
              btn.innerText = "Copied";
              setTimeout(function () {
                btn.classList.remove("copy-success");
                btn.innerText = "Copy";
              }, 1500);
            } catch (e) {
              btn.innerText = "Copy failed";
              setTimeout(function () { btn.innerText = "Copy"; }, 1500);
            }
            document.body.removeChild(textarea);
          }
        } catch (e) {
          // silent fail (README rendering environments like GitHub may ignore scripts)
        }
      });
    </script>
</head>
<body>

<h1>aria_move (Rust)</h1>

<!-- Badges -->
<p>
  <a href="#license"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-green.svg"></a>
  <a href="#development"><img alt="Build" src="https://img.shields.io/badge/build-cargo-blue.svg"></a>
  <a href="#usage"><img alt="Platform" src="https://img.shields.io/badge/platform-macOS%20|%20Linux%20|%20Windows-orange.svg"></a>
</p>

<!-- Table of Contents -->
<nav aria-label="Table of contents">
  <h2 id="toc">Table of Contents</h2>
  <ul>
    <li><a href="#quickstart">Quickstart</a></li>
    <li><a href="#requirements">Requirements & Build Tools</a></li>
    <li><a href="#installation">Installation</a></li>
    <li><a href="#usage">Usage</a> • <a href="#help-snapshot">--help snapshot</a></li>
    <li><a href="#aria2-integration">Integration with aria2</a> • <a href="#systemd-integration">systemd integration</a></li>
    <li><a href="#configuration">Configuration</a></li>
    <li><a href="#logging">Logging</a></li>
    <li><a href="#development">Development</a></li>
    <li><a href="#troubleshooting">Troubleshooting</a></li>
    <li><a href="#prebuilt">Prebuilt Binaries</a></li>
    <li><a href="#feature-matrix">Platform Feature Matrix</a></li>
    <li><a href="#links">Links</a> • <a href="#license">License</a> • <a href="#contributing">Contributing</a></li>
  </ul>
</nav>

<!-- Intro -->
<p>
    aria_move makes moving completed downloads effortless and safe — whether you run a single desktop client
    or manage a headless download server. Install in minutes, plug it into aria2 or any downloader hook, and
    let aria_move reliably place finished files into a curated completed directory with zero fuss.
</p>
<p>
    Designed for ease-of-use and reliability:
    quick sensible defaults, a tiny XML config you can edit later, safe-by-default behavior (no symlink-trickery,
    secure log file handling on Unix), and robust fallbacks when a straight rename isn't possible.
</p>

<h2>Why choose aria_move?</h2>
<ul>
    <li>Zero-surprise operation: safe defaults so you can run it unattended.</li>
    <li>Plug-and-play with aria2 (or any hook) — pass the task id, file count and source path and you're done.</li>
    <li>Fast and efficient: atomic renames when possible, reliable copy+rename fallback across filesystems.</li>
    <li>Safe for production: symlink defenses, disk-space checks (Unix), and secure log/config file handling.</li>
    <li>Clear observability: compact human logs or JSON for structured pipelines and log aggregation.</li>
</ul>

<h2>Key features (end users)</h2>
<ul>
    <li>Automatic move of completed items from download base to completed base</li>
    <li>Dry-run mode to preview actions without touching files</li>
    <li>Optional preservation of file permissions and timestamps</li>
    <li>Secure defaults: refuses to use log paths with symlinked ancestors on Unix</li>
    <li>Creates a secure template config on first run if none exists</li>
</ul>

<h2>Key features (for developers & integrators)</h2>
<ul>
    <li>Small, modular codebase with platform helpers for Unix/Windows separation</li>
    <li>Test suite covering races, symlink defenses and I/O helpers</li>
    <li>Structured, documented errors (AriaMoveError) for easy assertion in integration tests</li>
    <li>Traces and optional JSON logs for integration with log collectors</li>
    <li>Easy to extend: clear fs/ and platform/ boundaries to add features safely</li>
</ul>

<h2>Features</h2>
<ul>
    <li>Atomic rename when possible; safe copy+rename fallback</li>
    <li>Optional metadata preservation (permissions, mtime)</li>
    <li>Disk space check (Unix)</li>
    <li>Refuses log paths under symlinked ancestors</li>
    <li>Structured logging (human or JSON)</li>
    <li>Clear, testable error kinds</li>
    <li>Cross-platform (macOS, Linux, Windows)</li>
</ul>

<!-- Quickstart -->
<h2 id="quickstart">Quickstart (3 steps)</h2>
<ol>
  <li>
    <strong>Install</strong><br>
    <pre><code>cargo install --path .</code></pre>
  </li>
  <li>
    <strong>First run: generate a secure template config</strong><br>
    Run once so aria_move creates a config if none exists, then edit it:
    <pre><code>aria_move --print-config
# If no config exists, aria_move will create a secure template and exit.
# Edit the file shown to set your download_base and completed_base.</code></pre>
    Minimal XML template (with comments):
    <pre><code>&lt;config&gt;
  &lt;!-- Where partial/new downloads appear --&gt;
  &lt;download_base&gt;/path/to/incoming&lt;/download_base&gt;
  &lt;!-- Final destination for completed items --&gt;
  &lt;completed_base&gt;/path/to/completed&lt;/completed_base&gt;
  &lt;!-- quiet | normal | info | debug --&gt;
  &lt;log_level&gt;normal&lt;/log_level&gt;
  &lt;!-- Optional: full path to log file --&gt;
  &lt;log_file&gt;/path/to/aria_move.log&lt;/log_file&gt;
  &lt;!-- Preserve permissions and mtime when moving (slower) --&gt;
  &lt;preserve_metadata&gt;false&lt;/preserve_metadata&gt;
  &lt;!-- Recency window (seconds) for auto-resolving recent file --&gt;
  &lt;recent_window_seconds&gt;300&lt;/recent_window_seconds&gt;
&lt;/config&gt;</code></pre>
  </li>
  <li>
    <strong>Run a move</strong><br>
    Auto-resolve most recent file from download_base and move it:
    <pre><code>aria_move</code></pre>
    With explicit args (typical aria2 hook):
    <pre><code>aria_move 7b3f1234 1 /path/to/incoming/file.iso</code></pre>
  </li>
</ol>

<!-- Requirements -->
<h2 id="requirements">Requirements & build tools</h2>
<p>This project is written in Rust. You need the Rust toolchain, Git, and a few native build tools (pkg-config / C toolchain / OpenSSL headers) on some platforms. Install the items below for your OS before building.</p>

<h3>Common (all platforms)</h3>
<ul>
  <li><strong>rustup</strong> — the recommended way to install Rust (provides rustc, cargo).</li>
  <li><strong>git</strong> — to clone the repository.</li>
  <li><strong>Build tools</strong> — a C toolchain and <code>pkg-config</code> are required by some crates.</li>
  <li><strong>Extras for development:</strong> <code>rustfmt</code> and <code>clippy</code> (install via rustup).</li>
</ul>
<p>Install the Rust toolchain and developer components:</p>
<pre><code># Install rustup (one-liner)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup component add rustfmt clippy
rustc --version
cargo --version
</code></pre>

<h3>macOS (Homebrew)</h3>
<pre><code>xcode-select --install
brew install pkg-config openssl@3
export OPENSSL_DIR="$(brew --prefix openssl@3)"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig:$PKG_CONFIG_PATH"
</code></pre>

<h3>Debian / Ubuntu</h3>
<pre><code>sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl git
# optional: sudo apt install -y clang
</code></pre>

<h3>Fedora / RHEL (dnf)</h3>
<pre><code>sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config openssl-devel git
</code></pre>

<h3>Windows</h3>
<p>Pick one toolchain:</p>
<ul>
  <li><strong>MSVC (recommended)</strong>: Install “Build Tools for Visual Studio” with “Desktop development with C++”. Then:
    <pre><code>rustup default stable
rustup component add rustfmt clippy</code></pre>
  </li>
  <li><strong>MSYS2/MinGW</strong> (alternative):
    <pre><code>pacman -Syu
pacman -S --needed base-devel mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkg-config mingw-w64-x86_64-openssl
# use the mingw64 shell to build</code></pre>
  </li>
</ul>

<p><em>Notes:</em> If native deps complain about OpenSSL, install platform dev packages (<code>libssl-dev</code>/<code>openssl-devel</code>) and export <code>OPENSSL_DIR</code>/<code>PKG_CONFIG_PATH</code> as above.</p>

<hr>

<!-- Installation -->
<h2 id="installation">Installation</h2>
<p>Pick the method that fits your environment.</p>

<h3>Easy (cross‑platform)</h3>
<pre><code>cargo install --path .
aria_move --version</code></pre>

<h3>Build from source (manual)</h3>
<pre><code>cargo build --release
# binary: target/release/aria_move
sudo cp target/release/aria_move /usr/local/bin/    # macOS/Linux
# or copy aria_move.exe into a directory on PATH (Windows)</code></pre>

<h3>macOS</h3>
<pre><code>xcode-select --install
brew install pkg-config openssl@3
export OPENSSL_DIR="$(brew --prefix openssl@3)"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig:$PKG_CONFIG_PATH"
cargo install --path .   # or: cargo build --release</code></pre>

<h3>Debian/Ubuntu</h3>
<pre><code>sudo apt install -y build-essential pkg-config libssl-dev curl git
cargo install --path .   # or: cargo build --release</code></pre>

<h3>Fedora/RHEL</h3>
<pre><code>sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config openssl-devel git
cargo install --path .   # or: cargo build --release</code></pre>

<h3>Windows</h3>
<pre><code># MSVC
rustup default stable
cargo install --path .
# MSYS2 (from mingw64 shell)
cargo install --path .</code></pre>

<p><strong>Uninstall:</strong> <code>cargo uninstall aria_move</code></p>

<hr>

<!-- Usage -->
<h2 id="usage">Usage</h2>
<h3>Synopsis</h3>
<pre><code>aria_move [TASK_ID] [NUM_FILES] [SOURCE_PATH] [FLAGS]</code></pre>

<h3>Positional Arguments (Optional)</h3>
<p>When integrating with aria2, these are typically passed by the download-complete hook:</p>
<table border="1" cellpadding="8" cellspacing="0">
  <thead><tr><th>Argument</th><th>Type</th><th>Description</th></tr></thead>
  <tbody>
    <tr><td><code>TASK_ID</code></td><td>String</td><td>aria2 GID (informational)</td></tr>
    <tr><td><code>NUM_FILES</code></td><td>Integer</td><td>Number of files (0 if unknown)</td></tr>
    <tr><td><code>SOURCE_PATH</code></td><td>Path</td><td>File or directory to move</td></tr>
  </tbody>
</table>

<h3>Common Flags</h3>
<table border="1" cellpadding="8" cellspacing="0">
  <thead><tr><th>Flag</th><th>Description</th></tr></thead>
  <tbody>
    <tr><td><code>--download-base &lt;PATH&gt;</code></td><td>Override download base directory</td></tr>
    <tr><td><code>--completed-base &lt;PATH&gt;</code></td><td>Override completed base directory</td></tr>
    <tr><td><code>-d</code>, <code>--debug</code></td><td>Set log level to debug</td></tr>
    <tr><td><code>--log-level &lt;LEVEL&gt;</code></td><td>quiet, normal, info, debug</td></tr>
    <tr><td><code>--print-config</code></td><td>Show config file location and exit</td></tr>
    <tr><td><code>--dry-run</code></td><td>Log actions without modifying filesystem</td></tr>
    <tr><td><code>--preserve-metadata</code></td><td>Preserve file permissions and mtime</td></tr>
    <tr><td><code>--json</code></td><td>Emit logs in JSON format</td></tr>
  </tbody>
</table>

<h3>Examples</h3>
<pre><code># Auto-resolve most recent file
aria_move

# Move a specific path (typical aria2 hook)
aria_move 7b3f1234 1 /path/to/incoming/file.iso

# Override bases
aria_move --download-base /data/incoming --completed-base /data/completed

# Dry run with JSON logs
aria_move --dry-run --json

# Show config location
aria_move --print-config
</code></pre>

<!-- Help snapshot -->
<h3 id="help-snapshot">--help snapshot (example)</h3>
<pre><code>aria_move 0.6.0
Move completed aria2 downloads safely (Rust)

USAGE:
    aria_move [OPTIONS] [TASK_ID] [NUM_FILES] [SOURCE_PATH]

ARGS:
    &lt;TASK_ID&gt;        Aria2 task id (optional, informational)
    &lt;NUM_FILES&gt;      Number of files reported by aria2 (0 = unknown)
    &lt;SOURCE_PATH&gt;    Source path passed by aria2

OPTIONS:
        --download-base &lt;PATH&gt;      Override the download base directory
        --completed-base &lt;PATH&gt;     Override the completed base directory
    -d, --debug                      Enable debug logging (shorthand for --log-level debug)
        --log-level &lt;LEVEL&gt;         Set log level: quiet, normal, info, debug
        --print-config               Print the config file location used by aria_move and exit
        --dry-run                    Show what would be done, but do not modify files/directories
        --preserve-metadata          Preserve file permissions and mtime when moving (slower)
        --json                       Emit logs in structured JSON
    -h, --help                       Print help
    -V, --version                    Print version
</code></pre>

<hr>

<!-- Integration with aria2 -->
<h2 id="aria2-integration">Integration with aria2</h2>
<p>
  aria2 exposes an on-download-complete hook you configure in <code>aria2.conf</code>.
  Use absolute paths; aria2 runs the hook under its own environment. A tiny wrapper script is recommended.
</p>

<h3>Important notes</h3>
<ul>
  <li>Use absolute paths for the wrapper and the aria_move binary.</li>
  <li>Make the wrapper executable: <code>chmod +x /usr/local/bin/aria_move_hook.sh</code>.</li>
  <li>Run aria2 as the user that should own/see the download and completed directories.</li>
  <li>Test the wrapper manually before adding it to aria2.conf.</li>
</ul>

<h3>Example — Unix (bash) wrapper</h3>
<pre><code>#!/usr/bin/env bash
# filepath: /usr/local/bin/aria_move_hook.sh
# Make executable: chmod +x /usr/local/bin/aria_move_hook.sh
exec /usr/local/bin/aria_move "$1" "$2" "$3"
</code></pre>

<p>Add to <code>aria2.conf</code>:</p>
<pre><code>on-download-complete=/usr/local/bin/aria_move_hook.sh</code></pre>

<h3>Example — Windows (batch) wrapper</h3>
<pre><code>@echo off
REM filepath: C:\Program Files\aria_move\aria_move_hook.bat
"C:\Program Files\aria_move\aria_move.exe" %1 %2 %3
</code></pre>

<p>Add to <code>aria2.conf</code>:</p>
<pre><code>on-download-complete=C:\Program Files\aria_move\aria_move_hook.bat</code></pre>

<h3 id="systemd-integration">systemd integration (aria2 as a service)</h3>
<p>If aria2 runs under systemd, ensure the service user and environment are correct, and that the wrapper path is absolute.</p>
<pre><code># Example override: sudo systemctl edit aria2c.service
[Service]
User=aria2
# Ensure /usr/local/bin is on PATH (or use absolute paths in aria2.conf as above)
Environment=PATH=/usr/local/bin:/usr/bin
# Ensure aria2.conf contains the on-download-complete=/usr/local/bin/aria_move_hook.sh line
</code></pre>

<hr>

<!-- Configuration -->
<h2 id="configuration">Configuration</h2>

<h3>Config File Location (XML)</h3>
<table border="1" cellpadding="8" cellspacing="0">
  <thead><tr><th>Platform</th><th>Default Path</th></tr></thead>
  <tbody>
    <tr><td>macOS</td><td><code>~/Library/Application Support/aria_move/config.xml</code></td></tr>
    <tr><td>Linux</td><td><code>~/.config/aria_move/config.xml</code></td></tr>
    <tr><td>Windows</td><td><code>%APPDATA%\aria_move\config.xml</code></td></tr>
  </tbody>
</table>

<h3>Override Config Location</h3>
<p>Set the <code>ARIA_MOVE_CONFIG</code> environment variable:</p>
<pre><code># macOS/Linux
export ARIA_MOVE_CONFIG=/custom/path/to/config.xml
aria_move

# Windows (PowerShell)
$env:ARIA_MOVE_CONFIG = "C:\custom\path\to\config.xml"
aria_move
</code></pre>

<h3>First Run Behavior</h3>
<p>If no config exists and <code>ARIA_MOVE_CONFIG</code> is unset, aria_move creates a secure template and exits. Edit it and rerun.</p>

<h3>Security Notes</h3>
<ul>
  <li><strong>macOS/Linux:</strong> download_base and completed_base must be owned by the current user and not group/world writable (mode &amp; 0o022 == 0).</li>
  <li><strong>Windows:</strong> Basic readonly check only; use <code>icacls</code> to verify ACLs.</li>
  <li><strong>Log file:</strong> On Unix, log file path is refused if any ancestor is a symlink.</li>
</ul>

<hr>

<!-- Logging -->
<h2 id="logging">Logging</h2>
<ul>
  <li>Human-readable (default) or JSON (<code>--json</code>)</li>
  <li>Levels: quiet, normal, info, debug</li>
</ul>
<pre><code>aria_move --json --log-level info
aria_move -d
</code></pre>

<hr>

<!-- Development -->
<h2 id="development">Development</h2>
<h3>Build</h3>
<pre><code>cargo build
cargo build --release
</code></pre>
<h3>Format and Lint</h3>
<pre><code>cargo fmt
cargo clippy --all-targets -- -D warnings
</code></pre>
<h3>Run Tests</h3>
<pre><code>cargo test
</code></pre>

<hr>

<!-- Troubleshooting -->
<h2 id="troubleshooting">Troubleshooting</h2>
<h3>Proc-macro ABI mismatch</h3>
<pre><code>cargo clean
rm -rf target
rustup update stable && rustup default stable
cargo check
</code></pre>
<h3>"unresolved import aria_move"</h3>
<pre><code>[package]
name = "aria_move"
</code></pre>
<h3>Windows</h3>
<p>Use <code>icacls</code> to inspect ACLs; disk space check is Unix-only.</p>

<hr>

<!-- Prebuilt binaries -->
<h2 id="prebuilt">Prebuilt Binaries</h2>
<p>If you publish releases, attach signed archives for macOS, Linux, and Windows on your Releases page. Verify checksums after download.</p>
<pre><code># macOS/Linux
shasum -a 256 aria_move-*.tar.gz
# Windows (PowerShell)
Get-FileHash .\aria_move-*.zip -Algorithm SHA256
</code></pre>
<p>Consider publishing a CHECKSUMS.txt and signing it (GPG) for verification.</p>

<hr>

<!-- Feature matrix -->
<h2 id="feature-matrix">Platform-Specific Feature Matrix</h2>
<table border="1" cellpadding="8" cellspacing="0">
  <thead><tr><th>Feature</th><th>macOS</th><th>Linux</th><th>Windows</th></tr></thead>
  <tbody>
    <tr><td>Atomic rename</td><td>✅</td><td>✅</td><td>✅</td></tr>
    <tr><td>Safe copy+rename fallback</td><td>✅</td><td>✅</td><td>✅</td></tr>
    <tr><td>Metadata preservation</td><td>✅</td><td>✅</td><td>✅ (basic)</td></tr>
    <tr><td>Disk space check</td><td>✅</td><td>✅</td><td>❌</td></tr>
    <tr><td>Directory security validation</td><td>✅</td><td>✅</td><td>⚠️ (readonly only)</td></tr>
    <tr><td>Symlink ancestor detection</td><td>✅</td><td>✅</td><td>❌</td></tr>
    <tr><td>O_NOFOLLOW log open</td><td>✅</td><td>✅</td><td>❌</td></tr>
    <tr><td>Structured logging (JSON)</td><td>✅</td><td>✅</td><td>✅</td></tr>
  </tbody>
</table>

<hr>

<!-- Links / License / Contributing -->
<h2 id="links">Links</h2>
<ul>
  <li><a href="./CHANGELOG.md">CHANGELOG</a> (add this file when you publish releases)</li>
  <li><a href="#">Issues</a> • <a href="#">Pull Requests</a> (update with your repository links)</li>
</ul>

<h2 id="license">License</h2>
<p>MIT</p>

<h2 id="contributing">Contributing</h2>
<p>Contributions welcome! Please open an issue or pull request. Ensure all tests pass and code is formatted/linted.</p>
<ul>
  <li>Run <code>cargo fmt</code></li>
  <li>Run <code>cargo clippy --all-targets -- -D warnings</code></li>
  <li>Run <code>cargo test</code></li>
  <li>Update this README if adding features</li>
</ul>

</body>
</html>