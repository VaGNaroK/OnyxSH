# Changelog

Todas as mudanças notáveis no projeto **OnyxSH** a partir de 13 de Agosto de 2026 estão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

---

## [0.10.0] - Em Desenvolvimento

### Adicionado
- **Gerenciador Visual de Túneis SSH e Port Forwarding (Local, Remote e Dynamic SOCKS5)**: Interface gráfica dedicada e serviço autônomo em background para gerenciamento e monitoramento em tempo real de túneis SSH (`src/onyxsh/terminal/tunnel_manager.py`, `src/onyxsh/ui/dialogs/tunnel_manager_dialog.py`, `src/onyxsh/ui/dialogs/tunnel_edit_dialog.py`).
  - 🌐 **Suporte aos 3 Modos de Encaminhamento SSH**:
    - 🟢 **Local Port Forwarding (`-L`)**: Redirecionamento de portas locais para serviços e bancos de dados remotos através da conexão SSH.
    - 🔄 **Remote Port Forwarding (`-R`)**: Exposição de portas de serviços locais diretamente no servidor SSH remoto.
    - 🛡️ **Dynamic Port Forwarding SOCKS5 (`-D`)**: Criação instantânea de proxy SOCKS5 local para roteamento de tráfego de rede criptografado via SSH.
  - 🎛️ **Diálogo de Gerenciamento Visual (`TunnelManagerDialog`)**:
    - Interface Libadwaita com barra de busca integrada por nome, porta, host ou tipo.
    - Status em tempo real com chips coloridos (🟢 Ativo / 🟡 Conectando / 🔴 Parado / ⚠️ Erro).
    - Switches `Gtk.Switch` para ligar e desligar túneis com 1 clique sem necessidade de terminal interativo.
    - Botão de cópia rápida de endereço local (`localhost:PORT`) para a área de transferência.
    - Botão de parada global para encerrar todos os túneis ativos de uma só vez.
  - 📝 **Editor de Túneis com Validação Rigorosa (`TunnelEditDialog`)**:
    - Diálogo com `Adw.ComboRow` e campos reativos contextuais que ocultam ou adaptam portas e hosts dependendo do tipo selecionado.
    - Opção de inicialização automática junto com a sessão SSH (`auto_start`).
  - ⚙️ **Integração com Sessões SSH e Command Palette**:
    - Integração de `port_forwardings` na aba de opções de conexão do `SessionEditDialog`.
    - Atalho direto no Menu Principal e indexação com palavras-chave de busca na Command Palette (`Ctrl + Shift + P`).
  - 🔄 **Serviço de Background com Auto-Detecção de Falhas (`SSHTunnelManager`)**:
    - Processos autônomos `ssh -N -T` com monitoramento de integridade via `GLib.timeout_add_seconds` e sinais GObject em tempo real.
  - 🌐 **Internacionalização Completa (28 Idiomas)**: 980 novas traduções sincronizadas e compiladas em todos os 28 idiomas.
- **Modo Proteção de Produção (Production Guard)**: Sistema abrangente de salvaguardas visuais, operacionais e de privacidade para sessões e servidores de produção (`src/onyxsh/terminal/production_guard.py`, `src/onyxsh/ui/widgets/production_banner.py`, `src/onyxsh/ui/dialogs/production_confirm_dialog.py`).
  - 🛡️ **Banner Visual de Produção**: Banner persistente de alta visibilidade em degradê crimson no topo das abas conectadas a ambientes de produção (`ProductionBanner`), com indicação visual de host/sessão, badge de guarda ativa e popover explicativo das políticas de segurança ativas.
  - 🔒 **Identificação Visual na Árvore de Sessões e Abas**: Exibição instantânea do badge `🛡️` ao lado de pastas e sessões marcadas como Produção na barra lateral e nas abas de terminal ativas.
  - ⚙️ **Configuração de Ambiente de Produção**: Novos switches dedicados nos diálogos de criação/edição de sessões (`SessionEditDialog`) e pastas (`FolderEditDialog`), com suporte a herança de ambiente em pastas.
  - 🛑 **Interceptação Automática de Comandos Destrutivos de Alto Risco**: Interceptação em tempo real no terminal ao pressionar `Enter` antes da execução de operações de risco crítico (`ProductionGuard`):
    - Destruição de arquivos e diretórios: `rm -rf`, `rm -fr`, `rm -r`, `shred -u`, `wipefs`.
    - Formatação e manipulação de blocos brutos de disco: `mkfs.*`, `dd of=/dev/...`, `fdisk`, `gdisk`, `parted`.
    - Desligamento, reinicialização e parada do sistema: `shutdown`, `reboot`, `poweroff`, `halt`, `init 0/6`.
    - Parada e desativação de serviços críticos: `systemctl stop/disable/mask`, `service ... stop/restart`.
    - Operações destrutivas em bancos de dados: `DROP DATABASE`, `TRUNCATE TABLE`, `DROP SCHEMA`.
    - Operações forçadas de Git: `git reset --hard`, `git clean -fd`, `git push --force`.
    - Suporte automático à remoção de prefixos escaladores (`sudo`, `doas`, `pkexec`, `nohup`, `env`, `time`).
  - 🔐 **Diálogo Modal de Confirmação Dupla (`ProductionConfirmDialog`)**: Bloqueio da execução exigindo que o usuário digite o nome exato do host ou sessão antes de desbloquear a execução em produção, com cancelamento seguro via `Esc` ou botão de abortar (`Ctrl+C` enviado ao shell).
  - 🕵️ **Proteção de Privacidade & Redação Automática no Assistente de IA**: Anonimização e ofuscação automática de segredos, senhas, tokens e chaves de API (`redact_secrets`) antes de enviar comandos ou saídas de terminais para modelos de IA externos.
- **Identidade Visual e Melhorias na Janela Principal**:
  - 🏷️ **Renomeação do Título Principal**: Atualização do nome da aplicação na barra superior (`Adw.WindowTitle`) e em todos os diálogos de restauração para **OnyxSH** (substituindo antigas referências a *Terminal Zash*).
- **Aprimoramentos no Histórico Enriquecido de Comandos**:
  - ⌨️ **Novo Atalho Padrão (`Ctrl + H`)**: Migração do atalho do Histórico de Comandos de `Ctrl + R` para `Ctrl + H` (padrão consagrado em navegadores web), liberando o `Ctrl + R` para a busca reversa interativa nativa do readline/bash no terminal.
  - 🗑️ **Opção de Limpeza com Confirmação (`CommandHistoryDialog`)**:
    - Botão dedicado de exclusão/limpeza com ícone `user-trash-symbolic` na barra superior do diálogo.
    - Diálogo modal de confirmação com opções para *Limpar Não Favoritos*, *Limpar com Falha*, e *Limpar Absolutamente Tudo*.
    - Suporte ao atalho rápido de teclado `Ctrl + Shift + Delete` para disparar a limpeza.
    - Inclusão do atalho do Histórico de Comandos no catálogo do diálogo de Atalhos de Teclado (`ShortcutsDialog`).
- **Otimizações de Desempenho e Eliminação de Engasgos**:
  - ⚡ **Remoção de I/O de Log Síncrono por Tecla**: Eliminação do logging síncrono por caractere digitado no terminal manager.
  - ⏱️ **Ajuste Fino do Debounce de Autocomplete**: Calibração do timer de debounce de 70ms para 180ms, garantindo digitação fluida sem bloqueio da UI thread enquanto o usuário digita.
  - 🚀 **Throttling no Detector de Autenticação Gateway**: Debounce de 250ms na extração de texto de 60 linhas para SSH, eliminando sobrecarga de CPU durante streams de alta taxa de dados (`top`, `tail -f`, `cat`, etc.).
  - 🎯 **Cache do Gerenciador de Configurações no PTY Proxy**: Cache estático do `SettingsManager` no `HighlightedTerminalProxy`, eliminando importações redundantes no loop de leitura de pacotes PTY.

## [0.9.0] - 2026-08-18

### Adicionado
- **Autocomplete Inteligente de Comandos e Sugestões Inline (Ghost Text & Popup Specs)**: Novo motor nativo de predição e autocompletar de comandos em tempo real (`src/onyxsh/terminal/completion/`).
  - 🛠️ **Catálogo de Specs Linux**: Dicionário curado de comandos e opções com explicações ricas em linguagem natural para `cd`, `pwd`, `cp`, `mv`, `touch`, `cat`, `less`, `head`, `tail`, `clear`, `tree`, `df`, `du`, `free`, `htop`, `top`, `ps`, `which`, `whereis`, `echo`, `nano`, `vim`, `ln`, `uname`, `history`, `ping`, `wget`, `scp`, `zip`, `unzip`, `gzip`, `gunzip`, `sudo`, `apt`, `systemctl`, `journalctl`, `docker`, `git`, `ssh`, `curl`, `tar`, `ufw`, `ip`, `rsync`, `ss`, `chmod`, `chown`, `mkdir`, `rm`, `ls`, `grep`, `find`, `kill`, `pkill`.
  - 📚 **Sugestões a partir do Histórico SQLite**: Priorização inteligente de comandos frequentes no mesmo diretório e host.
  - 🧩 **Templates de Snippets**: Sugestão de comandos parametrizados a partir do gerenciador de snippets.
  - ⚡ **Popup Flutuante Ancorado ao Cursor**: Widget popover moderno com ícones e descrições, navegável com `↑` e `↓`, e confirmação em 1 tecla com `Tab` ou `Enter`.
  - 🛡️ **Segurança Semântica com OSC 133**: Ativação restrita ao prompt de comando, desativando-se automaticamente em editores e ferramentas interativas (`vim`, `nano`, `top`, `fzf`).
  - ⚙️ **Painel de Configurações nas Preferências (`F2`)**: Controle total para habilitar/desabilitar fontes e autocompletar.
- **Busca Avançada no Scrollback do Terminal**: Modernização completa da barra de busca do terminal (`Ctrl + Shift + F`) com botões compactos e elegantes estilo flat para **`Aa`** (Diferenciar Maiúsculas e Minúsculas), **`\b`** (Palavra Inteira / Whole Word) e **`.*`** (Expressões Regulares / Regex). Exibição de contagem de correspondências totais em tempo real (`1/14` ou `Nenhum resultado`), navegação por teclado (`Enter` para próxima correspondência, `Shift + Enter` para anterior e `Escape` para fechar) e atalho de exportação rápida na própria barra.
- **Exportação do Terminal em Múltiplos Formatos (`.txt`, `.log`, `.md`, `.html`, `.cast`)**: Novo subsistema modular `TerminalExporter` e diálogo modal Libadwaita `ExportTerminalDialog` permitindo exportar o buffer completo do terminal ou apenas o texto selecionado. Suporte nativo a 5 formatos profissionais:
  - 📄 **Texto Puro (`.txt`)**: Saída do terminal limpa e sem formatação.
  - 📋 **Arquivo de Log (`.log`)**: Saída estruturada com cabeçalho de metadados de sessão, host, diretório `$PWD`, data/hora e dimensões.
  - 📝 **Documento Markdown (`.md`)**: Formatado com tabela de metadados e bloco de código ````bash ... ```` pronto para documentação e wikis.
  - 🌐 **Página HTML Estilizada (`.html`)**: Página web autônoma com tema dark moderno, tipografia monospace, metadados e botão integrado de cópia em 1 clique.
  - 🎬 **Gravação Asciinema v2 (`.cast` / `.asciinema`)**: Formato de gravação JSON v2 compatível com reprodução no terminal (`asciinema play`) e web players.
- **Integração Exclusiva da Exportação de Terminal**: Acesso simplificado e intuitivo via Menu Principal (Hambúrguer), Menu de Contexto do Terminal (botão direito) e Command Palette (`Ctrl + Shift + P`), com internacionalização completa de cabeçalhos e metadados.
- **Notificações Desktop Nativas para Comandos Longos**: Disparo automático de notificações do sistema operacional (via D-Bus e portal `Gio.Notification`) quando comandos demorados terminarem em segundo plano ou em abas inativas. Exibição de badge de status (`✅ Sucesso (0)` ou `❌ Falha (código)`), nome do comando, duração formatada (`⏱ duration`) e diretório/host. Notificação interativa que, ao ser clicada na central de notificações do desktop, traz o OnyxSH para o primeiro plano e seleciona automaticamente a aba onde o comando foi concluído. Configurações completas nas Preferências (`F2`) para threshold em segundos, condição de foco e alertas sonoros.
- **Gerenciador de Snippets de Comandos Reutilizáveis Parametrizados**: Suporte completo a templates de comandos com variáveis customizáveis (`{{variavel}}` e `{{variavel=valor_padrao}}`), resolução contextual automática de variáveis de sistema (`{{cwd}}`, `{{host}}`, `{{user}}`, `{{date}}`, `{{time}}`, `{{datetime}}`, `{{git_branch}}`, `{{clipboard}}`, `{{selection}}`), diálogo interativo de preenchimento de parâmetros com preview de sintaxe em tempo real (`SnippetParameterDialog`), inserção no prompt (`Tab`), execução direta (`Enter`) e indexação na Command Palette (`Ctrl + Shift + P`).
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
- **Correção de Posicionamento do Autocomplete com Scrollback e `Ctrl+L`**: O cálculo da posição vertical (`y`) do balão de autocomplete agora desconta o offset de rolagem do VTE (`vadjustment`), garantindo que o popup seja ancorado exatamente na linha visual do cursor (inclusive no topo da janela após limpar a tela com `Ctrl+L` ou comando `clear`).
- **Fechamento Automático do Autocomplete em Atalhos de Controle, Scroll e Perda de Foco**: O balão de sugestões agora é fechado imediatamente ao pressionar combinações com `Ctrl` ou `Alt` (`Ctrl+L`, `Ctrl+C`, `Ctrl+U`, `Ctrl+D`, etc.), ao rolar o terminal com a roda do mouse ou scrollbar, ao clicar no terminal ou ao alternar o foco para outra janela/aba.
- **Eliminação de Warnings Gdk-CRITICAL e Limpeza de Saída de Terminal**: Adicionada validação estrita de mapeamento (`get_realized()`, `get_mapped()`) e clamping seguro das coordenadas de apontamento do cursor dentro da área visível do terminal, eliminando avisos de `gdk_monitor_get_geometry` no console, além de silenciar mensagens residuais de debug de notificações.
- **Eliminação de Bloqueio de Digitação no Popup de Autocomplete**: Configurado o popover do autocomplete como estritamente não-modal e não-capturador de foco (`autohide=False`, `focusable=False`, `can_focus=False`), garantindo que o terminal VTE receba continuamente todas as teclas digitadas pelo usuário (como `cd`, `ls`, etc.) de forma totalmente fluida e sem interrupções. A tecla `Tab` ou `→` aceita a sugestão, enquanto a tecla `Enter` fecha o popup e executa o comando digitado no terminal.
- **Suporte Multilíngue Completo nas Sugestões de Autocomplete (28 Idiomas)**: Internacionalização e sincronização de 2.772 novas entradas de tradução em todos os 28 arquivos `.po` e compilação nos catálogos binários `.mo`. As explicações de comandos e subcomandos do Linux (`sudo`, `apt`, `systemctl`, `journalctl`, `docker`, `git`, `ssh`, `curl`, `tar`, `ufw`, `ip`, `rsync`, `ss`, `chmod`, `chown`, etc.) agora são traduzidas dinamicamente em runtime para o idioma ativo do usuário (`CompletionItem.get_description()`).
- **Correção de Importação do Módulo `pathlib` em `DesktopNotifier`**: Adicionado `import pathlib` em `desktop_notifier.py`, eliminando o erro de execução `name 'pathlib' is not defined` durante o envio de notificações nativas de conclusão de comandos longos.
- **Correção da API Gio.Notification (`set_default_action_and_target`)**: Correção do nome do método de ação padrão no PyGObject (`set_default_action_and_target`), eliminando a exceção `AttributeError` e liberando o disparo imediato da notificação desktop ao término dos comandos longos.
- **Interceptação Direta de Eventos Semânticos no Fluxo Binário do PTY**: Implementação de parser em tempo real (`_handle_semantic_stream_bytes`) no proxy do PTY (`HighlightedTerminalProxy`), capturando sequências OSC 133 (`\033]133;C\007`, `\033]133;D\007`) e `__zt_sem__` diretamente no canal de I/O do processo. Isso elimina qualquer interferência de personalizações de `$PS1` no `.bashrc`, filtros de títulos de janela ou coalescência de propriedades do VTE.
- **Botão de Teste de Notificação e Diagnóstico em Tempo Real**: Adicionado botão interativo *Testar Notificação Agora* nas Preferências (`F2` ➔ *Terminal* ➔ *Notificações de Comandos*) e telemetria de log detalhada (`[NOTIF-DEBUG]`) no terminal com flush imediato para diagnóstico de ciclo de vida e estado de foco de janelas/abas.
- **Envio Duplo e Resiliente de Notificações em Flatpak**: Implementado despacho de notificações combinando o portal `Gio.Notification` (com suporte a clique para focar a aba de origem) com fallback automático via `flatpak-spawn --host notify-send` garantindo 100% de compatibilidade e visibilidade em qualquer ambiente de desktop Linux (GNOME, KDE, XFCE, Cinnamon, MATE, i3).
- **Correção no Matching de Início de Execução de Comandos**: Ajustado o tratamento do evento `C_` nos listeners de URI do terminal, garantindo que o início da medição de tempo (`start_time`) seja registrado com precisão no momento do envio do comando no prompt.
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
