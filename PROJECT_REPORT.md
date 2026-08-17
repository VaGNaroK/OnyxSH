# Relatório Técnico Completo da Arquitetura e Funcionalidades do Zashterminal

> **Destinatário:** Agentes de IA, Desenvolvedores e Mantenedores do Projeto  
> **Projeto:** Zashterminal (Fork)  
> **Versão Atual:** `0.8.18` (Data de Referência: 16 de Agosto de 2026)  
> **Repositório:** [VaGNaroK/zashterminal-Fork](https://github.com/VaGNaroK/zashterminal-Fork)  
> **Tecnologias Centrais:** Python 3.12+, GTK4, Libadwaita, VTE 0.76+, Host-Spawn 1.6.2, Asyncio, PyGObject, GLib/Gio.

---

## 1. Visão Geral do Sistema

O **Zashterminal** é um emulador de terminal moderno, seguro e extensível desenvolvido para o ecossistema Linux/GNOME utilizando **GTK4** e **Libadwaita**. O projeto combina alto desempenho de emulação com recursos avançados de engenharia de sistemas, gerenciamento de infraestrutura SSH/SFTP, realce sintático interativo em tempo real e um **Assistente de IA / Modo Agente Autônomo com Arquitetura Zero-Trust**.

### Destaques do Projeto
- **Emulação de Terminal de Alta Fidelidade:** Alimentado pelo widget VTE compilado nativamente com suporte a GObject Introspection e GTK4.
- **Integração Nativa com Shell do Host (Flatpak Sandbox Bypass):** Uso de `host-spawn` para alocação de PTY real no host Linux, garantindo suporte completo a elevação de privilégios (`sudo`), comandos do host (`apt`, `pacman`, `dnf`, `systemctl`) e controle de jobs de shells modernos (`bash`, `zsh`, `fish`).
- **Assistente de IA & Agente Seguro:** Suporte a múltiplos provedores LLM (Groq, Gemini, OpenRouter, Modelos Locais via Ollama), com validação de políticas de segurança, prevenção contra prompt injection, revisão de diffs e registro de auditoria.
- **Gerenciamento Completo de Sessões:** Árvore de sessões SSH, gerenciador de arquivos SFTP integrado, servidor TFTP embutido e credenciais protegidas via `SecretService` / `GNOME Keyring`.
- **Distribuição Multiplataforma Híbrida:** Suporte completo a pacotes `.flatpak`, `.deb` (Debian/Ubuntu/Mint) e `PKGBUILD` (Arch Linux).

---

## 2. Estrutura e Mapeamento de Diretórios

```
zashterminal-Fork/
├── .github/workflows/         # Pipeline de CI/CD para compilação automatizada de pacotes
│   └── build-packages.yml
├── data/                      # Metadados de desktop, ícones, esquemas GSettings e AppStream
│   ├── icons/
│   ├── org.leoberbert.zashterminal.desktop
│   └── org.leoberbert.zashterminal.metainfo.xml
├── locale/                    # Catálogos de localização e tradução gettext (.po / .mo)
│   ├── pt.po                  # Português do Brasil (Referência primária)
│   └── [en, es, fr, de, ...]
├── manifests/                 # Manifestos de empacotamento Flatpak
│   └── org.leoberbert.zashterminal.yaml
├── scripts/                   # Automação de builds e sincronização
│   ├── build_deb.sh           # Construtor do pacote Debian (.deb)
│   ├── build_flatpak.sh       # Construtor do bundle Flatpak (.flatpak)
│   └── sync_version.py        # Sincronizador de versão entre arquivos do projeto
├── src/zashterminal/          # Código-fonte principal da aplicação
│   ├── admin/                 # Módulo de execução e elevação administrativa
│   │   └── admin_helper.py    # Bridge com subprocessos privilegiados
│   ├── agent/                 # Engine do Modo Agente Seguro de IA
│   │   ├── audit_logger.py    # Gravação e consulta de auditoria de ações (audit_log.json)
│   │   ├── context_manager.py # Engenharia de prompts e sanitização de tags untrusted
│   │   ├── diff_reviewer.py   # Análise e geração de diffs unificados antes da escrita
│   │   ├── path_guard.py      # Validação de escopos e travessia de diretórios seguros
│   │   ├── policy_engine.py   # Motor de regras, risco (0 a 3) e listas de permissão/bloqueio
│   │   ├── redactor.py        # Mascaramento de dados sensíveis e credenciais em logs
│   │   └── tools/             # Ferramentas executáveis pelo agente (fs_tools, shell_tools)
│   ├── core/                  # Tipos base, interfaces e constantes centrais
│   ├── data/                  # Gerenciamento de persistência de histórico e estilos
│   │   ├── ai_history_manager.py # Persistência de conversas do chat (ai_history.json)
│   │   └── styles/            # Folhas de estilo CSS dinâmicas (tema claro / escuro)
│   ├── filemanager/           # Navegador e gerenciador de arquivos SFTP/Local
│   ├── sessions/              # Gerenciador de sessões SSH, pastas, chaves e credenciais
│   ├── settings/              # Gerenciador de configurações, temas e esquemas de cores
│   │   ├── config.py          # Configurações padrão e constantes do aplicativo
│   │   └── manager.py         # SettingsManager (persistência em JSON em ~/.config/zashterminal/)
│   ├── state/                 # Gerenciamento de abas, painéis divididos (splits) e layout
│   ├── terminal/              # Camada de emulação VTE, PTY e realce sintático
│   │   ├── _highlighter_impl.py # Motor de realce sintático em tempo real e bypass Readline
│   │   ├── ai_assistant.py    # Cliente assíncrono de IA (Gemini, Groq, OpenRouter, Local)
│   │   ├── manager.py         # TerminalManager: ciclo de vida do PTY, keybindings e eventos
│   │   └── highlighter/       # Regras sintáticas, tokens, expressões regulares e proxies
│   ├── ui/                    # Interface gráfica GTK4 e Libadwaita
│   │   ├── actions.py         # Ações globais da aplicação (Gio.SimpleAction)
│   │   ├── dialogs/           # Diálogos modais (IA, Atalhos, Sessões, Diffs, Auditoria, TFTP)
│   │   ├── menus.py           # Menus de contexto e cabeçalho
│   │   ├── widgets/           # Widgets personalizados (AIChatPanel, SyntaxViews, Banners)
│   │   │   ├── ai_chat_panel.py # Painel de chat de IA, cartões de ação e exportação
│   │   │   └── conversation_history.py # Histórico de conversas anteriores
│   │   └── window_ui.py       # Montagem da janela principal (Adw.ApplicationWindow)
│   ├── utils/                 # Utilitários de sistema, criptografia, logging e tema
│   │   ├── platform.py        # Detecção de OS real, GPU/VRAM e spawners Flatpak
│   │   ├── theme_engine.py    # Engine de temas GTK e sincronização Dark/Light
│   │   ├── crypto.py          # Criptografia de senhas e backups
│   │   └── translation_utils.py # Wrapper Gettext internacionalizado
│   ├── app.py                 # Ponto de entrada Adw.Application e ciclo de vida
│   └── window.py              # ZashTerminalWindow (controlador principal da janela)
├── tests/                     # Suíte de testes unitários automatizados (52 testes)
├── CHANGELOG.md               # Registro cronológico detalhado de alterações
├── install.sh                 # Script universal de instalação e empacotamento
└── pyproject.toml             # Metadados do pacote Python (PEP 518/621)
```

---

## 3. Detalhamento dos Subsistemas

### 3.1. Terminal Engine & Gestão de PTY (`src/zashterminal/terminal/`)
- **Integração com o Host via `host-spawn` no Flatpak:**
  Quando executado dentro da sandbox Flatpak, o terminal não roda o shell restrito do container. Ele utiliza `host-spawn` (v1.6.2) conectado ao daemon `org.freedesktop.Flatpak` para invocar o shell padrão do usuário (`/etc/passwd` do host) com um PTY Linux real alocado. Isso viabiliza comandos como `sudo apt update` solicitando senhas de forma segura e suporte completo a sinais POSIX (`SIGINT`, `SIGTSTP`, `SIGWINCH`).
- **Bypass Interativo do Prompt de Comando:**
  Em `_highlighter_impl.py`, os eventos de digitação do usuário e sequências de controle sem quebra de linha (`\n`) são repassados imediatamente para o VTE (`term.feed(clean_data)`), eliminando qualquer atraso visual ou artefato de duplicação do Readline durante edições com as teclas `Delete`, `Backspace` e setas.
- **Tratamento Específico de Teclas de Edição no Flatpak:**
  Para evitar que o `GtkIMContext` (método de entrada do GTK) intercepte a tecla física `Delete` como uma tecla morta de composição em distribuições com IBus/Fcitx, o `manager.py` captura `Gdk.KEY_Delete` na fase de captura (`CAPTURE`) e injeta a sequência escape canônica `\x1b[3~` diretamente no PTY da sessão.
- **Rastreamento de Diretório em Tempo Real (OSC 7):**
  Implementação em `utils/osc7_tracker.py` que interpreta sequências OSC 7 emitidas pelo shell, mantendo o `$PWD` atualizado para que novas abas ou divisões (splits) abram exatamente no diretório de trabalho corrente da aba ativa.

---

### 3.2. Assistente de IA & Raciocínio (`ai_assistant.py` & `ai_chat_panel.py`)
- **Provedores de LLM Integrados:**
  - **Google Gemini:** `gemini-2.5-flash` (ou personalizado via API key).
  - **Groq:** `llama-3.1-8b-instant`, `mixtral-8x7b-32768`.
  - **OpenRouter:** Acesso a modelos como `openrouter/polaris-alpha`, `claude-3.5-sonnet`, `deepseek-r1`.
  - **Modelos Locais (Ollama / LocalAI):** Conexão direta via endpoint HTTP (`http://localhost:11434/v1`).
- **Engenharia de Prompts Avançada:**
  - **Consciência de Terminal Ativo:** A IA reconhece que o usuário já está no terminal Zashterminal e fornece fluxos estritamente CLI, sem instruções desnecessárias como *"Pressione Ctrl+Alt+T"*.
  - **Caminhos Dinâmicos:** Uso obrigatório de `$HOME` e `~`, proibindo caminhos fictícios como `/home/usuario/`.
  - **Criação Atômica de Scripts:** Geração de scripts via blocos heredoc (`cat << 'EOF' > ~/script.sh`) e `chmod +x` em vez de múltiplos comandos `echo >>`.
  - **Padrões Modernos:** Foco em `systemd`, `systemctl`, `journalctl`, `apt`, `flatpak`, `ss`, evitando ferramentas legadas como SysVinit (`/etc/init.d/`).
- **Exportação Completa de Conversas:**
  - 📄 **Markdown (`.md`):** Exporta o diálogo completo formatado, com metadados de sessão, papéis (*User*/*Assistant*), comandos bash e níveis de risco.
  - 📋 **JSON (`.json`):** Estrutura completa dos objetos de histórico para depuração e melhorias de prompt.
  - ✂️ **Área de Transferência:** Cópia rápida para o clipboard do sistema com notificação toast de confirmação.
- **Aparência Adaptativa:**
  - Realce sintático Pygments adaptativo com contraste ajustado: no tema claro, fundos suaves com texto escuro `#1f2328` e tokens coloridos; no tema escuro, esquema Dracula de alto contraste.

---

### 3.3. Modo Agente Seguro com Arquitetura Zero-Trust (`src/zashterminal/agent/`)
- **Anti-Prompt Injection:** Todo conteúdo não confiável (saídas do terminal, arquivos lidos do disco, dados de ambiente) é encapsulado na tag `<untrusted>...</untrusted>`, instruindo o LLM a tratar esses dados exclusivamente como texto bruto, nunca como comandos executáveis.
- **Motor de Políticas (`PolicyEngine`):**
  - Classificação de risco em 4 níveis:
    - `0 (Seguro):` Leitura de arquivos, listagens (`ls`, `cat`, `grep`, `pwd`).
    - `1 (Baixo):` Escritas em diretórios de trabalho temporários ou arquivos novos.
    - `2 (Médio):` Modificações de arquivos existentes, instalação de pacotes de usuário.
    - `3 (Crítico):` Ações administrativas (`sudo`, `rm -rf`, modificação de `/etc` ou serviços de sistema).
  - Listas de permissão e bloqueio configuráveis pelo usuário no diálogo de escopo do agente (`AgentScopeDialog`).
- **Guardião de Caminhos (`PathGuard`):**
  - Impede qualquer ataque de travessia de diretório (`../`), validando caminhos canônicos antes de qualquer operação de leitura ou escrita.
- **Revisão de Diffs e Staging (`diff_reviewer.py`):**
  - Modificações de arquivos geram propostas com diffs unificados antes de serem aplicadas, permitindo que o usuário inspecione e aprove as alterações visualmente no `DiffReviewDialog`.
- **Registro de Auditoria e Rollback (`audit_logger.py` & `audit_rollback.py`):**
  - Toda ação executada pelo agente é registrada com timestamp ISO, argumentos, status de execução e backup do arquivo original em `~/.config/zashterminal/audit/`, permitindo reversão (rollback) a qualquer momento via interface gráfica (`AuditLogDialog`).

---

### 3.4. Gerenciamento de Sessões, SSH e SFTP (`src/zashterminal/sessions/` & `filemanager/`)
- **Organização em Árvore Hierárquica:** Criação de pastas, subpastas e sessões personalizadas com ícones, cores e títulos.
- **Configurações Avançadas de SSH:**
  - Autenticação por Chaves SSH (RSA, Ed25519, ECDSA) e senhas criptografadas via `SecretService`.
  - Encaminhamento de portas (Port Forwarding / SSH Tunnels Local e Remoto).
  - Parâmetros customizados de terminal e variáveis de ambiente por sessão.
- **Gerenciador de Arquivos SFTP Integrado:**
  - Painel lateral visual para upload, download, renomeação, criação e exclusão de arquivos e diretórios remotos sem necessidade de ferramentas externas como FileZilla.
- **Servidor TFTP Embutido (`tftp_server_dialog.py`):**
  - Servidor TFTP configurável diretamente na interface para transferência de firmwares e configurações para switches, roteadores e dispositivos embarcados.

---

### 3.5. Configurações, Temas e Internacionalização (`src/zashterminal/settings/` & `utils/`)
- **Sincronização de Configurações:** As preferências do usuário são gravadas em JSON em `~/.config/zashterminal/settings.json`. Na primeira inicialização no Flatpak, o app migra automaticamente as configurações existentes no host para dentro do sandbox.
- **Gestão de Temas Dinâmica (`ThemeEngine`):**
  - Suporte automático a alternância Claro/Escuro do sistema GNOME.
  - Paletas de cores customizáveis para o terminal (Dracula, Solarized, Monokai, Nord, Gruvbox, etc.).
  - Controle dinâmico de fontes e tamanho de texto (`FontSizer`).
- **Detecção de Hardware do Host (`detect_gpu_info`):**
  - Detecta placas de vídeo NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs) e memória RAM no host, recomendando o tamanho ideal da janela de contexto de IA (4K até 128K tokens) conforme a VRAM disponível.
- **Internacionalização (Gettext):**
  - Suporte a 28 idiomas, com catálogo completo em Português do Brasil (`locale/pt.po`).

---

## 4. Estratégia de Empacotamento e Distribuição

| Formato | Arquivo Gerador / Manifesto | Características Principais |
| :--- | :--- | :--- |
| **Flatpak Bundle (`.flatpak`)** | `manifests/org.leoberbert.zashterminal.yaml`<br>`scripts/build_flatpak.sh` | Compilação do runtime VTE 0.76 com GTK4; integração `host-spawn` v1.6.2; permissões D-Bus para `Flatpak`, `IBus`, `Fcitx`, `Secret` e `Notifications`; isolamento com acesso ao host. |
| **Pacote Debian (`.deb`)** | `scripts/build_deb.sh` | Instalação nativa em `/usr/share/zashterminal` e `/usr/bin/zashterminal`; controle de dependências APT (`python3-gi`, `gir1.2-gtk-4.0`, `gir1.2-adw-1`, `gir1.2-vte-3.91`). |
| **Arch Linux (`PKGBUILD`)** | `PKGBUILD` | Instalação no Arch / Manjaro / EndeavourOS seguindo os padrões da AUR. |
| **Instalador Geral (`install.sh`)** | `install.sh` | Menu interativo com opções para compilar pacotes, instalar dependências e instalar localmente no sistema. |

---

## 5. Diretrizes e Regras para Agentes de IA

Ao manter ou estender este projeto, qualquer agente de IA deve seguir rigorosamente as seguintes regras:

1. **Atualização Mandatória do Changelog:**
   - **Toda modificação relevante commitada deve ser documentada em [CHANGELOG.md](file:///home/vagnarok/zashterminal-Fork-main/CHANGELOG.md)** na seção da versão corrente (`[0.8.18]`), sob as categorias apropriadas (`### Adicionado` ou `### Corrigido`).
2. **Execução de Testes Automatizados:**
   - Antes de concluir alterações, execute sempre a suíte de testes unitários:
     ```bash
     PYTHONPATH=src python3 -m unittest discover tests/
     ```
   - Todos os 52 testes devem passar com status `OK`.
3. **Preservação de PTY e Comportamento de Terminal:**
   - Nunca remova o bypass interativo de dados ou o tratamento explícito de teclas especiais em `terminal/manager.py` e `terminal/_highlighter_impl.py`.
4. **Respeito ao Sandbox Flatpak e Host-Spawn:**
   - Quaisquer novos utilitários CLI que precisem rodar no sistema hospedeiro devem usar o mecanismo de execução com fallback para `host-spawn` / `flatpak-spawn --host`.

---

*Relatório gerado e validado em 16 de Agosto de 2026 para sincronização técnica de agentes e desenvolvedores do projeto Zashterminal.*
