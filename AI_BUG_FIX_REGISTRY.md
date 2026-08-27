# 🧠 OnyxSH — Registro de Bugs Corrigidos & Base de Conhecimento para IA

> **Arquivo:** `AI_BUG_FIX_REGISTRY.md`  
> **Versão do Projeto:** v0.10.0  
> **Última Atualização:** Agosto/2026  
> **Finalidade:** Servir como guia definitivo e índice de consulta para agentes de IA e desenvolvedores, detalhando todos os bugs já identificados, diagnosticados e corrigidos no repositório.

---

## 📌 Instruções para Agentes de IA

Antes de propor diagnósticos, refatorações ou modificações no código do **OnyxSH**, o agente de IA **DEVE**:
1. **Consultar este registro** para verificar se o sintoma ou comportamento já foi abordado anteriormente.
2. **Evitar regressões arquiteturais:** Muitas soluções (ex.: caminhos do Flatpak, reciclagem de linhas no GTK4, manipulação de PTY, `AsyncTaskManager`) foram desenhadas para resolver problemas sutis de concorrência ou sandbox.
3. **Manter o padrão de testes obrigatório:** Qualquer nova funcionalidade ou correção deve ser acompanhada de testes unitários em `tests/` e passar 100% com:
   ```bash
   PYTHONPATH=src python3 -m unittest discover -s tests
   ```

---

## 🗂️ Índice por Categorias

- [1. Flatpak & Integração com o Host](#1-flatpak--integração-com-o-host)
- [2. Gerenciador de Arquivos & Quick Look](#2-gerenciador-de-arquivos--quick-look)
- [3. Assistente de IA, Parser de Scripts & Agent Mode](#3-assistente-de-ia-parser-de-scripts--agent-mode)
- [4. Terminal, Rastreamento Semântico & Atalhos](#4-terminal-rastreamento-semântico--atalhos)
- [5. Core, Async Tasks, Segurança & Infraestrutura](#5-core-async-tasks-segurança--infraestrutura)

---

## 1. Flatpak & Integração com o Host

### [BUG-FP-001] Resolução de Caminhos do Host em Salvamento com Sudo/Pkexec
- **Commit:** `a000c79`
- **Componente:** `src/onyxsh/filemanager/operations.py`
- **Sintoma:** Ao salvar arquivos de sistema (ex.: `/etc/hosts`) como Superusuário no Quick Look, ocorria o erro: `/usr/bin/tee: /run/host/monitor/hosts: Arquivo ou diretório inexistente`.
- **Causa Raiz:** O código executava `Path(file_path).resolve()`. No Flatpak, `/etc/hosts` é um link simbólico que aponta internamente para `/run/host/monitor/hosts`. Ao despachar o comando para o host via `flatpak-spawn --host pkexec tee <path>`, o caminho interno do sandbox não existia no host.
- **Correção:** Substituído por `os.path.abspath(str(file_path))` e limpeza de prefixos `/run/host/` e `/var/run/host/` para preservar o caminho canônico do host (`/etc/hosts`).
- **Testes:** `tests/test_quick_look.py` (`test_save_file_content_sudo_flatpak_path_resolution`).

---

### [BUG-FP-002] PTY Real no Host para Prompts Interativos de Sudo e Controle de Jobs
- **Commit:** `e0a9c39`
- **Componente:** `src/onyxsh/terminal/spawner.py`
- **Sintoma:** Comandos interativos com `sudo` no terminal embutido não exibiam o prompt de senha ou falhavam no controle de jobs do Bash.
- **Causa Raiz:** O spawner não alocava PTY com flags adequadas no host ao iniciar via `flatpak-spawn`.
- **Correção:** Configuração de alocação de pseudo-terminal real conectando VTE diretamente aos canais de I/O do processo no host.

---

### [BUG-FP-003] Detecção da Distribuição Real do Host vs Runtime do Sandbox
- **Commit:** `ac86ec0`
- **Componente:** `src/onyxsh/utils/platform.py`
- **Sintoma:** O assistente de IA sugeria comandos para o runtime do Flatpak (ex.: GNOME/Freedesktop SDK) em vez da distro do usuário (Debian, Arch, Fedora, Ubuntu).
- **Causa Raiz:** `/etc/os-release` era lido diretamente do filesystem do sandbox.
- **Correção:** Implementada consulta via `flatpak-spawn --host cat /etc/os-release` quando em sandbox Flatpak.

---

### [BUG-FP-004] Diretório Inicial do Shell no Home do Usuário
- **Commit:** `3b50aba`
- **Componente:** `src/onyxsh/terminal/spawner.py`
- **Sintoma:** Novos terminais abriam em diretórios internos do app em vez de `~`.
- **Causa Raiz:** Ausência de resolução explícita do `HOME` do host ao despachar o shell.
- **Correção:** Garantida a inicialização do shell padrão no `$HOME` do usuário do host.

---

### [BUG-FP-005] Busca de Traduções e Migração Automática de Configurações
- **Commit:** `5d15918`
- **Componente:** `src/onyxsh/utils/translation_utils.py`, `src/onyxsh/config.py`
- **Sintoma:** Idiomas não carregavam corretamente no Flatpak/Debian e configurações do host não eram migradas.
- **Causa Raiz:** Caminhos de `.mo` divergentes em layouts sandboxed e isolamento de `/home/user/.config/onyxsh`.
- **Correção:** Busca em múltiplos caminhos (incluindo `app/share/locale` e `src/onyxsh/locale`) e migração transparente de diretórios antigos.

---

## 2. Gerenciador de Arquivos & Quick Look

### [BUG-FM-001] Latência na Navegação de Pastas por Recriação Síncrona do Quick Jump
- **Commit:** `5286bbe`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Delay perceptível e micro-travamentos ao trocar de diretório no File Manager.
- **Causa Raiz:** `_update_breadcrumb()` chamava síncronamente `_update_quick_jump_popover()`, destruindo e instanciando dezenas de botões e labels GTK em toda e qualquer navegação.
- **Correção:** O popover foi convertido para carregamento sob demanda (*lazy* no evento `notify::visible`).
- **Testes:** `tests/test_filemanager_filtering_sorting.py` (`test_lazy_quick_jump_popover`).

---

### [BUG-FM-002] Invalidação Dupla Redundante de Filtro e Sorter na Navegação
- **Commit:** `5286bbe`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Processamento dobrado na UI thread após trocar de pasta.
- **Causa Raiz:** `_restore_search_entry()` chamava `combined_filter.changed()` e `sorter.changed()` imediatamente após o `Gio.ListStore.splice()`.
- **Correção:** Removidas as chamadas manuais redundantes, permitindo que a infraestrutura nativa do GTK gerencie a atualização dos modelos.

---

### [BUG-FM-003] Destruição e Re-alocação Excessiva de Widgets de Badges no ListView/GridView
- **Commit:** `5286bbe`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Micro-stutters e alto consumo de memória durante rolagem rápida de listas com milhares de arquivos.
- **Causa Raiz:** `_bind_detailed_item` e `_bind_grid_item` executavam loops de `badges_box.remove()` e instanciamento de `Gtk.Label` a cada reciclagem de linha.
- **Correção:** Badges pré-alocados no template da linha (`_setup_*`) com alternância estritamente via `set_visible(True/False)` e classes CSS no bind.

---

### [BUG-FM-004] Bloqueio da UI por Chamada Síncrona a `statvfs` / `disk_usage`
- **Commit:** `5286bbe`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Travamento momentâneo na interface ao clicar em arquivos ou atualizar a barra de status.
- **Causa Raiz:** `shutil.disk_usage()` executava `statvfs` síncrono no loop principal do GTK em toda seleção.
- **Correção:** Implementado cache de espaço livre em disco com TTL de 10 segundos.
- **Testes:** `tests/test_filemanager_filtering_sorting.py` (`test_disk_usage_cache`).

---

### [BUG-FM-005] Segfault por Associação de Gestos no Bind em vez de Setup
- **Commit:** `496815d`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Crash esporádico ou vazamento de handlers ao clicar com o botão direito em itens reciclados.
- **Causa Raiz:** `Gtk.GestureClick` era anexado dentro da função de *bind* da célula.
- **Correção:** Gestos de clique e controladores de evento movidos exclusivamente para o estágio de *setup* da fábrica de itens.

---

### [BUG-FM-006] Duplicação de Colunas no ColumnView
- **Commit:** `da6b803`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Colunas de proprietário e permissões apareciam duplicadas ou com layout quebrado.
- **Causa Raiz:** Múltiplas funções de fábrica (`_setup_owner_cell`, `_setup_detailed_item`) concorrendo no mesmo container.
- **Correção:** Unificação da renderização em um template coeso e remoção de factories obsoletas.

---

### [BUG-FM-007] Loop Infinito de Focus Idle em Diálogos de Criação e Renomeação
- **Commit:** `eeba8fe`
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Alto uso de CPU quando diálogos de criação de arquivo/pasta estavam abertos.
- **Causa Raiz:** Função de foco em `GLib.idle_add()` retornava `True` em vez de `GLib.SOURCE_REMOVE` (`False`).
- **Correção:** Retorno explícito de `False` após aplicar `grab_focus()`.

---

### [BUG-FM-008] Quick Look com Tipografia Monospace e ToolbarView Estável
- **Commits:** `bf787e2`, `b877d63`
- **Componente:** `src/onyxsh/filemanager/quick_look.py`
- **Sintoma:** Headerbar sumindo no visualizador e código sem fonte monoespaçada.
- **Correção:** Restauração do layout padrão com `Adw.ToolbarView` e classe CSS `monospace` nos buffers de visualização.

---

## 3. Assistente de IA, Parser de Scripts & Agent Mode

### [BUG-AI-001] Bypass de Comandos Perigosos no Production Guard
- **Commit:** `c463a3f`
- **Componente:** `src/onyxsh/terminal/production_guard.py`
- **Sintoma:** Comandos destrutivos (como `rm -rf /`) passavam despercebidos quando encapsulados em `bash -c`, `eval`, `xargs rm`, pipes (`| bash`) ou variáveis.
- **Causa Raiz:** Análise léxica superficial baseada apenas no primeiro token do comando.
- **Correção:** Desaninhamento recursivo de subshells, extração de comandos embutidos em strings, sanitização de wrappers (`exec`, `builtin`, `xargs`) e detecção de decodificadores base64.
- **Testes:** `tests/test_production_guard.py`.

---

### [BUG-AI-002] Quebra de Heredocs e Caminhos com Espaços no Planner Argv
- **Commit:** `c5d0171`
- **Componente:** `src/onyxsh/agent/planner.py`
- **Sintoma:** Scripts multilinhas gerados pelo planejador de IA falhavam na execução com erros de sintaxe shell.
- **Causa Raiz:** Divisão ingênua com `cmd.split()` que corrompia blocos `<< 'EOF'`.
- **Correção:** Implementada função `split_command_to_argv()` com `shlex.split()` preservando heredocs como blocos atômicos.
- **Testes:** `tests/test_ai_assistant_script_filter.py`.

---

### [BUG-AI-003] Fragmentação de Scripts e Placeholders Incompletos em Respostas LLM
- **Commits:** `19cc879`, `0f2a29a`, `6d407e3`, `f1a293a`, `d58ee5f`
- **Componente:** `src/onyxsh/terminal/ai_assistant.py`
- **Sintoma:** O LLM gerava scripts com placeholders (`...` ou `[resto do script aqui]`) ou dividia a criação de um único arquivo em dezenas de comandos `echo "linha" >> arquivo`.
- **Causa Raiz:** Falta de pós-processamento para reconstruir blocos atômicos de escrita de script.
- **Correção:** Algoritmo de síntese automática que colapsa sequências de `echo` e fecha heredocs incompletos em um script único e validado.

---

### [BUG-AI-004] Escape de Metacaracteres Shell na Verificação Pós-Execução
- **Commit:** `426dca9`
- **Componente:** `src/onyxsh/agent/verifier.py`
- **Sintoma:** Comandos de verificação falhavam ou executavam injeções se o path contivesse caracteres como `$`, `;`, `&`, `|`.
- **Causa Raiz:** Falta de sanitização profunda em `safe_quote_path()` para subcaminhos de `$HOME`.
- **Correção:** Validação de metacaracteres shell com aplicação estrita de `shlex.quote()`.
- **Testes:** `tests/test_post_verification.py`.

---

### [BUG-AI-005] Vazamento de Segredos em Tokens de Provedores (GitLab, Slack, Vault)
- **Commit:** `2b0c7e2`
- **Componente:** `src/onyxsh/agent/redactor.py`
- **Sintoma:** Tokens como `glpat-*`, `xoxb-*`, `hvs.*` eram enviados ao LLM em logs e anexos de contexto.
- **Causa Raiz:** Expressões regulares cobriam apenas tokens padrão da AWS e GitHub.
- **Correção:** Expansão da base de expressões regulares com proteção contra re-redação de placeholders e contagem precisa via `re.subn()`.
- **Testes:** `tests/test_redactor.py`.

---

### [BUG-AI-006] Corrupção do Log de Auditoria em I/O Lento
- **Commit:** `735500c`
- **Componente:** `src/onyxsh/agent/audit.py`
- **Sintoma:** Log de auditoria de ações do agente ficava truncado ou corrompido em quedas de energia ou I/O pesado.
- **Causa Raiz:** `rotate()` reescrevia o arquivo diretamente no mesmo descriptor.
- **Correção:** Rotação atômica utilizando arquivo temporário, `os.fsync()` e substituição com `os.replace()`.
- **Testes:** `tests/test_audit_rollback.py`.

---

### [BUG-AI-007] Travamento de Tecla Delete e Caracteres Mortos no Chat
- **Commit:** `0b96807`
- **Componente:** `src/onyxsh/terminal/ai_assistant.py`
- **Sintoma:** Pressionar `Delete` ou acentos no input do chat causava comportamento anormal de cursor ou ignorava a digitação.
- **Correção:** Tratamento direto do evento de exclusão no `Gtk.TextView` interceptando `Gdk.KEY_Delete`.

---

### [BUG-AI-008] Execução em Lote de Planos de Múltiplos Passos
- **Commit:** `a6278fa`
- **Componente:** `src/onyxsh/terminal/ai_assistant.py`
- **Sintoma:** Se um comando intermediário de um plano em lote falhava, os comandos subsequentes continuavam executando cegamente.
- **Correção:** Encadeamento estrito de passos via `&&` garantindo aborto imediato em caso de erro no código de retorno.

---

### [BUG-AI-009] Localização Estrita de Idioma no Prompt de Sistema
- **Commit:** `ab80c33`
- **Componente:** `src/onyxsh/terminal/ai_assistant.py`
- **Sintoma:** O assistente respondia em inglês mesmo quando a interface do OnyxSH estava em Português ou outro idioma.
- **Correção:** Injeção mandatória da tag de idioma e diretivas de localização no template de prompt do sistema.

---

## 4. Terminal, Rastreamento Semântico & Atalhos

### [BUG-TERM-001] Cálculo de Coordenadas e Salto de Prompts Semânticos (OSC 133)
- **Commits:** `986223a`, `8624169`, `6b5c439`, `5b5c0e4`, `e5c99a0`
- **Componente:** `src/onyxsh/terminal/semantic_tracker.py`, `src/onyxsh/ui/actions.py`
- **Sintoma:** O atalho `Alt+Up` / `Alt+Down` para pular entre comandos executados rolava a tela para posições incorretas ou travava em saltos consecutivos.
- **Causa Raiz:** Erro na conversão entre coordenadas de linha lógica do VTE e pixels de rolagem do `Gtk.Adjustment`, além de perda do cursor de histórico.
- **Correção:** Implementada fórmula canônica de conversão pixel/linha com rastreamento persistente de `last_nav_target` e fallback para varredura de buffer.
- **Testes:** `tests/test_semantic_prompts.py`.

---

### [BUG-TERM-002] Descompasso de Cursor com Realce de Sintaxe do Shell
- **Commits:** `026cb09`, `e762320`
- **Componente:** `src/onyxsh/terminal/highlighter.py`
- **Sintoma:** Caracteres digitados no prompt sofriam atraso visual ou o cursor pulava para posições inválidas ao usar sequências de escape ANSI.
- **Correção:** Reset automático do buffer do realçador ao interceptar sequências de controle e repasse nativo da sequência `DELETE_SEQUENCE`.

---

### [BUG-TERM-003] Criação Segura do Diretório de Logs
- **Commits:** `1a24fab`, `e66b54d`
- **Componente:** `src/onyxsh/utils/logger.py`
- **Sintoma:** Falha silenciosa de inicialização caso o diretório de logs em `~/.cache/onyxsh/logs` não existisse.
- **Correção:** Criação automática de diretórios com permissões seguras `0700` no startup.

---

## 5. Core, Async Tasks, Segurança & Infraestrutura

### [BUG-CORE-001] Mapeamento Incorreto de Tasks no `AsyncTaskManager`
- **Commit:** `b6f3fd2`
- **Componente:** `src/onyxsh/core/tasks.py`
- **Sintoma:** `pending_io_tasks` e `pending_cpu_tasks` retornavam sempre `0`, impedindo monitoramento correto de tarefas em segundo plano.
- **Causa Raiz:** O código buscava a propriedade `_thread_name_prefix` dentro do objeto `Future` (que não existe na biblioteca padrão).
- **Correção:** Mapeamento explícito de futures por tipo em dicionário protegido por `RLock` com autolimpeza em callback de conclusão.
- **Testes:** `tests/test_tasks.py`.

---

### [BUG-CORE-002] Vazamento de File Descriptors em `LoggerManager`
- **Commit:** `3d18ce0`
- **Componente:** `src/onyxsh/utils/logger.py`
- **Sintoma:** Abertura crescente de descritores de arquivo ao alterar configurações de log em tempo de execução.
- **Causa Raiz:** Handlers eram desanexados sem invocar `.flush()` e `.close()`.
- **Correção:** Fechamento explícito de todos os handlers com métodos `close()` em `ThreadSafeLogger` e `close_all_loggers()`.
- **Testes:** `tests/test_logger.py`.

---

### [BUG-CORE-003] `SIGALRM` Quebrava Resolução de Hostnames em Threads Secundárias
- **Commit:** `0943305`
- **Componente:** `src/onyxsh/utils/security.py`
- **Sintoma:** `ValueError: signal only works in main thread of the main interpreter` durante testes de conexão SSH em background.
- **Causa Raiz:** `signal.setitimer(SIGALRM)` não pode ser invocado fora da thread principal no Python.
- **Correção:** Timeout implementado via worker thread com `thread.join(timeout=timeout)`.
- **Testes:** `tests/test_security.py`.

---

### [BUG-CORE-004] Função Deprecada `locale.getdefaultlocale()`
- **Commit:** `fee81a1`
- **Componente:** `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/utils/platform.py`
- **Sintoma:** `DeprecationWarning` no Python 3.11+ e quebra iminente no Python 3.13+.
- **Correção:** Migrado para `locale.getlocale()` com fallback resiliente para a variável de ambiente `LANG`.

---

### [BUG-CORE-005] Quebra de Markup Pango em Rótulos com Caractere `&`
- **Commits:** `b0ac4a4`, `a21e206`, `dec6ad9`
- **Componente:** `src/onyxsh/ui/actions.py`, `src/onyxsh/terminal/ai_assistant.py`
- **Sintoma:** `Gtk-CRITICAL` e texto não renderizado em nomes de sessões ou comandos contendo `&` (ex.: `Quick Jump & Bookmarks`).
- **Correção:** Uso de `GLib.markup_escape_text()` antes de repassar strings a componentes com `use-markup=True`.

---

## 🔒 Regras de Ouro para Não Reintroduzir Bugs

1. **Nunca use `.resolve()` em caminhos locais dentro do Flatpak antes de enviar a `flatpak-spawn --host`:**
   Links simbólicos dentro do Flatpak podem apontar para diretórios virtuais (`/run/host/...`) que **não existem** no sistema operacional real.
2. **Nunca crie ou destrua widgets GTK repetidamente no `_bind` de ListViews:**
   Widgets devem ser alocados **apenas uma vez** no `_setup_*` e reciclados no `_bind_*` alternando visibilidade (`set_visible`) e propriedades.
3. **Nunca execute `shutil.disk_usage` ou `statvfs` síncronos na thread principal da UI:**
   Sistemas com NFS, HDs mecânicos ou alta carga de I/O congelam a interface gráfica. Use o cache com TTL.
4. **Nunca chame APIs GTK/Adw fora da thread principal:**
   Qualquer notificação ou atualização de UI vinda de threads assíncronas **deve** usar `GLib.idle_add()`.
5. **Nunca commite código sem rodar a suíte completa de testes unitários:**
   ```bash
   PYTHONPATH=src python3 -m unittest discover -s tests
   ```
