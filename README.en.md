<p align="right">
  <a href="README.md">🇧🇷 Português</a> | <strong>🇺🇸 English</strong>
</p>

# OnyxSH

<p align="center">
  <img src="usr/share/icons/hicolor/scalable/apps/io.github.vagnarok.OnyxSH.svg" alt="OnyxSH Logo" width="128" height="128">
</p>

<p align="center">
  <strong>A modern terminal emulator with AI, SSH, and semantic intelligence for developers and sysadmins</strong>
</p>
<p align="center">
  <a href="https://github.com/VaGNaroK/OnyxSH/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-green.svg" alt="License"/></a>
  <a href="https://www.gtk.org/"><img src="https://img.shields.io/badge/GTK-4.0+-orange.svg" alt="GTK Version"/></a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/"><img src="https://img.shields.io/badge/libadwaita-1.0+-purple.svg" alt="libadwaita Version"/></a>
</p>

> [!NOTE]
> **OnyxSH** is an independent evolution and hard fork of [Zashterminal](https://github.com/leoberbert/zashterminal) (originally created by Leonardo Berbert). It features a completely rebranded identity, deep AI integration, semantic shell lifecycle tracking (OSC 133), enriched SQLite command history, spotlight Command Palette, automatic session restoration, and 28 supported languages.

**OnyxSH** is a modern, intuitive, and high-performance terminal emulator built with GTK4 and Libadwaita. It combines advanced features for DevOps/SREs with a clean, fast, and native Linux interface.

---

## Why OnyxSH?

- **Integrated & Privacy-First AI**: 1-click error diagnostics, multiple providers (Local Ollama, Gemini, Groq, OpenRouter), background VRAM preloading and auto-unload.
- **Visual SSH Tunnel Manager & Port Forwarding**: Real-time management of Local (`-L`), Remote (`-R`), and Dynamic SOCKS5 (`-D`) SSH tunnels with 1-click activation switches.
- **Production Guard Security Mode**: Persistent high-visibility banner, secret redaction, and automatic interception of destructive commands on production servers.
- **Intelligent Autocomplete with Linux Specs**: Rich cursor-anchored completion with declarative specifications for 50+ Linux commands, SQLite history, and snippet templates.
- **Semantic Shell Integration (OSC 133 / OSC 6)**: Tracks command lifecycle, measures execution time (`⏱ 1.4s`), enables quick prompt jumping (`Alt + Up` / `Alt + Down`), and surgical output extraction.
- **Enriched SQLite History (`Ctrl + H`)**: Instant fuzzy search with contextual filters (current directory, remote host, ⭐ pinned favorites), execution counters, clear options, and prompt injection (`Tab`).
- **Spotlight Command Palette (`Ctrl + Shift + P`)**: Fast keyboard-driven discovery and execution of all terminal actions, SSH tunnels, sessions, tabs, and preferences.
- **Multi-format Terminal Exporter**: Export terminal history or selections into `.txt`, `.log`, `.md`, `.html`, and `.cast` (Asciinema).
- **Automatic Session Restoration**: Restores tabs, split layouts, `$PWD` working directories, and SSH sessions across app restarts.
- **Complete Remote Workflow Management**: SSH/SFTP folder trees, integrated TFTP server, Drag & Drop transfers, and transparent remote file editing.
- **Native & Elegant Design**: Built on Libadwaita with light/dark modes, smooth transparency, and customizable color themes.
- **Full Internationalization**: Fully translated and synchronized across **28 languages**.

📖 **Check out the [Complete User Manual](docs/MANUAL.md)** for an in-depth guide on all features!

---

## SecureCRT Migration & PAM Compatibility

- **Direct SecureCRT Session Import**: Import `.ini` sessions directly from the application menu.
- **Batch Folder Tree Import**: Recursively imports nested session folder hierarchies.
- **SecureCRT Password V2 Support**: Automatically decrypts `02:<hex>` passwords.
- **Balabit / One Identity Gateway Support**: Full support for *keyboard-interactive* authentication used in enterprise PAM environments.

---

## Screenshots

<img width="1457" height="699" alt="OnyxSH Main Interface" src="https://github.com/user-attachments/assets/4c264548-909e-4edb-95be-a5dc6a6756bb" />

<img width="1457" height="699" alt="Session Manager and Split Panes" src="https://github.com/user-attachments/assets/6aba3c63-a181-4e3c-8870-d58ceae11daa" />

<img width="1457" height="699" alt="Remote File Manager Panel" src="https://github.com/user-attachments/assets/46e41739-7c28-47d7-b4ba-26e9320b0061" />

---

## Key Features

### 🌐 Visual SSH Tunnel Manager and Port Forwarding
* **3 Forwarding Modes**:
  - 🟢 **Local Forwarding (`-L`)**: Access remote databases, web dashboards, and internal services via local ports.
  - 🔄 **Remote Forwarding (`-R`)**: Expose local developer services directly onto the remote server.
  - 🛡️ **Dynamic Port Forwarding (`-D`)**: Instant encrypted SOCKS5 proxy creation over SSH.
* **Real-time Control Panel**: 1-click toggling with `Gtk.Switch`, quick clipboard copy of local address, and global stop button.
* **Session Auto-Start**: Automatically start tunnels when connecting to associated SSH sessions.

---

### 🛡️ Production Guard Protection Mode
* **Production Banner**: High-visibility persistent crimson gradient banner on tabs connected to production environments.
* **Destructive Command Interception**: Intercepts high-risk operations such as `rm -rf`, `mkfs`, `dd of=/dev/...`, `shutdown`, `reboot`, `systemctl stop`, `DROP DATABASE`, and `git push --force`.
* **Safe Double Confirmation**: Requires typing the exact session/host name before unlocking execution.
* **Secret Redaction**: Automatically sanitizes API tokens, private keys, and passwords before AI transmission.

---

### ⚡ Intelligent Autocomplete & Linux Command Specs
* **Cursor-Anchored Floating Popup**: Declarative explanations and flags for 50+ standard Linux commands (`apt`, `docker`, `git`, `systemctl`, `curl`, `rsync`, `chmod`, etc.).
* **Contextual Suggestions**: Intelligent scoring combining command specs, frequent commands from SQLite in the current directory, and snippets.
* **Seamless Navigation**: Navigate with `↑`/`↓` and complete with `Tab` or `Enter`.

---

### 🤖 Integrated AI Assistant & VRAM Management

<img width="1457" height="699" alt="AI Chat Assistant Panel" src="https://github.com/user-attachments/assets/762fa599-a266-41c3-83c2-f28fe825f0f6" />

* **Multiple Providers**: Native support for **Local Models** (Ollama / LM Studio), **Groq**, **Google Gemini**, and **OpenRouter**.
* **Automatic GPU & VRAM Detection**: Dynamically recognizes NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs), and system RAM.
* **Context Window Selector (4K to 128K tokens)**: Allows fine-tuning context size (`num_ctx`) with hardware-aware recommendations.
* **Smart VRAM Lifecycle**:
  - **Async Preloading:** Preloads the local model into VRAM in the background at startup.
  - **Auto-Unload:** Automatically unloads the model from GPU memory when the application closes.
* **1-Click Error Diagnostics**: When a command fails (`exit_code != 0`), an interactive badge allows sending the output directly to the AI for analysis and proposed fix.

---

### 🔍 Command Palette (`Ctrl + Shift + P`) & SQLite History (`Ctrl + H`)

* **Command Palette**: Modal spotlight dialog indexing all application actions, split controls, AI assistant, syntax highlights, SSH tunnels, and sessions.
* **Enriched History**: Structured SQLite storage for every executed command, directory `$PWD`, duration, timestamps, and exit codes.
  - **Pill Filters:** *All*, *Current Directory*, *Remote Host*, *⭐ Pinned*.
  - **Quick Keybindings:** `Enter` (execute), `Tab` (insert into prompt for editing), `Ctrl + P` (pin favorite), `Delete` (delete entry), `Ctrl + Shift + Delete` (open clear options).
  - **Flexible Clear Options:** Clear non-favorite commands, failed commands, or the entire history in 1 click.

---

### 📤 Multi-Format Terminal Exporter
* Export terminal buffer or selected text into 5 formats:
  - 📄 **Plain Text (`.txt`)**
  - 📋 **Log File (`.log`)** with complete session metadata
  - 📝 **Markdown (`.md`)** formatted in code blocks
  - 🌐 **HTML (`.html`)** with full ANSI colors and dark theme styling
  - 🎬 **Asciinema (`.cast`)** for sharing terminal recordings

---

### 🛡️ Secure Agent Mode (Zero Direct Execution)

Strict security architecture for AI-assisted operations:

```
[ User ] ── Request ──▶ [ LLM Provider (Groq / Gemini / Ollama) ]
                                      │
                             Generates ActionPlan JSON
                                      ▼
                             [ PolicyEngine ]
                      (0-4 Classification + Denylists)
                                      │
                                      ▼
                         [ UI: User Approval ]
                                      │
                  ┌───────────────────┴───────────────────┐
                  ▼                                       ▼
      🟢 Level 0 (1-click)                    🔵 Level 1 (Diff + Backup)
      🟠 Level 2 (Polkit Admin)               ⛔ Level 4 (Blocked)
                  │                                       │
                  └───────────────────┬───────────────────┘
                                      ▼
                              [ ToolRegistry ]
                                      │
                                      ▼
                       [ AuditLog + Rollback JSONL ]
```

- **Automatic Secret Redactor:** Masks API keys, SSH private keys, and credentials before remote LLM dispatch.
- **PathGuard Anti-Bypass:** Blocks reading/writing sensitive paths (`~/.ssh`, `~/.aws`, `.env`) and shell startup scripts (`.bashrc`, `.zshrc`).
- **Audit Trail & Rollback:** Continuous history in `audit.jsonl` with SHA-256 integrity-verified rollback.

---

## 📥 Installation & Packaging

### 📦 Flatpak (Recommended for Any Linux Distribution)

Flatpak is the recommended format to run OnyxSH on any Linux distribution (Ubuntu, Debian, Fedora, Arch Linux, Manjaro, openSUSE, etc.) with sandboxing.

#### 1. Prerequisites (Flathub Repository and GNOME Runtime):
If you are on a fresh system (such as Manjaro or Arch) or haven't configured Flathub for user scope yet, ensure Flathub is added and the **GNOME 46** runtime is installed:

```bash
# Add official Flathub repository:
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

# Install required GNOME 46 runtime:
flatpak install --user flathub org.gnome.Platform//46 -y
```

#### 2. Installing and Running the Bundle:
```bash
# Install Flatpak bundle:
flatpak install --user -y --reinstall dist/onyxsh_0.9.0.flatpak

# Run:
flatpak run io.github.vagnarok.OnyxSH
```

### 📦 Debian Package (.deb - Ubuntu, Linux Mint, Debian)

```bash
sudo apt install ./dist/onyxsh_0.9.0_all.deb
```

### ⚡ Universal Installer (`install.sh`)

```bash
# Install on system:
./install.sh install

# Build Flatpak bundle:
./scripts/build_flatpak.sh --clean-cache

# Build .deb package:
./scripts/build_deb.sh --clean-cache
```

---

## 💻 Default Shortcuts

| Shortcut | Action |
|---|---|
| **`F2`** | Open Preferences Dialog |
| **`Ctrl + H`** | Enriched Command History (SQLite) |
| **`Ctrl + Shift + P`** | Command Palette (Spotlight) |
| **`Alt + Up` / `Alt + Down`** | Jump Between Prompts |
| **`Ctrl + Shift + T`** | New Tab |
| **`Ctrl + Shift + W`** | Close Tab / Active Split Pane |
| **`Ctrl + Shift + D`** | Split Terminal Horizontally |
| **`Ctrl + Shift + E`** | Split Terminal Vertically |
| **`Ctrl + Shift + F`** | Search in Terminal Scrollback (with Regex support) |
| **`Ctrl + Shift + I`** | Open AI Assistant Panel |
| **`Ctrl + Shift + B`** | Broadcast Command to All Tabs |

---

## ⚙️ Configuration Files

Configs are stored in `~/.config/onyxsh/` (with automatic migration from `~/.config/zashterminal`):

| File / Directory | Description |
|---|---|
| `settings.json` | General preferences, appearance, shortcuts, AI configuration |
| `sessions.json` | Saved SSH/SFTP connections and folder hierarchy |
| `command_history.db` | SQLite database for enriched command history |
| `session_state.json` | Tab state and session restore data |
| `layouts/` | Saved window layouts and split configurations |
| `backups/` | Backups and manifest files |

---

## 📄 License

This project is licensed under the **GNU General Public License v3 (GPLv3)** — see the [LICENSE](LICENSE) file for details.

---

## 👏 Credits & Acknowledgments

- **Original Project:** Originally based on [Zashterminal](https://github.com/leoberbert/zashterminal), created by **Leonardo Berbert**.
- Thanks to the developers and communities behind **GNOME**, **GTK**, **libadwaita**, **VTE**, and **Pygments**.
