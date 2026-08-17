<p align="right">
  <a href="README.md">🇧🇷 Português</a> | <strong>🇺🇸 English</strong>
</p>

# OnyxSH

<p align="center">
  <img src="https://github.com/VaGNaroK/OnyxSH/blob/main/usr/share/icons/hicolor/scalable/apps/onyxsh.svg" alt="OnyxSH Logo" width="128" height="128">
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
- **Semantic Shell Integration (OSC 133 / OSC 6)**: Tracks command lifecycle, measures execution time (`⏱ 1.4s`), enables quick prompt jumping (`Alt + Up` / `Alt + Down`), and surgical output extraction.
- **Enriched SQLite History (`Ctrl + R`)**: Instant fuzzy search with contextual filters (current directory, remote host, ⭐ pinned favorites), execution counters, and prompt injection (`Tab`).
- **Spotlight Command Palette (`Ctrl + Shift + P`)**: Fast keyboard-driven discovery and execution of all terminal actions, SSH sessions, tabs, and preferences.
- **Automatic Session Restoration**: Restores tabs, split layouts, `$PWD` working directories, and SSH sessions across app restarts.
- **Complete Remote Workflow Management**: SSH/SFTP folder trees, integrated TFTP server, Drag & Drop transfers, and transparent remote file editing.
- **Native & Elegant Design**: Built on Libadwaita with light/dark modes, smooth transparency, and customizable color themes.
- **Full Internationalization**: Fully translated and synchronized across **28 languages**.

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

### 🔍 Command Palette (`Ctrl + Shift + P`) & SQLite History (`Ctrl + R`)

* **Command Palette**: Modal spotlight dialog indexing all application actions, split controls, AI assistant, syntax highlights, and SSH sessions.
* **Enriched History**: Structured SQLite storage for every executed command, directory `$PWD`, duration, timestamps, and exit codes.
  - **Pill Filters:** *All*, *Current Directory*, *Remote Host*, *⭐ Pinned*.
  - **Quick Keybindings:** `Enter` (execute), `Tab` (insert into prompt for editing), `Ctrl + P` (pin favorite), `Delete` (delete entry).

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
| **`Ctrl + R`** | Enriched Command History (SQLite) |
| **`Ctrl + Shift + P`** | Command Palette |
| **`Alt + Up` / `Alt + Down`** | Jump Between Prompts |
| **`Ctrl + Shift + T`** | New Tab |
| **`Ctrl + Shift + W`** | Close Tab / Active Split Pane |
| **`Ctrl + Shift + D`** | Split Terminal Horizontally |
| **`Ctrl + Shift + E`** | Split Terminal Vertically |
| **`Ctrl + Shift + F`** | Search in Terminal |

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
