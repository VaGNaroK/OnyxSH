# Changelog

Todas as mudanças notáveis no projeto **Zashterminal (Fork)** a partir de 13 de Agosto de 2026 estão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

---

## [0.9.0] - Em Desenvolvimento

### Adicionado
- **Rebranding Oficial do Projeto para OnyxSH (`io.github.vagnarok.OnyxSH`)**: Transição completa de identidade visual, namespace e empacotamento para OnyxSH (Onyx Shell & SSH). Novo Application ID Flathub-ready, novo comando executável (`onyxsh`), novos manifestos, empacotamento modernizado e migração automática e transparente de preferências e histórico de `~/.config/zashterminal` para `~/.config/onyxsh` sem perda de dados.
- **Histórico Enriquecido de Comandos com Busca Fuzzy (`Ctrl + R`)**: Persistência estruturada em SQLite (`CommandHistoryManager`) registrando metadados completos de cada comando (texto, `$PWD`, host/sessão SSH, código de saída, duração, timestamps, contagem de execuções). Interface modal estilo Spotlight (`CommandHistoryDialog`) com busca fuzzy em tempo real, filtros dinâmicos (*Todos*, *Diretório Atual*, *Host Atual*, *Favoritos ⭐*), comandos favoritos fixados (Pin), inserção no prompt (`Tab`), execução imediata (`Enter`), atalhos de teclado e integração total com a Command Palette.
- **Integração Semântica com Shell (OSC 133 / Semantic Prompts)**: Rastreamento inteligente do ciclo de vida de comandos no shell (início de prompt, início de comando, início de execução e código de saída), medição de tempo de execução (ex: `⏱ 1.4s`) e badge visual de status no cabeçalho do painel.
- **Navegação Rápida entre Prompts (`Alt + Up` / `Alt + Down`)**: Salto instantâneo do scroll do terminal entre os prompts de comandos anteriores e posteriores.
- **Cópia Cirúrgica da Saída do Último Comando**: Extração e cópia direta para a área de transferência apenas do texto produzido pelo último comando executado.
- **Ação Rápida de Diagnóstico de Erros com IA**: Botão inteligente e ação na Command Palette que detecta código de saída com falha (`exit_code != 0`) e abre o chat de IA com o comando e a mensagem de erro já formatados para diagnóstico e correção.
- **Restauração Automática e Inteligente de Sessões (Session Restore)**: Capacidade de salvar e restaurar o estado completo do terminal entre fechamentos e inicializações, incluindo abas, layouts em split, diretórios correntes `$PWD`, sessões SSH (com auto-reconexão configurável), foco da aba ativa e visibilidade dos painéis de IA e sessões.
- **Opções de Inicialização nas Preferências**: Novos controles de restauração (*Sempre restaurar*, *Perguntar ao iniciar via Toast*, *Nunca restaurar*), auto-reconexão remota SSH e restauração de painéis da interface.
- **Command Palette (`Ctrl + Shift + P`)**: Interface modal spotlight para busca rápida e execução instantânea de todas as ações do terminal, abas, divisão de telas, sessões SSH, assistente de IA, regras de realce e preferências com suporte a busca fuzzy, navegação por teclado (`Up`/`Down`/`Enter`) e atalhos customizáveis.
- **Indexação Dinâmica de Sessões SSH na Command Palette**: Pesquisa rápida por nome, host ou usuário com conexão direta em 1 clique ou via teclado.
- **Sincronização Multilíngue Automatizada (28 Idiomas)**: Criação de `scripts/sync_translations.py` integrado aos fluxos de compilação Flatpak e DEB, garantindo a sincronização e tradução em lote de todas as novas mensagens e ações para todos os 28 idiomas suportados pelo Zashterminal (`en`, `es`, `fr`, `de`, `it`, `zh`, `ja`, `ru`, `pt`, etc.).

### Corrigido
- **Captura e Extração de Linha de Comando no Histórico**: Transmissão do texto completo de cada comando executado via base64 nos hooks de shell (`precmd`) com fallback no buffer VTE (`get_last_command_text`), e normalização de URI de CWD garantindo a exibição e busca instantânea no diálogo de Histórico de Comandos.
- **Eliminação de Warnings do VTE (`get_text_range_format`)**: Migração do extrator de texto semântico em `SemanticTracker` para a API moderna `get_text_range_format(Vte.Format.TEXT)`, eliminando avisos de depreciação do VTE GTK4 no terminal.
- **Acionamento do Chat de IA e Formatação do Badge Semântico**: Correção da abertura do painel de chat de IA ao clicar no botão ✨ do badge de erro semântico via `ui_builder.show_ai_panel()`, e otimização visual para não exibir `0ms` em comandos com erro de execução instantânea.
- **Isolamento de Eventos Semânticos com OSC 6 (`current-file-uri-changed`)**: Migração da captura de marcadores semânticos para canal OSC 6 dedicado, prevenindo que o prompt padrão `$PS1` do Linux (que redefine `window-title` para `user@host:dir`) descarte os eventos de término e início de comandos no shell.
- **Injeção de Hooks Semânticos no Host via Flatpak e Overlay em Aba Individual**: Garantida a injeção correta dos scripts de inicialização de hooks OSC 133 para comandos iniciados via `host-spawn` / `flatpak-spawn`, e adicionado suporte a badge flutuante em abas de terminal único.
- **Importação do Módulo Gdk em PreferencesDialog**: Adicionada a importação de `Gdk` em `preferences_dialog.py`, eliminando o `NameError: name 'Gdk' is not defined` ao fechar a tela de Preferências com a tecla `Escape`.
- **Atalho Universal Exclusivo para Preferências (`F2`)**: Configuração da tecla de função `F2` como o atalho padrão e universal em qualquer layout de teclado (ABNT2, US, etc.), com interceptação direta na janela e registro na Command Palette.
- **Métodos `get_active_tab` e `get_active_tab_index` no TabManager**: Implementação direta dos métodos para acesso seguro à aba ativa durante a serialização de estado da janela (`save_session_state`), prevenindo `AttributeError` no fechamento do aplicativo.
- **Interceptação Global do Atalho de Preferências (`Ctrl + ,`)**: Captura em nível de janela com `Gtk.PropagationPhase.CAPTURE`, evitando que o terminal VTE absorva o atalho e garantindo abertura instantânea em qualquer aba.
- **Tradução da Aba Perfis e Dados (`Profiles and Data`)**: Internacionalização completa do título da aba nos 28 idiomas suportados.
- **Ajuste de Rótulos no Dropdown de Inicialização**: Rótulos compactos e elegantes (*Sempre restaurar*, *Perguntar ao iniciar*, *Nunca restaurar*) evitando truncamento e reticências no popover do Libadwaita.
- **Preservação de Sessão ao Sair pelo Menu Principal**: O botão *Sair* do menu hambúrguer agora fecha as janelas sequencialmente, acionando o salvamento de sessão e confirmações pendentes.
- **Fechamento por Tecla Escape (Padrão VS Code)**: Suporte completo à tecla `Escape` para fechar imediatamente a Command Palette (`Ctrl + Shift + P`) e o diálogo de Preferências.
- **Confirmação no Modo Perguntar ao Iniciar**: Exibição de diálogo modal claro ao iniciar o aplicativo quando o modo *Perguntar se deseja restaurar a sessão anterior* estiver ativado, permitindo restaurar abas ou iniciar limpo.
- **Expansão Visual do Grupo de Inicialização**: Reformulação visual do grupo de inicialização nas Preferências com títulos, subtítulos descritivos e margens generosas.
- **Highlighting de Código JSON e Logs no Chat de IA**: Correção de look-behind de largura variável (`re.error: look-behind requires fixed-width pattern`) em tempo de execução no Python < 3.12 e ajuste de parâmetros no `ThreadSafeLogger.warning`.
- **Execução Direta de Ações na Command Palette**: Vinculação direta aos métodos do `WindowActions`, `tab_manager` e da janela principal, garantindo que `Enter` ou clique acionem imediatamente a ação selecionada (como abrir o chat de IA, criar abas ou dividir telas).
- **Escopo de Variável de Tradução na Command Palette**: Correção de `UnboundLocalError` em `_build_catalog` causado por shadowing da função `_()` durante o desempacotamento de sessões salvas.

---

## [0.8.18] - 2026-08-16


### Adicionado
- **Arquitetura Híbrida de Empacotamento**:
  - `scripts/build_deb.sh`: Criação automatizada de pacotes `.deb` com suporte a `--clean-cache`.
  - `scripts/build_flatpak.sh`: Construção de bundles Flatpak com o manifesto GNOME 46 (`manifests/org.leoberbert.zashterminal.yaml`) e limpeza de cache.
  - `install.sh`: Novos subcomandos `package deb`, `package flatpak`, `package all` e opções de instalação direta de bundles no menu interativo.
- **Pipeline de CI/CD para GitHub Actions**: Workflow `.github/workflows/build-packages.yml` para compilar e anexar `.deb` e `.flatpak` automaticamente nas releases do GitHub.
- **Migração Automática de Configurações no Flatpak**: Importação transparente de configurações do host (`~/.config/zashterminal/`) para o sandbox Flatpak na primeira execução.
- **Detecção de GPU e VRAM no Flatpak**: Consulta de hardware do host via portal `flatpak-spawn --host` (`org.freedesktop.Flatpak`), permitindo que a GPU (NVIDIA `nvidia-smi`, AMD/Intel) e VRAM sejam detectadas perfeitamente em Flatpak.
- **Integração Nativa do Shell do Host no Flatpak**: Execução do shell do usuário (`bash`, `zsh`, `fish`) diretamente no sistema hospedeiro via módulo integrado `host-spawn` (v1.6.2), habilitando suporte total a elevação de privilégios (`sudo`), gerenciadores de pacotes (`apt`, `pacman`, `dnf`), ferramentas CLI e dotfiles do usuário.
- **Exportação de Conversas do Assistente de IA**: Novo menu de exportação no cabeçalho do chat com suporte para salvar a conversa completa em **Markdown (.md)**, **JSON (.json)** ou **Copiar para a Área de Transferência**, facilitando o compartilhamento e a depuração de prompts, planos de ação e respostas do modelo.
- **Otimização de Raciocínio e Engenharia de Prompts na IA**: Novo System Prompt com consciência de terminal ativo no Zashterminal, uso obrigatório de caminhos dinâmicos (`$HOME`, `~`), criação de scripts atômica via heredoc (`cat << 'EOF'`) e priorização estrita de padrões modernos do Linux, eliminando comandos redundantes ou legados.
- **Relatório Arquitetural e Funcional para Agentes de IA**: Criação do arquivo `PROJECT_REPORT.md` detalhando arquitetura de software, subsistemas de PTY/host-spawn, segurança Zero-Trust do agente, fluxos de empacotamento e diretrizes para manutenção automatizada.
- **Roadmap e Backlog Técnico Estruturado (TODO)**: Criação de `TODO.md` com planejamento e priorização de futuras funcionalidades (Command Palette, Restauração de Sessões, Semantic Prompts OSC 133, Production Guard, Túneis SSH visuais, Multi-Host Exec e API de Plugins).





### Corrigido
- **Módulo VTE no Flatpak**: Inclusão e compilação do módulo `vte-0.76` (com suporte a GTK4 e GObject Introspection) no manifesto Flatpak para sanar o erro `Namespace Vte not available`.
- **Localização e Traduções (PT-BR)**: Busca dinâmica de catálogos gettext em `/app/share/locale` e diretórios internos do app, além de compilação explícita para a variante `pt_BR`.
- **Diretório Inicial do Terminal (CWD)**: Correção no launcher `usr/bin/zashterminal` e no spawner para garantir que novas abas e janelas abram no `$HOME` do usuário em vez do caminho interno de sandbox do app.
- **Identificação do Sistema Operacional Hospedeiro na IA**: Resolução automática do `os-release` real do sistema hospedeiro (`/var/run/host/os-release` ou via `flatpak-spawn`), evitando que a IA identifique incorretamente o ambiente como o runtime container do Flatpak.
- **Alocação de Pseudo-Terminal (PTY) e Teclado no Flatpak**: Gerenciamento de PTY real no host via `host-spawn`, eliminando avisos de `ioctl` do bash, garantindo correto funcionamento de teclas especiais (`Delete`, `Backspace`, setas) e entrada segura de senhas no `sudo`.
- **Sincronização do Realce de Digitação em Tempo Real**: Reset imediato do buffer de realce de digitação em sequências de escape (setas, `Delete`, `Backspace`) e bloqueio de injeção retroativa de cursor fora da digitação inicial, corrigindo sobreposição e corrupção de caracteres na tela.
- **Bypass Interativo de Edição no Prompt**: Repasse direto e sem buffer para eventos interativos de teclado sem quebra de linha (setas, `Delete`, `Backspace`, autocompletar), corrigindo duplicação de redesenho do Readline e garantindo exclusão instantânea de caracteres com a tecla `Delete`.
- **Isolamento de Teclas de Edição do GtkIMContext no Flatpak**: Envio direto da tecla `Delete` (`\x1b[3~`) para o PTY do terminal via `feed_child` na fase de captura de eventos e concessão de permissões de D-Bus para `IBus` e `Fcitx`, impedindo que o método de entrada do GTK interprete o Delete como tecla morta de composição.
- **Contraste de Cores no Painel do Assistente de IA em Tema Claro**: Adaptação dinâmica das cores de blocos de comando, cartões de ação do agente e paletas de realce sintático (Pygments e Fallback) para garantir contraste elevado e perfeita legibilidade tanto no tema claro quanto no escuro.





---

## [0.8.17] - 2026-08-15

### Adicionado
- **Detecção de Hardware e GPU**: Nova função `detect_gpu_info()` em `platform.py` com detecção automática de GPUs NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs) ou memória RAM do sistema.
- **Seletor de Janela de Contexto**: Opção na interface do Modo Agente Seguro com dropdown entre 4K, 8K, 16K, 32K, 64K e 128K tokens, recomendando automaticamente o melhor perfil baseado na VRAM detectada.
- **Buffer de Contexto no Ollama (`num_ctx`)**: Repasse dinâmico de `options.num_ctx` no pré-carregamento e nas chamadas de inferência local.
- **Retenção Inteligente de Histórico**: Algoritmo de janela deslizante token-aware no `TerminalAiAssistant` para preservar o histórico de conversas longas sem exceder o limite de contexto do modelo.
- **Ciclo de Vida de VRAM (Pré-carregamento Assíncrono e Liberação)**: Carregamento do modelo local em background no startup do terminal e descarregamento automático da VRAM ao fechar o app.

---

## [0.8.16] - 2026-08-14

### Corrigido
- Escapamento de caracteres especiais (`&`) em títulos de janelas e diálogos para garantir compatibilidade total com o renderizador de markup Pango do GTK4.

---

## [0.8.15] - 2026-08-14

### Adicionado
- Seletor gráfico de níveis de risco do Modo Agente e switches de contexto de sistema (OS/distro/hardware) e diretório de trabalho (`PWD`) no diálogo de configuração da IA.
- Botões de ação rápida para cópia de blocos de scripts e resposta completa do assistente de IA.

---

## [0.8.14] - 2026-08-14

### Corrigido
- Extrator de blocos JSON com suporte a parsing flexível via regex para respostas da IA sem tags markdown strict.
- Aumento do limite de tokens de geração para permitir a emissão de scripts e diagnósticos extensos sem cortes.

---

## [0.8.13] - 2026-08-14

### Corrigido
- Remoção de filtros hardcoded que bloqueavam solicitações legítimas de geração de código e scripts de administração.

---

## [0.8.12] - 2026-08-14

### Melhorado
- Refinamento do prompt de sistema do Assistente de IA para garantir explicações didáticas e ordenação causal lógica de comandos de terminal.

---

## [0.8.11] - 2026-08-14

### Corrigido
- Ajuste de quebra natural de texto (`natural wrap mode`) nos rótulos do GTK4 para evitar avisos de cálculo de layout em redimensionamentos.

---

## [0.8.10] - 2026-08-14

### Corrigido
- Garantia de thread-safety com despachos via `GLib.idle_add` durante o streaming de respostas da IA.
- Adição de handlers globais para logging resiliente de exceções não capturadas.

---

## [0.8.9] - 2026-08-14

### Corrigido
- Correção de chamada de função no status do instalador `install.sh`.

---

## [0.8.8] - 2026-08-13

### Adicionado
- **Início do Fork do Zashterminal**:
  - **Modo Agente Seguro**:
    - Motor de políticas (`PolicyEngine`) estratificado por níveis de risco (Nível 0 a Nível 4).
    - Registro de ferramentas (`ToolRegistry`) para execução controlada e auditada de comandos.
    - Auditoria completa em formato JSONL com trilha de ações executadas pela IA.
    - Suporte a elevação segura via Polkit (`pkexec`) com dry-run e diffs de backup para arquivos de configuração.
  - **Documentação Multilíngue**: `README.md` em português como documentação principal e `README.en.md` em inglês.
