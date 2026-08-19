<p align="right">
  <a href="MANUAL.md">🇧🇷 Português</a> | <strong>🇺🇸 English</strong>
</p>

# 📖 Complete User Manual — OnyxSH

<p align="center">
  <img src="../usr/share/icons/hicolor/scalable/apps/onyxsh.svg" alt="OnyxSH Logo" width="96" height="96">
</p>

Welcome to the **OnyxSH User Manual**. This comprehensive guide covers all features, workflows, keyboard shortcuts, and configuration options of the terminal, from basic navigation to advanced capabilities for DevOps engineers, SREs, and Linux system administrators.

---

## 📑 Table of Contents

1. [Overview and Architecture](#1-overview-and-architecture)
2. [Graphical Interface and Navigation](#2-graphical-interface-and-navigation)
   - [Tabs and Dynamic Title Bar](#tabs-and-dynamic-title-bar)
   - [Split Panes (Horizontal and Vertical)](#split-panes)
   - [Broadcast Mode (Multi-tab Command Dispatch)](#broadcast-mode)
3. [SSH & SFTP Session Manager](#3-ssh--sftp-session-manager)
   - [Creating Folders and Connections](#creating-folders-and-connections)
   - [Authentication with SSH Keys and Passwords](#authentication-with-ssh-keys-and-passwords)
   - [Enterprise PAM / Balabit One Identity Gateways](#enterprise-pam--balabit-gateways)
   - [Importing Sessions from SecureCRT](#importing-sessions-from-securecrt)
4. [Visual SSH Tunnel Manager & Port Forwarding](#4-visual-ssh-tunnel-manager--port-forwarding)
   - [Local Port Forwarding (-L)](#local-port-forwarding--l)
   - [Remote Port Forwarding (-R)](#remote-port-forwarding--r)
   - [Dynamic SOCKS5 Proxy (-D)](#dynamic-socks5-proxy--d)
   - [Real-time Control and Session Auto-Start](#real-time-control-and-session-auto-start)
5. [Production Guard Security Mode](#5-production-guard-security-mode)
   - [Defining Production Environments](#defining-production-environments)
   - [High-Visibility Crimson Banner](#high-visibility-crimson-banner)
   - [Destructive Command Interception](#destructive-command-interception)
   - [Safe Double Confirmation and Secret Redaction](#safe-double-confirmation-and-secret-redaction)
6. [Intelligent Autocomplete & Linux Command Specs](#6-intelligent-autocomplete--linux-command-specs)
   - [Cursor-Anchored Floating Popup](#cursor-anchored-floating-popup)
   - [Built-in Command Specs Catalog](#built-in-command-specs-catalog)
   - [Keyboard Navigation and Insertion](#keyboard-navigation-and-insertion)
7. [Enriched SQLite Command History (`Ctrl + H`)](#7-enriched-sqlite-command-history-ctrl--h)
   - [Fuzzy Search and Contextual Filters](#fuzzy-search-and-contextual-filters)
   - [Pinning Favorites (⭐ Pinned)](#pinning-favorites--pinned)
   - [Flexible History Cleansing](#flexible-history-cleansing)
8. [Spotlight Command Palette (`Ctrl + Shift + P`)](#8-spotlight-command-palette-ctrl--shift--p)
9. [Integrated AI Assistant & Secure Agent Mode](#9-integrated-ai-assistant--secure-agent-mode)
   - [Supported Providers (Ollama, Gemini, Groq, OpenRouter)](#supported-providers)
   - [Automatic GPU & VRAM Detection](#automatic-gpu--vram-detection)
   - [1-Click Error Diagnostics](#1-click-error-diagnostics)
   - [Agent Mode with Audit Trail and Rollback](#agent-mode-with-audit-trail-and-rollback)
10. [Remote File Manager (SFTP) & Integrated TFTP Server](#10-remote-file-manager-sftp--integrated-tftp-server)
    - [SFTP Sidebar and Drag & Drop Transfers](#sftp-sidebar-and-drag--drop-transfers)
    - [Transparent Remote File Editing](#transparent-remote-file-editing)
    - [Integrated TFTP Server](#integrated-tftp-server)
11. [Multi-Format Terminal Exporter](#11-multi-format-terminal-exporter)
12. [Advanced Scrollback Search (`Ctrl + Shift + F`)](#12-advanced-scrollback-search-ctrl--shift--f)
13. [Semantic Shell Lifecycle Tracking (OSC 133)](#13-semantic-shell-lifecycle-tracking-osc-133)
14. [Complete Keyboard Shortcuts Table](#14-complete-keyboard-shortcuts-table)
15. [Configuration and Local Storage](#15-configuration-and-local-storage)

---

## 1. Overview and Architecture

**OnyxSH** is engineered as a modern, high-performance terminal emulator for Linux:
- **Libadwaita & GTK4 UI:** Clean, adaptive, and fully compliant with modern GNOME design standards.
- **VTE Engine with PTY Proxying:** High-speed terminal rendering with robust isolation and syntax highlighting.
- **Transactional SQLite Database:** Instant fuzzy queries, execution statistics, and persistent command history.
- **Secure AI Architecture:** Human-in-the-loop policy engine preventing direct arbitrary execution.

---

## 2. Graphical Interface and Navigation

### Tabs and Dynamic Title Bar
- OnyxSH groups your terminal sessions in top tabs.
- When a single tab is open, the title bar displays a clean **OnyxSH** or `OnyxSH - [Session Host]` header. When multiple tabs are created, it automatically switches to a scrollable tab bar.
- **New Tab:** Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> or click `+`.
- **Close Tab:** Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd>.

### Split Panes
Divide any tab into multiple simultaneous terminals:
- **Split Horizontally:** Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd>.
- **Split Vertically:** Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd>.
- **Focus Pane:** Click on the desired terminal.
- **Resize:** Drag the divider handles between panes.

### Broadcast Mode
Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>B</kbd> to open the broadcast input bar. Keystrokes entered in this bar are sent simultaneously to **all open tabs and panes** — ideal for multi-server operations.

---

## 3. SSH & SFTP Session Manager

Open the sessions sidebar from the top bar icon or the main menu.

### Creating Folders and Connections
- **Hierarchical Tree:** Group servers by client, environment (*Production*, *Staging*, *Dev*), or region.
- **Session Settings:**
  - Host/IP and Port (default: 22).
  - Username (`root`, `ubuntu`, `admin`, etc.).
  - Authentication Method (Password, SSH Private Key, or SSH Agent).
  - Character Encoding (UTF-8, ISO-8859-1, etc.).
  - Custom colors and syntax highlighting rules per session.

### Authentication with SSH Keys and Passwords
- Passwords are encrypted and safely stored using system security backends.
- Full support for RSA, Ed25519, and ECDSA keys (`~/.ssh/id_*`).

### Enterprise PAM / Balabit Gateways
OnyxSH automatically intercepts interactive gateway banners from **Balabit / One Identity Safeguard / CyberArk** and displays a dedicated dialog to submit credentials without breaking the terminal stream.

### Importing Sessions from SecureCRT
1. Open the Main Menu (`☰`) ➔ **Import SecureCRT Sessions**.
2. Select your SecureCRT folder containing `.ini` session files.
3. The complete folder tree, hosts, ports, users, and Password V2 credentials (`02:<hex>`) are imported automatically.

---

## 4. Visual SSH Tunnel Manager & Port Forwarding

Access via the Main Menu (`☰`) ➔ **SSH Tunnel Manager** or Command Palette (<kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> ➔ *SSH Tunnels*).

### Local Port Forwarding (`-L`)
Forward a local port to a remote service reachable through the SSH server.
- **Example:** Access a remote MySQL database on port 3306 via `localhost:3306`.
- **Config:** Local Port (`3306`) ➔ Remote Host (`127.0.0.1`) ➔ Remote Port (`3306`).

### Remote Port Forwarding (`-R`)
Expose a local development port directly onto the remote server.
- **Example:** Let the remote server access your local test API on port `8000`.

### Dynamic SOCKS5 Proxy (`-D`)
Create an encrypted local SOCKS5 proxy server.
- **Example:** Set local port `1080`. Route browser or network traffic through `socks5://127.0.0.1:1080` to tunnel all outbound requests through the remote SSH host.

### Real-time Control and Session Auto-Start
- **Quick Switches:** Toggle any tunnel with 1 click using `Gtk.Switch`.
- **Auto-start:** Enable *Start automatically with session* to launch tunnels upon connecting to the associated SSH host.
- **Global Stop:** Terminate all active tunnels instantly with the global stop button.

---

## 5. Production Guard Security Mode

**Production Guard** provides comprehensive safeguards to prevent accidental operations on mission-critical servers.

### Defining Production Environments
- Enable the **Production Environment** switch in any session or folder edit dialog.
- Sessions inside marked folders inherit production policies automatically.

### High-Visibility Crimson Banner
When connecting to a production terminal, a persistent crimson gradient banner with `🛡️ PRODUCTION` remains visible at the top of the tab.

### Destructive Command Interception
Pressing <kbd>Enter</kbd> on dangerous commands intercepts execution before sending to the shell:
- **Mass Deletion:** `rm -rf`, `rm -fr`, `shred -u`, `wipefs`.
- **Disk Formatting:** `mkfs.*`, `dd of=/dev/...`, `fdisk`, `parted`.
- **System Shutdown:** `shutdown`, `reboot`, `poweroff`, `init 0`.
- **Critical Services:** `systemctl stop/disable`, `service ... stop`.
- **Database Operations:** `DROP DATABASE`, `TRUNCATE TABLE`.
- **Forced Git:** `git push --force`, `git reset --hard`.

### Safe Double Confirmation and Secret Redaction
- The confirmation dialog requires typing the **exact host or session name** to proceed.
- Press <kbd>Esc</kbd> or click *Abort* to cancel safely (`Ctrl+C` signal sent).
- **AI Privacy:** Passwords, API tokens, and private keys are automatically redacted (`[REDACTED]`) before dispatch to AI providers.

---

## 6. Intelligent Autocomplete & Linux Command Specs

OnyxSH features a real-time, cursor-anchored autocomplete engine:

### Cursor-Anchored Floating Popup
- Displays completions directly beneath the prompt cursor with icons, flags, and natural language descriptions.

### Built-in Command Specs Catalog
Declarative specifications for 50+ essential tools:
- **System:** `apt`, `systemctl`, `journalctl`, `ufw`.
- **Containers & Network:** `docker`, `ssh`, `curl`, `ping`, `ip`, `ss`, `rsync`.
- **Files & Utilities:** `tar`, `chmod`, `chown`, `find`, `grep`, `mkdir`, `rm`, `ls`, `cp`, `mv`, `cat`.
- **Performance:** `htop`, `top`, `ps`, `df`, `du`, `free`, `kill`.

### Keyboard Navigation and Insertion
- **Navigate:** Use <kbd>↑</kbd> and <kbd>↓</kbd> arrows.
- **Confirm:** Press <kbd>Tab</kbd> or <kbd>Enter</kbd>.
- **Dismiss:** Press <kbd>Esc</kbd>.

---

## 7. Enriched SQLite Command History (`Ctrl + H`)

Press <kbd>Ctrl</kbd> + <kbd>H</kbd> in any terminal to open the Enriched Command History dialog.

### Fuzzy Search and Contextual Filters
- Type any part of a previous command, argument, or directory path.
- **Filter Pills:**
  - **All:** Full history list.
  - **📁 Current Directory:** Filters commands executed in current `$PWD`.
  - **🖥️ Remote Host:** Filters commands executed on active host.
  - **⭐ Pinned Favorites:** Shows starred commands.

### Pinning Favorites (⭐ Pinned)
- Click the star icon or press <kbd>Ctrl</kbd> + <kbd>P</kbd> to pin a command.
- Pinned items stay at the top and are preserved during routine history cleanses.

### Flexible History Cleansing
- Click the trash icon or press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Delete</kbd>:
  - **Clear Non-Favorites:** Keeps all starred ⭐ commands.
  - **Clear Failed:** Removes commands that exited with non-zero status (`exit_code != 0`).
  - **Clear Everything:** Wipes the entire history database.

---

## 8. Spotlight Command Palette (`Ctrl + Shift + P`)

Control 100% of OnyxSH from the keyboard:
- Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd>.
- Search for actions: *"new tunnel"*, *"split vertical"*, *"ai assistant"*, *"export"*, *"preferences"*, or saved SSH server names.
- Press <kbd>Enter</kbd> to execute immediately.

---

## 9. Integrated AI Assistant & Secure Agent Mode

Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>I</kbd> to open the AI Chat Assistant panel.

### Supported Providers
Configure your preferred AI provider in AI settings:
- **Local Ollama / LM Studio:** 100% offline on your local GPU.
- **Google Gemini:** High-speed inference with large context windows.
- **Groq:** Ultra-fast LPU inference (Llama 3 / Mixtral).
- **OpenRouter:** Multi-model ecosystem.

### Automatic GPU & VRAM Detection
- Detects GPU hardware and available VRAM.
- **Context Recommendations:** Suggests optimal context size (`num_ctx` from 4K to 128K) to prevent out-of-memory slowdowns.
- **VRAM Lifecycle:** Background preloading on startup and automatic GPU unloading on app exit.

### 1-Click Error Diagnostics
When a command fails (`exit_code != 0`), click **Analyze with AI** next to the prompt for an instant diagnosis and proposed fix.

### Agent Mode with Audit Trail and Rollback
- Structured `ActionPlan` generation.
- Security Policy Engine (Levels 0–4).
- Side-by-side diff previews for file edits.
- SHA-256 rollback support via `audit.jsonl`.

---

## 10. Remote File Manager (SFTP) & Integrated TFTP Server

### SFTP Sidebar and Drag & Drop Transfers
- On SSH tabs, click the folder icon to open the SFTP panel.
- Drag and drop files from your desktop file manager directly into the SFTP view to upload.

### Transparent Remote File Editing
- Right-click any remote file and select **Edit File**.
- OnyxSH downloads it to a secure temporary cache and opens it in your default local editor (VS Code, Gedit, Kate).
- Saving locally automatically uploads changes back to the remote server.

### Integrated TFTP Server
Access via Main Menu ➔ **TFTP Server** to transfer firmwares and router configs.

---

## 11. Multi-Format Terminal Exporter

Access via Main Menu ➔ **Export Terminal...**:
- 📄 **Plain Text (`.txt`)**
- 📋 **Log File (`.log`)** with session metadata
- 📝 **Markdown (`.md`)** formatted in code blocks
- 🌐 **Styled HTML (`.html`)** with dark theme and ANSI colors
- 🎬 **Asciinema (`.cast`)** for session playback

---

## 12. Advanced Scrollback Search (`Ctrl + Shift + F`)

Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> for the search bar:
- **`Aa`**: Case Sensitive.
- **`\b`**: Whole Word.
- **`.*`**: Regular Expressions.
- **Keys:** <kbd>Enter</kbd> (next), <kbd>Shift</kbd> + <kbd>Enter</kbd> (previous), <kbd>Esc</kbd> (close).

---

## 13. Semantic Shell Lifecycle Tracking (OSC 133)

- **Execution Timers:** Millisecond-accurate command timers (e.g., `⏱ 2.34s`).
- **Prompt Jumping:** Jump to previous/next prompt with <kbd>Alt</kbd> + <kbd>↑</kbd> and <kbd>Alt</kbd> + <kbd>↓</kbd>.
- **Surgical Output Capture:** Isolate command outputs cleanly without prompt noise.

---

## 14. Complete Keyboard Shortcuts Table

| Shortcut | Action | Scope |
|---|---|---|
| <kbd>F2</kbd> | Open Preferences Dialog | Global |
| <kbd>Ctrl</kbd> + <kbd>H</kbd> | Open Enriched Command History | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> | Open Spotlight Command Palette | Global |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>I</kbd> | Toggle AI Assistant Panel | Global |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> | Open Search in Terminal | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>B</kbd> | Toggle Broadcast Input Bar | Global |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> | New Local Tab | Tabs |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd> | Close Tab / Active Split Pane | Tabs |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd> | Split Terminal Horizontally | Panes |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd> | Split Terminal Vertically | Panes |
| <kbd>Alt</kbd> + <kbd>↑</kbd> | Jump to Previous Prompt (OSC 133) | Terminal |
| <kbd>Alt</kbd> + <kbd>↓</kbd> | Jump to Next Prompt (OSC 133) | Terminal |
| <kbd>Ctrl</kbd> + <kbd>+</kbd> | Zoom In Font Size | Terminal |
| <kbd>Ctrl</kbd> + <kbd>-</kbd> | Zoom Out Font Size | Terminal |
| <kbd>Ctrl</kbd> + <kbd>0</kbd> | Reset Font Zoom | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>C</kbd> | Copy Selected Text | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> | Paste from Clipboard | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Del</kbd> | Open Clear History Dialog | History Window |

---

## 15. Configuration and Local Storage

Configuration files are located in `~/.config/onyxsh/`:

```text
~/.config/onyxsh/
├── settings.json          # UI preferences, fonts, color themes, shortcuts, AI configuration
├── sessions.json          # Saved SSH connections, folder hierarchy, credentials
├── command_history.db     # SQLite database for enriched command history
├── session_state.json     # Tab state for automatic session restoration
├── layouts/               # Saved split pane layouts
└── backups/               # Configuration and session backups
```
