# Auditoria de Paridade: OnyxSH Legado (Python) vs. Port em Rust (`v0.14.0`) e `TODO.md`

Este documento compara sistematicamente o código legado em Python ([`src/onyxsh/`](file:///home/vagnarok/OnyxSH/src/onyxsh/)) e o roadmap em [`TODO.md`](file:///home/vagnarok/OnyxSH/TODO.md) com o estado real da implementação em Rust ([`crates/`](file:///home/vagnarok/OnyxSH/crates/)) na versão `0.14.0` ([`Cargo.toml`](file:///home/vagnarok/OnyxSH/Cargo.toml)).

> [!IMPORTANT]
> **Critério de Avaliação (Runtime vs. Biblioteca/Shell):**
> Diversos módulos do port em Rust existem como código de biblioteca testado unitariamente ou possuem janelas/diálogos declarados na UI, mas **não estão conectados ao fluxo real de execução** em [`crates/onyxsh-app/`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/) (ou utilizam stubs/simulações locais). Esta auditoria separa explicitamente o que funciona **End-to-End** do que é **Parcial / Sem Wiring** ou **Mock / Stub**.

---

## 1. Resumo Executivo e Arquitetura

### 1.1 Diferença Estrutural de Stack e Dependências

- **Planejado vs. Realizado:** O plano inicial em [`RUST_MIGRATION_PLAN.md`](file:///home/vagnarok/OnyxSH/RUST_MIGRATION_PLAN.md) previa `relm4` + `tokio`, mas o workspace atual ([`Cargo.toml`](file:///home/vagnarok/OnyxSH/Cargo.toml)) utiliza GTK4 + Libadwaita + VTE4 puros (`gtk4`, `libadwaita`, `vte4`) com gerenciamento de estado estilo Elm (`AppModel` em `state.rs`), threads `std::thread` e timers GLib.
- **Crates do Workspace (`0.14.0`):**
  - `onyxsh-core` (configurações, sessões, crypto, i18n, temas, snippets, backup)
  - `onyxsh-terminal` (spawner, semantic tracker, OSC 7/52, highlighter, autocomplete, broadcast, runbook, exporter, production guard, SSH health, túnel)
  - `onyxsh-filemanager` (backend local, transferências, quick look, checksums, TFTP)
  - `onyxsh-agent` (smart router, policy engine, path guard, redactor, client, plan parser, verifier, audit, context manager, error matcher)
  - `onyxsh-system` (diagnósticos, coleta `/proc` local, GPU/disco)
  - `onyxsh-admin-helper` (binário auxiliar privilegiado)
  - `onyxsh-app` (interface GTK4/Adwaita/VTE4 principal)
- **Ausências Críticas no `Cargo.toml`:** Não há dependência HTTP (`reqwest`, `ureq`, etc.), nem runtime assíncrono (`tokio`), nem `sqlite` (`rusqlite`), nem biblioteca de compressão `.7z`. Consequentemente, toda a camada de rede/IA HTTP e persistência SQLite do Python não possui transporte ativo no runtime Rust atual.

### 1.2 Diagnóstico Geral por Camada

| Subsistema | Status Geral no Rust | Observação Principal |
| :--- | :--- | :--- |
| **Janela, Temas, Menus & Preferências** | 🟢 Funcional (com bugs pontuais) | 30 paletas, 4 modos GTK, `FontSizerWidget`, barra `/proc` ao vivo, `adw::PreferencesWindow` |
| **i18n (28 idiomas)** | 🟢 Paridade Total | 1.937 chaves Rust 100% traduzidas nos 28 `.po` / 29 `.mo` + parser `.mo` puro |
| **Terminal VTE (Busca, Export, Palette, Atalhos)** | 🟢 Funcional / 🟡 Parcial | Busca PCRE2, Command Palette, Global Search e Atalhos funcionam; Splits, OSC 133/7/52, Highlighter e Production Guard estão parciais/desconectados |
| **Sessões SSH, Auth, Tunnels & Health** | 🟡 Parcial / 🔴 Stub | CRUD básico funciona; falta Keyring/decrypt, `-i`/`sshpass`, `Include` SSH, SFTP na aba, Tunnels reais e SSH Health |
| **File Manager Local & Checksums** | 🟢 Funcional | 3 modos locais (Lista, Grade, Árvore), filtros, Quick Jump, ações no terminal e Checksums completos |
| **File Manager SSH/SFTP, Dual-Pane, Queue, Quick Look & TFTP** | 🟡 Parcial / 🔴 Stub | Apenas `LocalFsBackend`; Dual-Pane local/local; fila/compare/TFTP são stubs ou incompletos |
| **Assistente de IA & Provedores** | 🔴 UI + Biblioteca (Stub em Runtime) | `handle_submission` chama `synthesize_agent_response()` local/simulado; `client.rs` nunca é chamado; zero HTTP |
| **Sistema, Diagnóstico, Backup & Restore** | 🟡 Parcial | Monitor local `/proc` funciona (dropdown SSH sem sinal); `window_state.rs` não conectado; backup JSON plano |
| **Empacotamento & Updater** | 🟡 Parcial / 🔴 Faltando | Manifesto Flatpak Rust existe; `build_deb.sh`/`build_flatpak.sh` ainda apontam para Python; sem Auto-Updater |

---

## 2. Auditoria Completa do `TODO.md`

Comparação item a item dos blocos marcados e pendentes no [`TODO.md`](file:///home/vagnarok/OnyxSH/TODO.md):

### 2.1 Itens Marcados como Concluídos (`[x]`) no `TODO.md`

| Item `TODO.md` | Descrição | Status no Python Legado | Status Real no Port Rust (`0.14.0`) |
| :--- | :--- | :--- | :--- |
| **1.1** | Visualizador de Arquivos com Tabs (Modo Lista) | 🟢 Implementado (Local + SFTP) | 🟡 **Parcial:** Funciona para arquivos locais (`ColumnView`, `GroupByType`), mas sem SFTP remoto e sem `df` remoto |
| **1.2** | Visualização em Grade e Árvore Hierárquica + Métrica de Pastas | 🟢 Implementado (recursivo assíncrono) | 🟡 **Parcial:** Grade (`GridView`) e Árvore (`ListBox` até profundidade 10) funcionam localmente; contagem de pastas é síncrona e rasa (1 nível) |
| **1.3** | Transferência Dual-Pane com Barra de Progresso | 🟢 Implementado (Local↔SFTP, Drag&Drop, fila) | 🟡 **Parcial / Stub:** Abre apenas Local (`/home`) ↔ Local (`/tmp`), sem SFTP, sem Drag&Drop (`DragSource`/`DropTarget`), `btn_history` e `btn_compare_dirs` não abrem diálogos de fila/diff |
| **1.4** | Quick Look e Editor de Arquivos Integrado | 🟢 Implementado (Pygments, Zoom/Pan, Hex, SSH, `sudo -S`) | 🟡 **Parcial:** Texto/imagem local básico funciona; faltam syntax highlighting, destaque de logs (`ERROR`/`WARN`), dump hex, Zoom/Pan, leitura/salvamento SSH e fallback `sudo -S` (só `pkexec`) |
| **1.5** | Integração File Manager ↔ Terminal | 🟢 Implementado | 🟢 **Implementado (Local):** `Copy Path`, `Insert in Prompt`, `cd`, `Run`, `Run with sudo`, `tail -f` e envio à IA (`Explain`, `Diagnose`, `Audit`) funcionam |
| **1.6** | Servidor TFTP Integrado para Redes/Switches | 🟢 Implementado (diálogo, leitura/escrita multi-bloco) | 🔴 **Stub:** `tftp.rs` faz apenas read-only de 1 bloco (512B sem loop ACK/WRQ); menu apenas faz `bind` em `127.0.0.1:0` sem disparar thread loop; sem `TftpServerDialog`; atalho é stub |
| **1.7** | Command Toolbar & Broadcast Routing | 🟢 Implementado (13 `FieldType` + seletor granular) | 🟡 **Parcial:** Toolbar, 5 tipos de campo (`Text`, `Password`, `Number`, `Switch`, `Dropdown`) e `BroadcastRouter::AllTerminals` funcionam; faltam 8 tipos de campo e seletor granular de alvos |
| **1.8** | Command Palette (`Ctrl+Shift+P`) | 🟢 Implementado | 🟢 **Implementado:** [`terminal_ux.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/terminal_ux.rs) e [`window.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/window.rs) unificam ações, sessões e comandos salvos |
| **1.9** | Busca Global Multi-Abas (`Ctrl+Shift+G`) | 🟢 Implementado | 🟢 **Implementado:** [`global_search_and_shortcuts.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/global_search_and_shortcuts.rs) busca em abas VTE, sessões e histórico |
| **2.1** | Production Guard (Proteção de Produção) | 🟢 Implementado | 🟡 **Parcial:** Banner existe no `HeaderBar`, mas digitar `Enter` no VTE não é interceptado em sessões de produção; confirmação não exige digitar o hostname; execução no painel IA não bloqueia comandos destrutivos |
| **2.2** | SSH Health Monitor (Latência RTT e Auto-Reconexão) | 🟢 Implementado | 🔴 **Sem Wiring:** Biblioteca existe (`ssh_health.rs`), mas sem uso em `onyxsh-app` (sem polling RTT, badges, auto-reconexão ou `SSHErrorBanner`) |
| **2.3** | Gerenciador Visual de Túneis SSH | 🟢 Implementado | 🔴 **Stub na UI:** [`tunnel_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/tunnel_ui.rs) salva CRUD em `tunnels.json`, mas o `Switch` só altera texto `"active"`/`"stopped"` sem iniciar processos `ssh -N -L/-R/-D` |
| **2.7** | Verificação de Integridade / Checksum | 🟢 Implementado | 🟢 **Implementado (Local):** `checksum_dialog.rs` suporta `MD5`, `SHA1`, `SHA256`, `SHA512`, progresso assíncrono, cancelamento, Hash Matcher e exportação Markdown |
| **3.1** | Assistente de IA Multi-Provedor + Ollama VRAM | 🟢 Implementado | 🔴 **Stub em Runtime:** [`ai_assistant_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/ai_assistant_ui.rs) chama `synthesize_agent_response()` local; [`client.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-agent/src/client.rs) não tem HTTP real; sem teste de chave, sem descoberta de modelos, sem controle de VRAM Ollama |
| **3.2** | Detecção Proativa de Erros com IA | 🟢 Implementado | 🔴 **Sem Wiring / Passivo:** `error_matcher.rs` só é acionado no menu de contexto de 6 linhas em [`terminal_ux.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/terminal_ux.rs); não há monitoramento contínuo de saída VTE nem toast proativo |
| **3.3** | Agente Autônomo Zero-Trust (Policy, Redactor, PathGuard, Audit, Diff) | 🟡 Infraestrutura em `agent/` (chat usa VTE direto) | 🔴 **Parcial / Desconectado:** `SecretRedactor` e `SmartRouter` rodam no chat simulado, porém `PathGuard` ignora `ai_agent_allowed_roots`, `ContextManager` (`<untrusted>`) não é chamado, `AuditLogger::append` não grava execuções reais e `show_diff_review_and_audit_dialog` é dead code com demo em `/tmp` |
| **3.4** | Plano de Ação Estruturado (`ActionPlan`) | 🟢 Implementado | 🟡 **Parcial:** Exibe cartões com botão individual `"Aprovar & Executar"`, mas faltam checkboxes de seleção, `"Executar Tudo"`, `"Apenas Diagnósticos"`, `"Passo-a-Passo"`, `"Parar"`, barra de progresso e modo Dry-Run |
| **3.5** | Auto-Verificação Pós-Execução + Auto-Correção | 🟢 Implementado | 🔴 **Sem Wiring:** `verifier.rs` cobre apenas `systemctl`/`nginx` e nunca é executado nem dispara auto-correção após rodar um passo |
| **3.7** | Catálogo de Quick Prompts + Prompt Editor | 🟢 Implementado (200+ prompts e editor) | 🔴 **Stub:** `quick_prompts_data` está fixo em 6 itens em [`ai_assistant_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/ai_assistant_ui.rs), sem editor de prompts customizados |
| **4.2** | Sistema de Diagnóstico & Logs | 🟢 Implementado | 🟡 **Parcial:** Diálogo GUI existe, mas não testa Ollama ao vivo, não lê/sanitiza logs recentes nem exporta `.json`/`.md`; `logger.rs` não grava arquivo (`log_to_file`); CLI não possui `--diagnose` |
| **5.1–5.5** | UI/UX Principal (Temas, Sidebar, Busca VTE, Splits, Histórico) | 🟢 Implementado | 🟢/🟡 **Misto:** Temas, Menus, Busca VTE e FontSizer funcionam; Splits abrem sempre **nova aba** com 2 terminais horizontais (não divide painel atual nem verticalmente); histórico não usa SQLite |
| **5.6** | Exportação de Sessão + Caderno de Bordo (Runbook) | 🟢 Implementado | 🟢 **Implementado (com ressalva):** Exporta `.txt`, `.log`, `.md`, `.html`, `.cast` e gera Runbook editável, mas usa `parse_fallback_steps` sem exit codes/durações por falta do `SemanticTracker` |
| **5.7** | Atalhos de Teclado Customizáveis (34 ações) | 🟢 Implementado | 🟢 **Implementado:** Captura visual, detecção de conflitos, Limpar e Restaurar Padrões para 34 ações em [`window.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/window.rs) e [`global_search_and_shortcuts.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/global_search_and_shortcuts.rs) |
| **5.8** | Monitoramento de Recursos + GPU VRAM | 🟢 Implementado | 🟡 **Parcial:** 4 cards Cairo + GPU VRAM + Disk I/O local funcionam em [`resource_dashboard.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/resource_dashboard.rs), mas `host_dd` (SSH) não tem sinal `connect_selected_notify` e falta `% de uso de partição (df -k /)` |
| **5.9** | Importadores (SSH Config, PuTTY, Terminator, URIs) | 🟢 Implementado (+ SecureCRT) | 🟡 **Parcial:** Falta suporte a `Include` globs no `~/.ssh/config`, ocultação de hosts ignorados, importador SecureCRT, e `parse_connection_uri` (`importers.rs`) não é usado na UI; MobaXterm INI citado em docstring não existe |
| **5.10** | Backup Criptografado, Keyring & Restauração de Sessão | 🟢 Implementado (`.7z` + `py7zr` + Keyring + Restore) | 🔴 **Parcial / Incompatível:** `backup.rs` grava JSON **sem criptografia** (apenas SHA-256) e referencia `window_state.json` enquanto a sidebar grava `state.json`; `KeyringManager` e `decrypt_session_password` nunca são chamados pela UI; `window_state.rs` não restaura/salva sessão ao abrir/fechar |

### 2.2 Itens em Aberto (`[ ]`) no `TODO.md`

Todos os itens abaixo permanecem abertos em [`TODO.md`](file:///home/vagnarok/OnyxSH/TODO.md) (tanto no legado quanto no Rust, com uma exceção parcial no item `1.10`):

- **1.10 Sixel Graphics Support:** `preferences_ui.rs` / `window.rs` aplicam `set_enable_sixel` no widget `vte4::Terminal`, mas sem pipeline avançado de renderização/inspeção de imagens Sixel descrito no backlog.
- **1.11** Compartilhamento de Snippets via Link Seguro.
- **1.12** Terminal Multiplayer / Sessões Colaborativas.
- **2.4** Multi-Host Command Execution (execução orquestrada em grupo de servidores com relatório consolidado).
- **2.5** SFTP Remote Diff (comparação real de conteúdo entre arquivos/diretórios remotos).
- **2.6** Audit Log com cadeia de hashes verificável (`hash_prev` / verificação criptográfica em `audit.jsonl`).
- **2.8** Production Guard com leitura obrigatória + justificativa registrada em auditoria.
- **2.9** Anomaly Detection (detecção comportamental de picos/anomalias em sessão ou sistema).
- **2.10** Integração com Cofres de Senhas Externos (Bitwarden CLI, 1Password CLI, `pass`).
- **3.6** Onyx Bridge — Plugin API para extensões de terceiros.
- **3.8** Receitas Guiadas de Agente (Guided Agent Recipes).
- **3.9** Demonstration Workflow Learning (gravação de fluxo humano transformado em automação reutilizável).
- **3.10** Integração Completa com **OpenCode CLI** (em Rust ainda não há checagem de binário, abertura em split no `$PWD` nem backend headless).
- **4.1** Expansão da Suíte de Testes (segurança zero-trust, E2E e integração VTE/UI).
- **4.3** Relatório de Saúde do Sistema exportável em HTML (`System Health Report`).
- **4.4** Shadow Mode (sandbox dry-run de execução antes de aplicar no host real).

---

## 3. Matriz Detalhada por Subsistema (Python vs. Rust)

### 3.1 Core, CLI, Configurações, Temas, i18n, Crypto & Backup

1. **CLI e Instância Única ([`crates/onyxsh-app/src/main.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/main.rs)):**
   - **Já implementado:** Parsing básico de `--tab`, `--version`, `--working-directory`, `--execute`.
   - **O que falta:** Apenas `working_directory` é repassado para `build_main_window`; `--tab` e `--execute` não executam ação; não existe flag `--diagnose`; `ApplicationFlags::NON_UNIQUE` desativa instância única via DBus (`new_instance_behavior`).
2. **Persistência e Migração de Configurações (`config.rs`, `preferences_ui.rs`):**
   - **Já implementado:** Carregamento/salvamento JSON e 4 abas em `adw::PreferencesWindow`.
   - **O que falta / Incompatibilidades:**
     - **Wrapper JSON do Python:** Python salva `{"metadata": ..., "settings": {...}}`; o `config.rs` em Rust lê JSON plano e não desenvelopa o formato legado.
     - **Colisão em `filemanager_ui.rs`:** `load_bookmarks` / `save_bookmarks` buscam `v["settings"]["file_manager_bookmarks"]`, chave inexistente no JSON plano do Rust.
     - **Chaves sem consumidor em runtime:** Novas preferências adicionadas para paridade (restore, `log_to_file`, `ssh_health`, erro proativo, toggles de autocomplete) são salvas no JSON, mas ainda não governam o comportamento da aplicação.
     - **Migração Flatpak / Zashterminal:** Ausente no Rust.
3. **Temas & Paletas (`theme_engine.rs`, `window.rs`):**
   - **Já implementado:** 30 paletas built-in (`BUILTIN_SCHEMES` + `ONYX_DARK_PALETTE`), 4 modos GTK (`System`, `Light`, `Dark`, `TerminalMatch` via `adw::StyleManager`), CSS dinâmico com opacidade/transparência.
   - **Bugs e lacunas:**
     - **Bug verificado:** `AppSettings::default().color_scheme` usa `"onyx-dark"` (hífen), enquanto `theme_engine.rs` espera `"onyx_dark"` (underscore).
     - **Editor de Esquemas Customizados (`custom_schemes.json`):** O botão de salvar (`save_btn`) descarta as alterações em vez de persistir esquemas customizados.
4. **Criptografia, Keyring, Restauração de Sessão e Backup ([`crates/onyxsh-core/src/crypto.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-core/src/crypto.rs)):**
   - **Já implementado:** Criptografia de senha de sessão V2 (`encrypt_session_password`) ao salvar sessão SSH; `KeyringManager` (`secret-tool`) e `WindowSessionState` implementados como módulos de biblioteca.
   - **O que falta:** `decrypt_session_password` e `KeyringManager` nunca são chamados pela UI ao conectar/editar sessões; `WindowSessionState::load_from_path` não possui chamadores em produção (sem restauração `always`/`ask`/`never` ao iniciar e sem salvamento no `connect_close_request`); falta CRUD de layouts nomeados (`layouts/*.json`); `backup.rs` gera JSON plano sem criptografia (Python usa `.7z` com `py7zr` + `passwords.json` do Keyring) e referencia `window_state.json` enquanto a sidebar grava `state.json`.
5. **Internacionalização (28 idiomas):**
   - **Já implementado (100%):** Parser `.mo` puro em Rust (`onyxsh-core/src/i18n.rs`), avaliador de expressões plurais, 29 catálogos `.mo` em `src/onyxsh/locale/`, e auditoria automática via [`scripts/sync_translations.py`](file:///home/vagnarok/OnyxSH/scripts/sync_translations.py) (`--audit-rust`: **1.937 chaves Rust únicas**, 100% presentes e traduzidas nos 28 arquivos `.po` e 29 diretórios `.mo`).

---

### 3.2 Sessões SSH, Autenticação & Importadores

- **Já implementado:** Sidebar hierárquica de pastas/sessões ([`session_sidebar.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/session_sidebar.rs)), diálogo de sessão, conexão SSH básica (`ssh -p port [-X] user@host`) no `spawn_session_cb` em [`window.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/window.rs), importação base de `~/.ssh/config`, PuTTY e Terminator.
- **O que falta:**
  - **Autenticação e Opções Avançadas SSH:** `spawn_session_cb` ignora `auth_type`/`auth_value` (chave `-i`, senha via `sshpass`/Keyring), `ControlMaster`/`KeepAlive`, `post_login_command`, abertura automática de túnel e abertura automática de aba SFTP.
  - **Protocolos Telnet/Serial:** Existe parser URI em `importers.rs` e opção de UI, mas caem no shell local (nota: o Python legado também não implementava spawner real para Telnet/Serial).
  - **Operações na Sidebar:** Faltam mover sessão entre pastas, clonar/duplicar sessão, editar/renomear/remover pastas e `InlineContextMenu` completo na barra lateral.
  - **Importadores:** Falta resolver `Include` globs no `~/.ssh/config`, ocultação de hosts ignorados, importador SecureCRT, e ligação na UI de `importers::parse_connection_uri`. (Nota: o comentário mencionando MobaXterm `.ini` não possui implementação correspondente).

---

### 3.3 Terminal, Spawner VTE, Rastreamento Semântico & Segurança

1. **Spawner e Bypass Flatpak ([`crates/onyxsh-terminal/src/spawner.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-terminal/src/spawner.rs), [`window.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/window.rs)):**
   - **Gap crítico:** `spawner.rs` define scripts de shell integration (OSC 133/7) e `ProcessTracker`, porém `create_vte_terminal` em `window.rs` **não usa `spawner.rs`**, não injeta hooks bash/zsh e não faz bypass de sandbox Flatpak via `host-spawn` / `flatpak-spawn`.
2. **Splits e Abas (`window.rs`):**
   - `btn_split` cria sempre uma **nova aba** contendo dois terminais novos em `Paned::Horizontal` — não divide o terminal focado em linha; `split-vertical` não divide verticalmente; `close-pane` fecha a aba inteira; falta mover painel para nova aba e drag-and-drop para reorganizar abas.
3. **Semântica OSC 133 / OSC 7 / OSC 52, Notificações & Highlighter:**
   - `semantic_tracker.rs`, `osc7.rs`, `osc52.rs` e `git_assistant::evaluate_command_notification` (`Gio.Notification`) não estão conectados ao VTE em `onyxsh-app`.
   - Atalhos `jump-previous-prompt` / `jump-next-prompt` apenas rolam ±5 linhas no `vadjustment` sem usar marcadores OSC 133.
   - Faltam spinner/cronômetro de execução na aba, "Copiar saída do último comando", badge de erro na aba para envio à IA e `SelectionActionPopover` flutuante ao selecionar texto no terminal.
   - `highlighter.rs` não está ligado a nenhum proxy PTY; as abas 2 e 3 de [`highlight_dialog_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/highlight_dialog_ui.rs) (51 perfis `.json` e regras por comando/regex) são apenas UI sem persistência.
4. **Autocomplete ([`terminal_ux.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/terminal_ux.rs)):**
   - **Já conectado:** `Popover` flutuante ligado ao `CompletionEngine` funciona em digitação VTE.
   - **O que falta:** Recebe histórico vazio (`&[]`), não completa snippets nem comandos remotos SSH, não verifica `OSC 133` para restringir ao prompt (dispara dentro de `vim`/`nano`) e não respeita `settings.autocomplete_enabled`.
5. **Production Guard & SSH Tunnels & SSH Health:**
   - **Production Guard:** Banner + diálogo de teste existem em [`terminal_ux.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/terminal_ux.rs), mas falta interceptação da tecla `Enter` no VTE, confirmação digitando o hostname e bloqueio de comandos destrutivos vindos da IA.
   - **SSH Tunnels ([`tunnel_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/tunnel_ui.rs)):** CRUD e `tunnels.json` funcionam; `Switch` é visual apenas (não inicia `ssh -N`).
   - **SSH Health (`ssh_health.rs`):** Módulo inteiro sem chamadores no app.
6. **Command Manager & Broadcast (`command_manager_ui.rs`, `broadcast.rs`, [`src/onyxsh/data/command_manager_models.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/data/command_manager_models.py)):**
   - Rust implementa 5 dos 13 `FieldType` do Python (`Text`, `Password`, `Number`, `Switch`, `Dropdown`), faltando `TextArea`, `Slider`, `FilePath`, `DirectoryPath`, `DateTime`, `Color`, `Radio` e `MultiSelect`, além do seletor granular de alvos de broadcast.

---

### 3.4 File Manager, Transferência, Quick Look & TFTP

- **Já implementado (Núcleo Local):**
  - 3 modos de visualização locais em `filemanager_ui.rs` (Lista `ColumnView`, Grade `GridView` com miniaturas, Árvore `ListBox`).
  - Navegação, Breadcrumbs clicáveis, Quick Jump + `git root` dinâmico, toggle de arquivos ocultos, filtro recursivo, Copiar/Recortar/Colar/Renomear/Excluir/Mkdir/Chmod local.
  - Ações de contexto no terminal + botões de análise por IA (`Explain`, `Diagnose`, `Audit`).
  - `checksum_dialog.rs` com paridade local completa (MD5/SHA1/SHA256/SHA512, Hash Matcher, exportação `.md`, comando no terminal — assim como no Python, checksums são locais).
- **O que falta / Incompleto:**
  - **Backend SSH/SFTP ([`crates/onyxsh-filemanager/src/backend.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-filemanager/src/backend.rs)):** Apenas `LocalFsBackend` implementado em runtime (`parse_sftp_ls_line` é apenas teste unitário); sem navegação/edição/transferência SFTP, sem `symlink_target` em `FileEntry`, sem espaço livre remoto (`df`).
  - **Dual-Pane e Fila de Transferências:** Abre Local ↔ Local, sem Drag & Drop GTK4, sem diálogo real de fila (`btn_history` só escreve texto no rodapé), sem progresso assíncrono/cancelamento/verificação de espaço livre, e `btn_compare_dirs` não abre `DiffReviewDialog` de conteúdo.
  - **Quick Look & Editor (`quick_look_dialog.rs`):** Faltam syntax highlighting, destaque de `ERROR`/`WARN`, dump hex de binários, Zoom/Pan (`GestureZoom`, `GestureDrag`, Ctrl+Scroll, Fit/100%), edição SFTP e fallback `sudo -S`.
  - **TFTP (`tftp.rs`, `window.rs`):** Protocolo incompleto (read-only 1 bloco de 512B), bind sem loop de recebimento no menu e atalho stub.

---

### 3.5 Assistente de IA, Agente Zero-Trust & Admin Helper

> [!NOTE]
> **Nuance importante do código Python:** No código legado em Python, `AgentOrchestrator`, `ToolRegistry`, `FSTools`, `shell_tools.run_argv` e `AdminTools` também existem em `src/onyxsh/agent/`, mas em runtime o chat (`TerminalAiAssistant`/`AIChatPanel`) utiliza chamadas HTTP diretas aos provedores + `safe_feed_command` no VTE, não esse orquestrador.

- **Já implementado em Biblioteca / UI Shell (`crates/onyxsh-agent/`, `ai_assistant_ui.rs`, `ai_config_ui.rs`):**
  - `SmartRouter`, `TaskComplexityClassifier`, `PolicyEngine`, `PathGuard`, `SecretRedactor`, `ErrorMatcher`, `PlanParser`, `PostVerifier` (parcial), `AuditLogger`, `ContextManager` e builders/parsers SSE em `client.rs`.
  - Painel de chat lateral, diálogo de configuração multi-provedor (`ai_config_ui.rs`), histórico de conversas (`conversation_history_dialog.rs`), visualizador de auditoria (`audit_log_dialog.rs`) e diálogo de escopo (`agent_scope_dialog.rs`).
- **O que falta / Stubs em Runtime:**
  1. **Transporte HTTP real inexistente:** `ai_assistant_ui.rs` chama `synthesize_agent_response()`, retornando respostas estáticas baseadas em palavras-chave locais; `client.rs` nunca é invocado; sem streaming HTTP real para OpenAI, Anthropic, Gemini, Groq, OpenRouter, Ollama ou servidores OpenAI-compatible.
  2. **Configuração de IA sem efeito em API (`ai_config_ui.rs`):** `wire_key_test()` não faz requisição de teste; falta listagem dinâmica de modelos e preload/unload de VRAM no Ollama.
  3. **ActionPlan limitado:** Só há `"Aprovar & Executar"` por passo; faltam checkboxes, `"Executar Tudo"`, `"Apenas Diagnósticos"`, `"Passo-a-Passo"`, `"Parar"`, barra de progresso e modo Dry-Run.
  4. **Segurança e Auditoria desconectadas:** `PathGuard::default()` ignora `ai_agent_allowed_roots`; `ContextManager` (`<untrusted>`) não envolve arquivos enviados ao chat; `AuditLogger::append` não registra execuções reais; `show_diff_review_and_audit_dialog` é código morto usando `/tmp`.
  5. **Auto-Verificação (`verifier.rs`) e Quick Prompts:** `PostVerifier` só reconhece `systemctl`/`nginx` e nunca é executado; `quick_prompts_data` está fixo em 6 prompts (contra 200+ e editor no Python).
  6. **Admin Helper (`onyxsh-admin-helper`):** O binário existe em [`crates/onyxsh-admin-helper/src/main.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-admin-helper/src/main.rs), mas nunca é chamado pelo app e falta o arquivo `.policy` do Polkit.

---

### 3.6 Monitoramento, Diagnóstico, Empacotamento & Updater

- **Monitor de Recursos (`resource_dashboard.rs`):** 4 gráficos Cairo (CPU, Memória, Rede, Disco I/O) e VRAM de GPU locais funcionam em tempo real; entretanto, o seletor de host SSH `host_dd` **não possui sinal `connect_selected_notify`** (permanece sempre lendo `/proc` local) e falta exibir a porcentagem de ocupação de partição (`df -k /`).
- **Empacotamento:** O manifesto Rust existe em [`manifests/io.github.vagnarok.OnyxSH.rust.yaml`](file:///home/vagnarok/OnyxSH/manifests/io.github.vagnarok.OnyxSH.rust.yaml) com [`scripts/build_rust.sh`](file:///home/vagnarok/OnyxSH/scripts/build_rust.sh), porém [`scripts/build_deb.sh`](file:///home/vagnarok/OnyxSH/scripts/build_deb.sh) e [`scripts/build_flatpak.sh`](file:///home/vagnarok/OnyxSH/scripts/build_flatpak.sh) ainda empacotam a versão Python (`src/onyxsh/`).
- **Auto-Updater:** `utils/updater.py` (verificação de releases no GitHub) ainda não existe no port em Rust.

---

## 4. Priorização Sugerida para Atingir Paridade com o Legado Python

1. **Prioridade Alta (Funcionalidades Centrais Hoje Simuladas ou Quebradas):**
   - **Camada HTTP real na IA (`onyxsh-agent/src/client.rs` + `ai_assistant_ui.rs`):** Adicionar cliente HTTP síncrono/assíncrono (ex.: `ureq` ou `reqwest`), remover `synthesize_agent_response()`, conectar streaming SSE, teste de chave, descoberta de modelos e VRAM do Ollama.
   - **File Manager SSH/SFTP & Dual-Pane (`backend.rs` + `filemanager_ui.rs`):** Implementar `SftpFsBackend`, leitura/gravação remota no Quick Look, fila assíncrona real com progresso/cancelamento e `DiffReviewDialog` no `btn_compare_dirs`.
   - **Spawner VTE + Flatpak Bypass (`spawner.rs` + `window.rs`):** Ligar `host-spawn` / `flatpak-spawn`, hooks bash/zsh (OSC 133 / OSC 7) e suportar chaves `-i`, `sshpass`/Keyring e `ControlMaster` no `spawn_session_cb`.
   - **Correções de Persistência Críticas:** Corrigir `"onyx-dark"` → `"onyx_dark"`, desenvelopar `"settings"` legado no `config.rs`, alinhar `save_bookmarks`/`load_bookmarks`, unificar `state.json` vs `window_state.json`, e conectar `decrypt_session_password` + `KeyringManager`.
2. **Prioridade Média (Paridade Operacional e Segurança Ativa):**
   - **Terminal UX & Segurança:** Conectar `SemanticTracker` (cronômetro/spinner na aba, salto real entre prompts, copiar última saída), `ErrorMatcher` proativo, `ProductionGuard` na tecla `Enter` e na execução de planos da IA, e `PathGuard`/`AuditLogger`/`ContextManager` no fluxo da IA.
   - **DevOps & Redes:** Execução real `ssh -N` em [`tunnel_ui.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/tunnel_ui.rs), wiring do `ssh_health.rs`, sinal `host_dd` para métricas SSH remotas em [`resource_dashboard.rs`](file:///home/vagnarok/OnyxSH/crates/onyxsh-app/src/resource_dashboard.rs), splits reais (vertical/horizontal no painel atual), `Broadcast` granular e `TftpServerDialog` multi-bloco/WRQ.
   - **Restauração de Sessão & Histórico:** Salvar/restaurar abas ao fechar/abrir (`window_state.rs`) e histórico pesquisável em SQLite.
3. **Prioridade Baixa / Evolução (`TODO.md` em aberto):**
   - Backup criptografado (`.7z` ou formato equivalente seguro), Editor de Esquemas Customizados e Highlighter em tempo real, Catálogo de 200+ Quick Prompts + editor, Auto-Updater GitHub, adaptação de `build_deb.sh`/`build_flatpak.sh`, além dos novos épicos abertos em [`TODO.md`](file:///home/vagnarok/OnyxSH/TODO.md) (Multi-Host Exec, SFTP Diff, Audit Hash Chain, Cofres externos, OpenCode CLI, Shadow Mode, Relatório HTML).
