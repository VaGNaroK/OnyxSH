<p align="right">
  <a href="README.md">🇧🇷 Português</a> | <strong>🇺🇸 English</strong>
</p>

# Zashterminal

<p align="center">
  <img src="https://github.com/VaGNaroK/zashterminal-Fork/blob/main/usr/share/icons/hicolor/scalable/apps/zashterminal.svg" alt="Zashterminal Logo" width="128" height="128">
</p>

<p align="center">
  <strong>A modern terminal emulator for developers, infrastructure, and system administration</strong>
</p>
<p align="center">
  <a href="https://github.com/VaGNaroK/zashterminal-Fork/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-green.svg" alt="License"/></a>
  <a href="https://www.gtk.org/"><img src="https://img.shields.io/badge/GTK-4.0+-orange.svg" alt="GTK Version"/></a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/"><img src="https://img.shields.io/badge/libadwaita-1.0+-purple.svg" alt="libadwaita Version"/></a>
</p>

> [!NOTE]
> This project is an enhanced fork of [Zashterminal](https://github.com/leoberbert/zashterminal) focusing on advanced security, **Secure Agent Mode (Zero Direct Execution)**, Polkit elevation, append-only audit trail, byte-identical rollback, and an improved installer.

**Zashterminal** is a modern, intuitive, and feature-rich terminal emulator built with GTK4 and Libadwaita. It blends powerful capabilities for developers and system administrators with a welcoming interface for newcomers. Built-in session management, a remote file side-panel, real-time syntax highlighting, and workflow-focused tools make command line work far more productive on Linux.

---

## Why Zashterminal?

- **Focused on Real Workflows**: Manage SSH/SFTP sessions, split panes, and window layouts without leaving the terminal.
- **Accessible & Intuitive**: Clean UI, smart defaults, and easily discoverable shortcuts.
- **Optional & Secure AI Assistance**: Only explicitly selected text is sent, keeping privacy and control in your hands.
- **Secure Agent Mode**: Policy-mediated AI task execution with visual unified diff review, Polkit elevation, and automatic backups.
- **Modern & Native UI**: Built with GTK4 + Libadwaita with light/dark themes and smooth window transparency.

---

## SecureCRT Migration & PAM Compatibility

Smooth migration from legacy tools to Zashterminal:

- **Direct SecureCRT Session Import**: Import sessions from the main menu (`Import SecureCRT Sessions`).
- **Bulk Directory Tree Import**: Support for folders with `.ini` session files.
- **SecureCRT Password V2 Compatibility**: Decrypts and imports `02:<hex>` password entries.
- **Balabit / One Identity Gateway Support**: Compatible with keyboard-interactive authentication flows used in enterprise Privileged Access Management (PAM) environments.

---

## Screenshots

<img width="1457" height="699" alt="Zashterminal Main Interface" src="https://github.com/user-attachments/assets/4c264548-909e-4edb-95be-a5dc6a6756bb" />

<img width="1457" height="699" alt="Session Manager & Panes" src="https://github.com/user-attachments/assets/6aba3c63-a181-4e3c-8870-d58ceae11daa" />

<img width="1457" height="699" alt="File Manager Side Panel" src="https://github.com/user-attachments/assets/46e41739-7c28-47d7-b4ba-26e9320b0061" />

---

## Key Features

### 🤖 AI Assistant & VRAM Management

<img width="1457" height="699" alt="AI Assistant Side Panel" src="https://github.com/user-attachments/assets/762fa599-a266-41c3-83c2-f28fe825f0f6" />

<img width="1457" height="699" alt="Command Suggestions & Execution" src="https://github.com/user-attachments/assets/4dd9482b-420d-4170-878d-e9a652493ec9" />

Zashterminal bridges your shell with Large Language Models (LLMs) with privacy and performance at its core:
* **Multi-Provider Support**: Native integration with **Local Models** (Ollama / LM Studio), **Groq**, **Google Gemini**, and **OpenRouter**.
* **Automatic GPU & VRAM Detection**: Identifies NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs), and system RAM to calculate recommended context boundaries.
* **Context Window Selector (4K to 128K tokens)**: Allows adjusting Ollama context size (`num_ctx`) with dynamic recommendations based on detected GPU VRAM.
* **Intelligent VRAM Lifecycle**:
  - **Asynchronous Preloading:** Preloads the configured local model into VRAM in the background at startup (zero latency on the first prompt).
  - **Automatic Unloading:** Automatically unloads the model from VRAM on exit, freeing the GPU for games and other workloads.
* **Sliding Window Context Retention**: Token-aware message building keeps relevant conversation history and instructions without overflowing model limits.
* **Dedicated Chat Panel**: Conversation history, command suggestions, and click-to-run buttons.

---

### 🛡️ Secure Agent Mode

The **Secure Agent Mode** empowers the AI assistant to safely plan and execute complex tasks with strict user oversight.

Unlike traditional agents that execute arbitrary shell strings, Zashterminal enforces a **Zero Direct Execution** architecture:

```
[ User ] ── Prompt Request ──▶ [ LLM Provider (Groq / Gemini / Ollama) ]
                                            │
                                  Generates ActionPlan JSON
                                            ▼
                                   [ PolicyEngine ]
                              (Risk 0-4 Classification + Denylists)
                                            │
                                            ▼
                               [ UI: User Approval Flow ]
                                            │
                        ┌───────────────────┴───────────────────┐
                        ▼                                       ▼
            🟢 Level 0 (1-Click)                    🔵 Level 1 (Diff + Backup)
            🟠 Level 2 (Polkit Admin)               ⛔ Level 4 (Blocked)
                        │                                       │
                        └───────────────────┬───────────────────┘
                                            ▼
                                    [ ToolRegistry ]
                                            │
                                            ▼
                             [ AuditLog + Rollback JSONL ]
```

#### Risk Stratification Table

| Level | Category | Examples | Approval Mechanism |
|---|---|---|---|
| 🟢 **Level 0** | Safe Read-Only | `ls`, `df -h`, `free -m`, `uptime`, `ip route` | **1 Click:** `[▶ Execute]` or `[🧪 Dry-run]` |
| 🔵 **Level 1** | User Space Write | Creating and editing files in user home | **Diff Review:** unified diff visualization with automatic backup |
| 🟠 **Level 2** | System Admin | Journal log vacuum, package cache cleanup | **Polkit Elevation:** GUI authentication via `zashterminal-admin-helper` |
| 🔴 **Level 3** | Critical Action | Package removals and system modifications | **Explicit User Confirmation** |
| ⛔ **Level 4** | Blocked / Prohibited | `rm -rf /`, `mkfs.*`, `dd of=/dev/sd*`, `chmod 777 /` | **Strict Block:** button disabled in UI |

#### Security Highlights
- **Context Isolation:** External logs and terminal output are wrapped in `<untrusted>...</untrusted>` envelopes to prevent indirect prompt injection.
- **Automatic Secret Redaction:** Masks API keys, private RSA/PGP keys, tokens, and credentials before dispatching prompts to remote LLMs.
- **PathGuard Anti-Bypass:** Blocks read/write on sensitive credentials (`~/.ssh`, `~/.aws`, `.env`) and shell dotfiles (`.bashrc`, `.zshrc`), resolving canonical paths before validation.
- **Audit Trail & Rollback:** Append-only logging in `audit.jsonl` with byte-identical restoration verified by SHA-256 checksums.

See [docs/SECURITY.md](docs/SECURITY.md) for the complete threat model specification.

---

### 📂 Advanced File Manager & Remote Editing

<img width="1457" height="699" alt="File Navigation" src="https://github.com/user-attachments/assets/a40bd623-eb31-4a8b-9fe2-e327d8b7de0c" />

- **Integrated File Panel**: Browse local and remote files without external tools.
- **Seamless Remote Editing**: Open remote files in your local editor; changes are automatically uploaded on save over SFTP/SCP.
- **Drag & Drop Transfers**: Upload files to remote servers by dragging them into the terminal window.
- **Transfer Manager**: Track progress and manage upload/download queues.

---

### ⚡ Productivity & System Administration

<img width="1457" height="699" alt="Input Broadcasting" src="https://github.com/user-attachments/assets/97aae8ed-6466-46b9-b7e4-ca1256f425ff" />

- **Input Broadcasting**: Type in one terminal and broadcast input simultaneously across selected tabs/panes.
- **Quick Prompts**: 1-click diagnostic prompts (e.g., "Explain error", "Optimize command").
- **Session Management**: Organize Local, SSH, and SFTP connections into custom folders.
- **Split Panes & Layouts**: Horizontal/vertical splits with layout saving and restoration.
- **Directory Tracking (OSC7)**: Automatic tab title updates matching the current working directory.
- **Real-Time Syntax Highlighting**: Over 50 command highlighting rules built-in.

---

## 📥 Installation & Packaging

### 📦 Flatpak (Recommended for Any Linux Distro)

The Flatpak bundle provides secure sandboxing and universal distribution across Fedora, Manjaro, Arch Linux, Debian, Ubuntu, openSUSE, etc.:

```bash
# Install bundle:
flatpak install --user dist/zashterminal_0.8.17.flatpak -y

# Run:
flatpak run org.leoberbert.zashterminal
```

### 📦 Debian Package (.deb - Ubuntu, Linux Mint, Debian)

For Debian-based systems:

```bash
# Install .deb package:
sudo apt install ./dist/zashterminal_0.8.17_all.deb
# Or: sudo dpkg -i ./dist/zashterminal_0.8.17_all.deb
```

### ⚡ Universal Installer & Hybrid Packaging (`install.sh`)

The `install.sh` script is modular and supports local installation, package building, and an interactive menu:

```bash
# Quick installation:
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash

# Or by cloning the repository:
git clone https://github.com/VaGNaroK/zashterminal-Fork.git
cd zashterminal-Fork

# Install on system:
./install.sh install

# Build .deb package:
./install.sh package deb --clean-cache

# Build Flatpak bundle:
./install.sh package flatpak --clean-cache

# Open interactive menu:
./install.sh menu
```

### Arch Linux / Manjaro

> [!IMPORTANT]
> The `zashterminal` package in the official AUR points to the legacy upstream repository. To get this enhanced Fork with all new features (Secure Agent Mode, VRAM Lifecycle, Flatpak), install by cloning the repository or using Flatpak:

```bash
git clone https://github.com/VaGNaroK/zashterminal-Fork.git
cd zashterminal-Fork
./install.sh install
```

### NixOS

On NixOS, use the project flake (`flake.nix` / `default.nix`):

```bash
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash
```

### WSL on Windows (Experimental)

```bash
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash
```

---

## 💻 Usage

```bash
zashterminal [options] [directory]
```

### Command Line Options

| Option | Description |
|---|---|
| `-w, --working-directory DIR` | Set initial working directory |
| `-e, -x, --execute COMMAND` | Execute command on startup |
| `--close-after-execute` | Close tab after command completes |
| `--ssh [USER@]HOST` | Connect directly to an SSH host |
| `--new-window` | Open in a new window instead of a tab |

### Examples

```bash
# Open in project directory
zashterminal ~/projects

# Connect to SSH server
zashterminal --ssh user@server.example.com

# Run command and close on exit
zashterminal --close-after-execute -e "htop"
```

---

## ⚙️ Configuration Files

Configurations are stored in `~/.config/zashterminal/`:

| File / Folder | Description |
|---|---|
| `settings.json` | General preferences, appearance, shortcuts, and AI settings |
| `sessions.json` | Saved SSH/SFTP connections and folders |
| `session_state.json` | Tab states and session restoration data |
| `layouts/` | Saved window layouts and split pane configurations |
| `backups/` | Staged file backups and manifests |

---

## 🤝 Contributing

Contributions, bug reports, and pull requests are welcome!

1. Fork the repository.
2. Create your feature branch (`git checkout -b feature/my-feature`).
3. Commit your changes (`git commit -m 'feat: add feature'`).
4. Push to your branch (`git push origin feature/my-feature`).
5. Open a Pull Request.

---

## 📄 License

This project is licensed under the **GNU General Public License v3 (GPLv3)** — see the [LICENSE](LICENSE) file for details.

---

## 👏 Acknowledgments & Credits

- **Original Project:** This repository is an enhanced fork of [Zashterminal](https://github.com/leoberbert/zashterminal), originally created by **Leonardo Berbert**.
- Special thanks to the developers and communities of **GNOME**, **GTK**, **libadwaita**, **VTE**, and **Pygments**.
