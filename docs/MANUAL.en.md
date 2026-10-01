# Changelog

Todas as mudanças notáveis no projeto OnyxSH serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

### Built-in Command Specs Catalog
Declarative specifications for 50+ essential tools:
- **System:** `apt`, `dnf` (Fedora/RHEL, aliases `yum`/`microdnf`), `systemctl`, `journalctl`, `ufw`.

### Built-in Command Specs Catalog (Multi-Distro)
Declarative specifications for 58 essential tools across distributions:
- **System:** `apt`, `dnf`, `yum`, `pacman`, `systemctl`, `journalctl`, `ufw`.
- **Containers & Network:** `docker`, `ssh`, `curl`, `ping`, `ip`, `ss`, `rsync`.
- **Files & Utilities:** `tar`, `chmod`, `chown`, `find`, `grep`, `mkdir`, `rm`, `ls`, `cp`, `mv`, `cat`.
- **Performance:** `htop`, `top`, `ps`, `df`, `du`, `free`, `kill`.

### Your System Commands (PATH)
Beyond the curated catalog, the engine detects executables from your machine's `PATH` (own scripts, installed programs, shortcuts) with a 60s cache. Disable with `autocomplete_system_enabled: false`.

### Browse Mode (Ctrl + Space)
- With an empty prompt, press <kbd>Ctrl</kbd> + <kbd>Space</kbd> to list all possible commands.
- With a command typed (e.g. `dnf `), lists all its subcommands and flags.

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
| <kbd>Ctrl</kbd> + <kbd>Space</kbd> | Browse commands/subcommands (autocomplete browse) | Terminal |
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
