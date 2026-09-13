# 🧠 OnyxSH — Registro de Bugs Corrigidos & Base de Conhecimento para IA

> **Arquivo:** `AI_BUG_FIX_REGISTRY.md`  
> **Versão do Projeto:** v0.10.1  
> **Última Atualização:** Setembro/2026 (12/09/2026)  
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
- [6. Interface do Usuário & Header Bar](#6-interface-do-usuário--header-bar)

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

### [BUG-FP-006] Bloqueio de Gravação Não-Privilegiada em Arquivos de Sistema no Sandbox
- **Commit:** `c95b194`
- **Componente:** `src/onyxsh/filemanager/operations.py`, `src/onyxsh/filemanager/quick_look.py`
- **Sintoma:** Ao clicar no botão comum "Salvar alterações" (sem Root) em arquivos de sistema (`/etc/hosts`), o app exibia mensagem de sucesso sem pedir senha, mas gravava apenas no overlay privado do container Flatpak sem modificar o arquivo real do host.
- **Causa Raiz:** No container Flatpak, a pasta `/etc` é montada como tmpfs/overlay com permissão de escrita para o processo do app. O comando `os.replace()` substituía o arquivo no container silenciosamente sem lançar `PermissionError`.
- **Correção:** Validação rigorosa de diretórios de sistema (`/etc/`, `/usr/`, `/var/`, etc.) e verificação de permissão `os.access(W_OK)`. Salvamentos comuns sem sudo são imediatamente rejeitados com `PERMISSION_DENIED`, abrindo o diálogo modal para salvar como Superusuário (Root).
- **Testes:** `tests/test_quick_look.py` (`test_save_file_content_normal_denied_for_system_files`, `test_quick_look_readonly_save_triggers_permission_dialog`).

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

### [BUG-FM-009] Deslocamento Vertical e Viewport Vazia ao Alternar para Grade de Ícones
- **Componente:** `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Ao trocar da Lista Detalhada para a Grade de Ícones, a tela ficava em branco e parecia que os diretórios não haviam sido carregados, forçando o usuário a navegar pela trilha de breadcrumbs (`/ > home > vagnarok`) para forçar o recarregamento dos ícones.
- **Causa Raiz:** O ajuste de rolagem vertical (`vadjustment`) do `Gtk.ScrolledWindow` mantinha a posição de rolagem da lista detalhada (onde o usuário havia rolado para baixo). Como a grade compacta os itens em múltiplas colunas, a altura total da grade é muito menor que a da lista, posicionando a viewport no espaço vazio abaixo do final da grade. Além disso, o `Gtk.Stack` era homogêneo por padrão, reservando altura fantasma da lista na grade.
- **Correção:** Desativação de homogeneidade no `view_stack` (`set_vhomogeneous(False)` e `set_hhomogeneous(False)`), reinício explícito dos ajustes de rolagem horizontal e vertical para `0.0` em `_set_view_mode`, solicitação de redimensionamento/redesenho (`queue_resize()`, `queue_draw()`), e suporte a refresh direto ao clicar no botão do diretório atual no breadcrumb.
- **Testes:** `tests/test_filemanager_filtering_sorting.py` (`test_view_mode_switching_resets_scroll`, `test_breadcrumb_click_current_path_refreshes`).

---

### [BUG-FM-010] Latência Severa de Abertura (>10s) e Stall na Navegação de Pastas
- **Componentes:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/utils/translation_utils.py`
- **Sintoma:** Ao clicar no ícone do gerenciador de arquivos, a tela permanecia com fundo branco exibindo "Carregando..." por 10 a 13 segundos antes de listar os diretórios. Ao dar duplo clique para abrir pastas, ocorria um atraso perceptível de mais de 3 segundos.
- **Causa Raiz:**
  1. `_set_store_items` alternava `set_filter(None)` $\rightarrow$ `splice` $\rightarrow$ `set_filter(filter)`, o que disparava 3 reconstruções e reordenações completas de árvore no GTK4 com Python (~5,4s a 7,7s por chamada).
  2. As três visões do `Gtk.Stack` (Lista, Grade e Árvore) mantinham modelos de dados simultaneamente conectados, forçando a criação síncrona concorrente de ~160 templates de widgets para as três visualizações. Na visão de árvore, disparava varredura recursiva de métricas (`scan_dir`) mesmo quando em modo lista.
  3. `rebind_terminal` e `set_visibility(True)` chamavam dois refreshes consecutivos na abertura do File Manager, dobrando o tempo de processamento.
  4. Falta de memoização em `translation_utils._()`, gerando mais de 24.000 chamadas `posix.stat` no disco a cada renderização (~0,8s).
- **Correção:**
  1. Atualização atômica direta via `store.splice(0, n, items)` sem desmontar e remontar filtros no `Gtk.FilterListModel`.
  2. Implementação de *Lazy Model Attachment*: apenas a visualização ativa mantém o modelo conectado (`set_model`); visualizações inativas recebem `None`.
  3. Condicionamento da execução de `_calculate_tree_dir_metrics_async` estritamente a `_current_view_mode == "tree"`.
  4. Proteção contra segundo refresh redundante em `set_visibility()` se o caminho já estiver sincronizado com o terminal.
  5. Memoização da função `_()` com `@functools.lru_cache(maxsize=1024)`.
- **Testes:** `tests/test_filemanager_filtering_sorting.py` (`test_lazy_model_attachment_on_view_switch`, `test_set_store_items_preserves_filter_without_toggle_stall`, `test_translation_cache_efficiency`).

---

### [BUG-FM-011] Quick Look Congelado em "Carregando..." ao Abrir Arquivos em Subpastas e Busca Recursiva
- **Componentes:** `src/onyxsh/filemanager/quick_look.py`, `src/onyxsh/filemanager/manager.py`
- **Sintoma:** Ao abrir arquivos em subpastas (ex.: `/home/vagnarok/Atividades/boas_vindas.py`) ou a partir da busca recursiva e Tree View, o diálogo do Quick Look permanecia congelado na tela de *"Carregando pré-visualização..."* sem exibir o conteúdo e sem transicionar para a página de erro.
- **Causa Raiz:**
  1. Concatenação incorreta de caminhos (`self.current_folder / item.name`), ignorando `item.full_path` do objeto `FileItem`. Isso gerava um caminho inexistente na raiz da pasta aberta, disparando `FileNotFoundError`.
  2. `NameError: cannot access free variable 'e'` no callback assíncrono `GLib.idle_add(on_error)`. No Python 3, a variável `e` do bloco `except Exception as e:` é desalocada ao término do bloco, falhando quando o callback da interface gráfica tentava acessá-la e travando a transição de telas.
- **Correção:**
  1. Criação do método canônico `QuickLookDialog._get_item_path()`, priorizando `item.full_path` com fallback para `current_folder / item.name`.
  2. Sincronização de `self.current_folder` com `PurePosixPath(item.full_path).parent`.
  3. Captura prévia de `err_msg = str(e)` repassando como parâmetro padrão `def on_error(msg=err_msg):` nos callbacks de carregamento e salvamento.
  4. Suporte aprimorado na navegação por teclado no modo Tree View dentro do Quick Look.
- **Testes:** `tests/test_quick_look.py` (`test_get_item_path_resolution`, `test_preview_item_with_subfolder_full_path`, `test_preview_text_file_not_found_transitions_to_error_without_name_error`, `test_preview_image_file_not_found_transitions_to_error_without_name_error`, `test_save_worker_error_does_not_raise_name_error`).

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

### [BUG-AI-010] Sobre-escape de Aspas e Alucinação de Tokens em Nomes com Apóstrofo
- **Componentes:** `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/agent/context_manager.py`
- **Sintoma:** Ao sugerir comandos para diretórios ou arquivos contendo espaços, maiúsculas ou apóstrofos (ex.: `"Dante's Inferno PC PORT"`), modelos locais (Ollama 7B) geravam comandos malformados com barras desnecessárias dentro de aspas duplas (`cd "$HOME/Dante\\'s 's Inferno PC PORT"`), repetição de tokens (`'s 's`) e prefixação redundante de `$HOME/`, fazendo o comando falhar com erro no Bash.
- **Causa Raiz:**
  1. Confusão entre regras de escape JSON (`\"`, `\\`) e sintaxe do Bash: dentro de aspas duplas `"..."`, uma aspa simples `'` não requer escape no Bash. A barra invertida torna-se literal (`\'`), quebrando a busca do diretório.
  2. Alucinação de repetição de token comum em tokenizers menores ao lidar com contrações e apóstrofos (`'s 's`).
  3. Falta de diretiva explícita de quoting limpo (KISS) e ausência de injeção do diretório de trabalho corrente (`$PWD`) no prompt do sistema.
- **Correção:**
  1. Inclusão de regra explícita de *Clean Quoting (KISS)* no prompt de sistema: orienta o uso de aspas duplas simples `cd "Nome da Pasta"`, proibindo barras invertidas redundantes (`\\'`) para apóstrofos e instruindo o uso de caminhos relativos no diretório atual.
  2. Implementação do método `get_current_working_directory()` que detecta o diretório do terminal ativo e injeta a seção `CURRENT WORKING DIRECTORY` no prompt do sistema.
  3. Implementação dos sanitizadores `_clean_overescaped_command()` e `_clean_overescaped_reply_text()`, que removem barras invertidas espúrias antes de apóstrofos dentro de aspas duplas, corrigem duplicação de tokens (`'s 's` $\rightarrow$ `'s`) e normalizam caminhos distorcidos.
- **Testes:** `tests/test_ai_assistant_script_filter.py` (`test_clean_overescaped_command_fixes_escaped_apostrophes_and_duplicate_tokens`, `test_clean_overescaped_reply_text`, `test_system_prompt_includes_cwd_context_and_clean_quoting`, `test_get_current_working_directory_from_terminal`).

---

### [BUG-AI-011] Falha de Conexão na API Groq por Modelos Descontinuados e Rejeição de Formato JSON
- **Componentes:** `src/onyxsh/agent/providers/groq.py`, `src/onyxsh/ui/dialogs/ai_config_dialog.py`, `src/onyxsh/settings/config.py`, `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/agent/router.py`
- **Sintoma:** Ao configurar a chave da Groq e testar a conexão no diálogo de configurações ou enviar prompts ao assistente de IA, ocorriam erros de conexão, HTTP 404 (`model_not_found`) ou HTTP 400 (`'messages' must contain the word 'json'`).
- **Causa Raiz:**
  1. A Groq Cloud API aposentou os modelos `llama-3.3-70b-versatile` e `llama-3.1-8b-instant`, retornando HTTP 404.
  2. O provedor `GroqProvider` enviava fixo `"response_format": {"type": "json_object"}`. A API da Groq rejeita estritamente esse parâmetro com HTTP 400 caso a palavra `"json"` não esteja presente no corpo da mensagem enviada.
  3. O botão de testar conexão executava inferência completa em vez de validar a chave contra o endpoint canônico `/openai/v1/models`.
  4. Falta de sincronização bidirecional em tempo real entre o campo geral de API Key e as chaves específicas por provedor.
- **Correção:**
  1. Atualização dos modelos padrão para `openai/gpt-oss-120b` (perfil avançado) e `qwen/qwen3.8-27b` (perfil rápido).
  2. Implementação do método `GroqProvider.discover_available_models()` com cache local e filtro de modelos incompatíveis (whisper, guard).
  3. Condicionamento de `"response_format": {"type": "json_object"}` à presença da palavra `"json"` na mensagem.
  4. O botão "Testar" agora utiliza a descoberta dinâmica de modelos com feedback detalhado.
  5. Sincronização em tempo real das chaves de API nos diálogos de configuração.
- **Testes:** `tests/test_groq_provider.py` (`test_default_model`, `test_custom_model`, `test_missing_api_key_raises`, `test_discover_available_models_fallback`, `test_discover_available_models_from_api`, `test_complete_without_json_does_not_set_response_format`, `test_complete_with_json_sets_response_format`, `test_complete_stream`).

---

### [BUG-AI-012] Falha de Resolução de Aspas em Variáveis de Ambiente no Verificador Pós-Execução
- **Componente:** `src/onyxsh/agent/verifier.py`
- **Sintoma:** O assistente sugeria a criação de scripts e, ao executar a verificação de sanidade pós-execução (`ai_agent_post_verification`), comandos como `ls -ld '${HOME}/diagnostico_python.sh'` falhavam acusando arquivo não encontrado.
- **Causa Raiz:** A função `safe_quote_path()` envolvia cegamente todo o caminho em aspas simples literais (`'${HOME}/...'`), impedindo a expansão de `${HOME}`, `$HOME` e `~` pelo interpretador Bash.
- **Correção:** Ajustada a função `safe_quote_path()` para reconhecer prefixos de variáveis de ambiente (`${HOME}`, `$HOME`, `~`), preservando a expansão da home do usuário sem quebrar caminhos contendo espaços.
- **Testes:** `tests/test_post_verification.py` (`test_safe_quote_path_expands_home_variables`, `test_post_verifier_script_diagnostics`).

---

### [BUG-AI-013] Retenção Indesejada de VRAM no Ollama com Provedores em Nuvem Ativos
- **Componentes:** `src/onyxsh/agent/providers/ollama.py`, `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/app.py`
- **Sintoma:** Mesmo com a API da Groq configurada e o Modo Estritamente Offline desativado, o Ollama mantinha o modelo `llama3.1:8b` carregado na GPU ocupando 5.9 GB de VRAM indefinidamente.
- **Causa Raiz:**
  1. O Ollama utiliza `keep_alive = -1` no pré-carregamento, fixando o modelo na memória GPU até receber `keep_alive = 0`.
  2. As rotinas `unload_model()` e `_unload_ai_model_on_exit()` abortavam precocemente se o provedor ativo fosse de nuvem (`provider_name not in ("local", "ollama")`).
  3. `OllamaProvider.unload()` só tentava descarregar `self.model`. Se as configurações já estivessem apontando para um modelo de nuvem (`openai/gpt-oss-120b`), o Ollama ignorava o comando de liberação.
- **Correção:**
  1. `OllamaProvider.unload()` agora consulta `/api/ps` e envia `{"keep_alive": 0}` para todos os modelos que estiverem ativos na VRAM.
  2. `unload_model()` agora é disparado sempre que o usuário muda para um provedor de nuvem ou desativa o Modo Offline.
  3. `preload_model_async()` faz a liberação residual em segundo plano ao abrir o terminal em modo nuvem se `ai_unload_on_exit` estiver ativo.
  4. O encerramento da aplicação em `app.py` sempre notifica o Ollama para liberar VRAM.
- **Testes:** `tests/test_llm_lifecycle.py` (`test_ollama_unload_all_active_models`, `test_ai_assistant_unload_when_cloud_provider`, `test_handle_setting_changed_offline_to_cloud`).

---

### [BUG-AI-014] Latência Excessiva (>1,6s) ao Abrir o Painel "Perguntar ao Assistente de IA"
- **Componentes:** `src/onyxsh/utils/tooltip_helper.py`, `src/onyxsh/agent/policy_engine.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/ui/window_ui.py`
- **Sintoma:** Ao clicar no ícone "Perguntar ao assistente de IA", ocorria um congelamento perceptível de 1,6 a 2 segundos na thread da interface antes da abertura do chat.
- **Causa Raiz:**
  1. No GTK 4 sob X11, o método `widget.set_tooltip_text()` bloqueava por ~15 ms por widget para sincronização com o display server. Com ~100 widgets com tooltip criados no histórico de mensagens, a UI congelava por mais de 1,4 segundo.
  2. O painel `AIChatPanel` era criado de forma preguiçosa e bloqueante no momento exato do clique.
  3. O `PolicyEngine()` era instanciado repetidamente para cada comando de cada balão de mensagem, lendo arquivos JSON e recompilando dezenas de regexes no loop da UI.
- **Correção:**
  1. No backend X11 do `tooltip_helper.py`, substituição de `set_tooltip_text` pelo sinal assíncrono nativo sob demanda `query-tooltip`, reduzindo o tempo de registro de 1,58s para 0,0003s (ganho de 4000x).
  2. Implementação do singleton `get_policy_engine()` em `policy_engine.py`.
  3. Pré-aquecimento do painel em background via `GLib.idle_add(self._prewarm_ai_panel)` ao inicializar a janela, tornando o clique no botão imediato (< 1 ms).
- **Testes:** `tests/test_tooltip_helper.py` (`test_add_tooltip_uses_query_tooltip_on_native`, `test_add_tooltip_with_shortcut_on_native`), `tests/test_policy_engine.py` (`test_get_policy_engine_singleton`), `tests/test_window_ui.py` (`test_prewarm_ai_panel_execution`, `test_glib_idle_add_prewarm_available`).

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

### [BUG-TERM-004] Colapso de Intervalo no VTE e Falhas Transitórias de Detecção de Erros
- **Commit:** `641f26b`
- **Componente:** `src/onyxsh/terminal/semantic_tracker.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/agent/error_matcher.py`
- **Sintoma:** Comandos rápidos de uma única linha (`cat /etc/shadow`) ou tracebacks em scripts Python caíam no fallback genérico `Comando Falhou (exit 1)` porque a extração semântica por intervalo retornava vazia antes da conclusão da renderização no buffer VTE. Além disso, variações textuais do curl (`Failed to connect to... Couldn't connect to server`, exit 7) não casavam com o padrão `Connection refused`.
- **Causa Raiz:** A janela calculada entre `output_start_row` e `output_end_row` colapsava ou desencontrava com o buffer real do VTE em comandos de finalização ultrarrápida.
- **Correção:** 
  1. Implementado fallback multinível em `extract_command_output`: se o range estiver vazio, varre as imediações do cursor (`cur_row - 25` a `cur_row + 1`) e extrai as linhas pós-comando do buffer completo via `terminal.get_text_format(Vte.Format.TEXT)`.
  2. Adicionada heurística estrita para alvos de sistema (`/etc/shadow`, `/etc/sudoers`, `/etc/gshadow`) e pacotes sem `sudo`.
  3. Adicionadas variantes de erro de conexão do curl (`Failed to connect`, `Couldn't connect to server`, exit 7) e extração de porta com sugestão de diagnóstico do serviço (`ss -tulpn | grep <port>`).
- **Testes:** `tests/test_terminal_error_suggestions.py`.

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

## 6. Interface do Usuário & Header Bar

### [BUG-UI-001] Marcação de Seleção Retangular Cinza Persistente e Dessincronia de Botões na Header Bar
- **Componentes:** `src/onyxsh/ui/window_ui.py`, `src/onyxsh/data/styles/window.css`, `src/onyxsh/window.py`, `src/onyxsh/ui/actions.py`
- **Sintoma:** 
  1. Os botões de Sessões, Gerenciador de Arquivos, Gerenciador de Comandos e Busca exibiam um retângulo cinza de seleção ao fundo mesmo quando inativos/fechados.
  2. O botão do Assistente de IA permanecia sem nenhuma marcação retangular cinza mesmo ao ser clicado e com o painel de chat aberto.
- **Causa Raiz:**
  1. No GTK4/Libadwaita, botões na `Adw.HeaderBar` sem a classe CSS `.flat` recebem por padrão moldura sólida de botão (retângulo com borda e fundo cinza claro). Os botões de Sessões, Arquivos, Comandos e Busca não tinham `.add_css_class("flat")`.
  2. Em `window.css`, havia uma regra forçando `.sidebar-toggle-button:active, .sidebar-toggle-button:checked { background: transparent; }`, que impedia o botão de sessões de mostrar o fundo ativo quando ligado.
  3. O botão do Assistente de IA era um `Gtk.Button` simples (sem estado toggle) com `.flat`, impossibilitando a exibição do estado `:checked`.
  4. O botão do Gerenciador de Comandos usava `set_action_name("win.show-command-manager")` apontando para uma `SimpleAction` sem estado (stateless), o que reseta o estado `active` de `Gtk.ToggleButton` no GTK4.
- **Correção:**
  1. Adicionado `.add_css_class("flat")` a todos os 5 botões de alternância da header bar.
  2. Convertidos `command_manager_button` e `ai_assistant_button` para `Gtk.ToggleButton`.
  3. Removida a sobreposição transparente em `window.css` para permitir a renderização do estado nativo `:checked` do Libadwaita.
  4. Sincronização bidirecional completa: abertura e fechamento via atalho ou close do diálogo/painel atualizam `set_active()` do respectivo botão.
- **Testes:** `tests/test_window_ui.py` (`test_header_bar_toggle_buttons_types_and_flat_classes`, `test_ai_assistant_button_sync_on_show_and_hide_panel`, `test_on_toggle_ai_assistant_button_handlers`, `test_window_css_no_sidebar_toggle_transparent_override`, `test_window_command_manager_toggle_sync`, `test_window_actions_show_command_manager_toggles_button`).

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
6. **Nunca faça `set_filter(None)` antes de `splice` no `FilterListModel` em GTK4:**
   No GTK4 com Python, redefinir filtros força a reavaliação de todos os itens e reordenação com altíssimo custo de CPU. Atualize o `store` subjacente diretamente.
7. **Nunca anexe modelos de dados em múltiplas visões concorrentes dentro de `Gtk.Stack`:**
   Use *Lazy Model Attachment* (`set_model(None)` nas visões inativas) para que o GTK não construa dezenas de instâncias de widgets para visualizações ocultas.
8. **Sempre use memoização com `@lru_cache` para rotinas de tradução `gettext` chamadas em loops de UI:**
   A função padrão `gettext.gettext` realiza I/O síncrono no disco (`posix.stat`) a cada lookup, provocando degradação drástica da taxa de quadros (FPS).
