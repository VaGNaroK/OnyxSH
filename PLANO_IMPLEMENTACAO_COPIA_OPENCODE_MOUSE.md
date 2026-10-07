# 📋 Plano de Implementação: Cópia de Código e Interação com Mouse no OpenCode (OnyxSH)

> **Documento Técnico de Arquitetura e Engenharia de Implementação**  
> **Status:** Concluído e Validado (Todas as Fases 1 a 6 Implementadas com 100% de Testes - 618 Testes Passando)  
> **Base de Estudo:** `viabilidade_copy.txt` & `ARQUITETURA_E_POSTMORTEM_OPENCODE.md`  
> **Módulos Afetados:** `src/onyxsh/terminal/`, `src/onyxsh/ui/`, `src/onyxsh/settings/`, `manifests/`, `tests/`  

---

## 1. 🔍 Diagnóstico Técnico do Problema

### 1.1. O que acontece internamente?
O **OpenCode** (assim como ferramentas TUI como Vim, Neovim, Tmux, Htop, Ranger) é construído sobre frameworks de terminal (Bubble Tea / Lipgloss) que ativam o rastreamento avançado de mouse do terminal através de sequências de escape DECSET:
- `\033[?1000h` (Rastreamento de cliques básicos de mouse)
- `\033[?1002h` (Rastreamento de botões e eventos de arraste/drag)
- `\033[?1003h` (Rastreamento de qualquer movimento)
- `\033[?1006h` (Extensão de coordenadas estendidas SGR)

Quando essas sequências são ativadas:
1. O emulador de terminal (**VTE 3.91+ / GTK4**) passa a interceptar todos os eventos de clique e arrasto do mouse.
2. Em vez de criar um retângulo de seleção de texto nativo no buffer gráfico, o VTE empacota as coordenadas e envia como sequências de escape para o processo filho no PTY (stdin do OpenCode).
3. **Efeito percebido pelo usuário:** O arraste do mouse com o botão esquerdo "não seleciona nada", parecendo que o terminal ou o mouse estão travados.

### 1.2. Autoridade do OnyxSH
O OnyxSH é o dono da janela e do widget `Vte.Terminal`. Portanto, o terminal tem controle soberano sobre a camada de eventos e o estado da emulação, sem depender de modificações no binário do OpenCode.

### 1.3. Alinhamento com o ADR e Diretrizes de Arquitetura
Este plano **respeita rigorosamente** o [ARQUITETURA_E_POSTMORTEM_OPENCODE.md](file:///home/vagnarok/OnyxSH/ARQUITETURA_E_POSTMORTEM_OPENCODE.md):
- ❌ **NÃO** intercepta o stream do PTY para tentar "adivinhar" código.
- ❌ **NÃO** faz parsing de sequências ANSI da interface do OpenCode.
- ❌ **NÃO** cria canais IPC frágeis ou subprocessos em background.
- ✅ **OPERA** exclusivamente nas camadas oficiais: eventos do VTE, sequências DEC de terminal, `GdkClipboard` e preenchimento de rascunho no `AIChatPanel`.

---

## 2. 💡 Avaliação das Soluções Propostas

| Solução | Esforço | Impacto UX | Viabilidade Técnica | Prioridade |
| :--- | :---: | :---: | :---: | :---: |
| **1. Shift + Arrastar + Badge Indicador** | Mínimo | Médio | 100% Nativo no VTE | **P0 (Imediato)** |
| **2. "Modo Seleção" Toggle (`Ctrl+Shift+S`)** | Baixo | Alto | 100% Viável via DEC Reset | **P0 (Imediato)** |
| **3. Barra Flutuante Pós-Seleção (Popover)** | Médio | Muito Alto | 100% Viável (GTK4 Popover) | **P1 (Core)** |
| **4. Validação da Ponte OSC 52 (Clipboard)** | Baixo | Médio | Suportado nativamente no VTE 0.76 | **P1 (Core)** |
| **5. Modo de Seleção via Teclado (Copy-Mode)** | Alto | Médio | Requer motor de cursor virtual | **P2 (Futuro)** |

---

## 3. 🛠️ Arquitetura Detalhada das Soluções

### 🟢 Solução 1: Shift + Arraste & Indicador Visual no Split
- **Mecanismo:** O VTE possui tratamento nativo para o modificador `Shift` (`GDK_SHIFT_MASK`). Quando o usuário segura a tecla Shift e arrasta com o botão esquerdo, o VTE ignora o rastreamento de mouse da aplicação TUI e realiza a seleção nativa de texto.
- **Melhoria no OnyxSH:** Como a maioria dos usuários desconhece esse recurso padrão do VTE, o cabeçalho da divisão de tela (*pane header*) do OpenCode ou de terminais com mouse tracking exibirá um indicador visual discreto:
  ```
  [ 🖱️ Mouse capturado | Segure Shift para selecionar texto ]
  ```
- **Local de Implementação:** `src/onyxsh/terminal/tabs.py` na função `_create_terminal_pane`.

---

### 🟡 Solução 2: "Modo Seleção" (Toggle Button & Atalho `Ctrl+Shift+S`)
- **Problema resolvido:** Usuários que desejam copiar múltiplos trechos com o mouse sem precisar ficar segurando a tecla `Shift` continuamente.
- **Mecanismo Técnico:**
  O VTE permite a interpretação de comandos de emulação via método `terminal.feed()`.
  1. **Ao Ativar o Modo Seleção:**
     OnyxSH envia para o VTE o comando para desativar temporariamente o mouse tracking:
     ```python
     terminal.feed(b"\033[?1000l\033[?1002l\033[?1003l\033[?1006l")
     ```
     O VTE passa a tratar qualquer clique e arraste do botão esquerdo como seleção de texto comum.
  2. **Ao Desativar (ou pressionar Esc / clicar novamente):**
     OnyxSH restaura o rastreamento para que o OpenCode volte a responder ao mouse:
     ```python
     terminal.feed(b"\033[?1000h\033[?1002h\033[?1006h")
     ```
- **Interface Gráfica:**
  - Botão de alternância (*toggle button*) no cabeçalho do painel com ícone `edit-select-all-symbolic`.
  - Tooltip: `"Modo Seleção de Texto (Ctrl+Shift+S) - Pausa o mouse do OpenCode para seleção direta"`.
  - Atalho global de janela: `Ctrl + Shift + S`.
  - Borda sutil com classe CSS `.pane-selection-mode` indicando que o terminal está pronto para seleção.

---

### 🟠 Solução 3: Barra Flutuante Pós-Seleção (Floating Action Popover)
- **O Diferencial de UX do OnyxSH:**
  Assim que o usuário conclui qualquer seleção de texto (no terminal comum, no OpenCode ou em split):
  ```
  ┌────────────────────────────────────────────────────────┐
  │ 📋 Copiar  │  🤖 Perguntar à IA  │  🛠️ Refatorar Código │
  └────────────────────────────────────────────────────────┘
  ```
- **Detalhamento dos Botões e Comportamento Específico no OpenCode:**

  1. **📋 Botão "Copiar" (Clipboard Instantâneo):**
     - **Cenário no OpenCode:** O OpenCode gerou na tela um bloco de código, um diff, um comando longo ou uma chave de configuração. O usuário seleciona o texto com o mouse (segurando Shift ou no Modo Seleção).
     - **Como funciona:**
       - Lê o texto selecionado via VTE: `terminal.get_text_selected(Vte.Format.TEXT)`.
       - Escreve na área de transferência do sistema via `GdkClipboard.set(text)`.
       - Fornece feedback visual: o ícone do botão vira um check (`emblem-ok-symbolic`) por 800ms e fecha o popover.
       - **Benefício no OpenCode:** Elimina a necessidade de decorar atalhos como `Ctrl+Shift+C` ou abrir menus de contexto. Com um único clique, o trecho gerado pelo OpenCode vai para o clipboard do sistema (pronto para colar em editores, navegadores ou outros terminais).

  2. **🤖 Botão "Perguntar à IA" (Offloading para LLM Local / Chat Zero-Custo):**
     - **Cenário no OpenCode:** O OpenCode sugeriu uma alteração complexa, exibiu um erro ou gerou código, e o desenvolvedor quer entender a lógica ou tirar uma dúvida rápida sem gastar uma rodada completa de agente na cota do OpenCode.
     - **Como funciona:**
       - Captura o trecho selecionado na tela da TUI do OpenCode.
       - Identifica o diretório de trabalho corrente do projeto onde o OpenCode está rodando via OSC 7 (`SemanticTracker.get_current_dir()`).
       - Abre instantaneamente o painel lateral do **Assistente de IA** do OnyxSH (`AIChatPanel`).
       - Preenche o campo de texto do chat no **modo rascunho** (`set_initial_text`):
         ```markdown
         Contexto do projeto ({cwd}):
         
         ```
         {trecho_selecionado}
         ```
         
         Explique este código ou me tire uma dúvida:
         ```
     - **Economia de Tokens & Execução 100% Local (Ollama / LM Studio):**
       - **Preservação de Cotas:** O OpenCode opera em loop agêntico pesado (ReAct, definições de ferramentas, histórico longo). Gastar mensagens nele para tirar dúvidas simples consome tokens da cota de nuvem. O chat do OnyxSH opera via REST leve direto.
       - **Uso com LLM Local:** Se o usuário configurou o provedor como **Ollama** (`qwen2.5-coder`, `llama3.2`, etc.) ou **LM Studio**, a resposta é gerada na própria máquina (CPU/GPU local), com custo financeiro **zero** e sem gastar nenhum token da cota do OpenCode.
       - **Privacidade Total:** Códigos sensíveis ou proprietários gerados pelo OpenCode podem ser analisados localmente sem sair da máquina.
       - **Zero-Trust:** O texto **NÃO** é enviado automaticamente. O usuário revisa o rascunho, digita sua dúvida e clica em enviar quando desejar.

  3. **🛠️ Botão "Refatorar Código" (Ciclo Iterativo com OpenCode):**
     - **Cenário no OpenCode:** 
       - *Se selecionado dentro do OpenCode:* O usuário seleciona um trecho anterior do chat da TUI que deseja reescrever com novas diretrizes. O botão injeta um prompt estruturado de refatoração no terminal do OpenCode (`terminal.feed_child()`), permitindo uma nova iteração imediata.
       - *Se selecionado no terminal comum ao lado:* O usuário seleciona uma função ou erro no seu terminal de trabalho e o botão abre ou foca o split do OpenCode passando o trecho como contexto inicial para o agente inspecionar e refatorar.
     - **Segurança Zero-Trust:** O código passa por `redact_secrets()` para mascarar senhas e tokens antes de qualquer montagem de prompt, envolvendo o conteúdo em `<untrusted>...</untrusted>`.

- **Implementação Técnica:**
  - Widget `Gtk.Popover` flutuante acoplado ao `Vte.Terminal`, estilizado com Libadwaita (`.linked`, `.flat`).
  - Conectado ao sinal `selection-changed` com debounce de 100ms.
  - Se `terminal.get_has_selection()` for falso, o popover se fecha automaticamente.

---

### 🟡 Solução 4: Ponte OSC 52 (Clipboard Integrado)
- **Cenário:** Quando o OpenCode ou uma ferramenta CLI emite sequências OSC 52 para copiar código internamente (ex: atalho yank), o VTE 0.76 (presente no Flatpak GNOME 46) já processa a sequência.
- **Ações:**
  - Garantir que a permissão de clipboard nos manifests Flatpak e nos builds Debian esteja perfeitamente mapeada (`--socket=wayland`, `--socket=x11`, `--socket=fallback-x11`).
  - Adicionar teste unitário e de integração para validar a integridade do clipboard.

---

## 4. 📅 Fases do Plano de Execução

### Fase 1: Dica Visual de Shift+Drag no Pane Header
- **Tarefas:**
  1. Em `src/onyxsh/terminal/tabs.py`, adicionar chip/badge informativo no `_create_terminal_pane`.
  2. Tornar o badge visível quando o terminal for identificado como instância do OpenCode ou quando mouse tracking for detectado.
  3. Tooltip explicativo orientando o uso do `Shift + Arrastar`.

### Fase 2: Implementação do "Modo Seleção" (`Ctrl+Shift+S`)
- **Tarefas:**
  1. Em `src/onyxsh/terminal/tabs.py`: adicionar botão toggle no cabeçalho do painel do terminal (`btn_selection_mode`).
  2. Em `src/onyxsh/ui/actions.py`: criar método `toggle_selection_mode(self, terminal=None)` e vincular à ação `win.toggle-selection-mode`.
  3. Adicionar atalho `Ctrl+Shift+S` no mapa de aceleradores da janela e na lista de atalhos (`shortcuts_dialog.py`).
  4. Implementar método `set_selection_mode(enabled: bool)` no terminal, disparando as sequências de escape `\033[?1000l` e `\033[?1000h`.
  5. Adicionar classe CSS `.terminal-selection-mode` para realce visual enquanto o modo estiver ativo.
  6. Handler de teclado para desativar automaticamente com a tecla `Escape`.

### Fase 3: Floating Action Popover Pós-Seleção
- **Tarefas:**
  1. Criar componente `SelectionActionPopover` em `src/onyxsh/ui/widgets/selection_popover.py`.
  2. Implementar botões estilizados: Copiar, Perguntar à IA e Refatorar.
  3. Integrar ao ciclo de eventos de `selection-changed` em `src/onyxsh/terminal/manager.py`.
  4. Integrar o botão "Perguntar à IA" com preenchimento em modo rascunho no `AIChatPanel` (`set_initial_text`), garantindo compatibilidade direta com provedores locais (Ollama / LM Studio) e nuvem rápida para offloading de tokens a custo zero.
  5. Adicionar chave de configuração em `src/onyxsh/settings/config.py`:
     - `"terminal.show_selection_toolbar": True`
     - Controle visual em Diálogo de Preferências (Geral / Terminal).

### Fase 4: Testes Unitários Obrigatórios (Conformidade com AGENTS.md)
- **Tarefas:**
  1. Criar `tests/test_terminal_selection_mode.py`:
     - Testar ativação e desativação do modo seleção.
     - Testar emissão de sequências `feed()`.
     - Testar toggle e atalho de ação.
  2. Criar `tests/test_selection_popover.py`:
     - Testar criação do popover, captura de texto selecionado e callback para o chat de IA.
  3. Executar a suíte completa:
     ```bash
     PYTHONPATH=src python3 -m unittest discover -s tests
     ```
     Garantir 100% de aprovação (583 testes passando).

### Fase 5: Validação e Ponte OSC 52 (Clipboard Integrado TUI ➔ Desktop)
- **Diagnóstico:** O VTE upstream intencionalmente não repassa comandos OSC 52 para a área de transferência do desktop por design do GNOME.
- **Implementação Realizada:**
  1. Criado módulo [`src/onyxsh/terminal/osc52_bridge.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/terminal/osc52_bridge.py):
     - Parser com suporte a terminadores BEL (`\x07`) e ST (`\x1b\\`).
     - Decodificação Base64 e suporte a clipboard (`c`) e seleção primária (`p`).
     - Conformidade Zero-Trust: Rejeição de consultas de leitura (`?`) para evitar exfiltração não autorizada do clipboard.
     - Limite de segurança rígido (10MB) contra ataques de exaustão de memória/DoS.
     - Tratamento defensivo e tolerante a falhas na API de clipboard do GTK4.
  2. Integração no fluxo de PTY em [`src/onyxsh/terminal/_highlighter_impl.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/terminal/_highlighter_impl.py):
     - Detecção de sequências `\x1b]52;` na stream de bytes e escrita direta no `GdkClipboard`.
     - Sanitização dos dados enviados para o VTE.
  3. Configurações e Preferências:
     - Adicionada opção `"enable_osc52": True` em [`src/onyxsh/settings/config.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/settings/config.py).
     - Switch row adicionado em [`src/onyxsh/ui/dialogs/preferences_dialog.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/dialogs/preferences_dialog.py).
  4. Manifest Flatpak:
     - Adicionado `--talk-name=org.freedesktop.portal.Desktop` em [`manifests/io.github.vagnarok.OnyxSH.yaml`](file:///home/vagnarok/OnyxSH/manifests/io.github.vagnarok.OnyxSH.yaml).
  5. Testes Unitários:
     - Criado [`tests/test_osc52_bridge.py`](file:///home/vagnarok/OnyxSH/tests/test_osc52_bridge.py) com 20 testes dedicados. Suíte completa aprovada: **603 testes (100% OK)**.

### Fase 6: Modo de Seleção por Teclado (Copy-Mode acessível estilo Tmux)
- **Problema resolvido:** Acessibilidade total e produtividade sem dependência de mouse, permitindo navegar, inspecionar o histórico e copiar trechos de código do terminal ou de TUIs (OpenCode, Vim, etc.).
- **Implementação Realizada:**
  1. Criado módulo [`src/onyxsh/terminal/copy_mode.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/terminal/copy_mode.py):
     - Classe `KeyboardCopyModeController` com interceptação `Gtk.EventControllerKey` em fase `CAPTURE`.
     - Navegação com Setas e teclas Vi (`h`, `j`, `k`, `l`), `Home`/`0`, `End`/`$`, `Page Up`/`Page Down`.
     - Início e extensão de seleção visual com `v`, `Espaço` ou `Shift + Setas`.
     - Normalização de limites `(start_row, start_col)` a `(end_row, end_col)` e extração via `terminal.get_text_range_format()`.
     - Cópia para `GdkClipboard` com `Enter`, `y` ou `c`.
     - Cancelamento de seleção ou saída com `Escape` ou `q`.
     - Pílula/banner flutuante com status, instruções e contagem de caracteres selecionados.
     - Auto-scroll vertical mantendo a linha do cursor virtual em foco.
  2. Borda Visual e Estilização:
     - Classes CSS `.terminal-pane.copy-mode-active`, `.terminal-copy-mode-active` e `.copy-mode-pill` em [`src/onyxsh/data/styles/window.css`](file:///home/vagnarok/OnyxSH/src/onyxsh/data/styles/window.css).
  3. Header do Painel e Atalhos:
     - Botão `copy_mode_btn` adicionado no cabeçalho do split em [`src/onyxsh/terminal/tabs.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/terminal/tabs.py).
     - Atalho `Ctrl + Shift + Espaço` registrado em [`src/onyxsh/settings/config.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/settings/config.py) e [`src/onyxsh/app.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/app.py).
     - Ação `win.toggle-keyboard-copy-mode` registrada em [`src/onyxsh/ui/actions.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/actions.py).
     - Itens adicionados na paleta de comandos ([`command_palette_dialog.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/dialogs/command_palette_dialog.py)) e diálogo de atalhos ([`shortcuts_dialog.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/dialogs/shortcuts_dialog.py)).
  4. Testes Unitários:
     - Criado [`tests/test_terminal_keyboard_copy_mode.py`](file:///home/vagnarok/OnyxSH/tests/test_terminal_keyboard_copy_mode.py) com 15 testes dedicados cobrindo navegação, seleção, cancelamento, cópia de linha/bloco e integração.
     - Suíte completa de testes unitários aprovada: **618 testes (100% OK)**.

---

## 5. 🛡️ Segurança e Compatibilidade (Zero-Trust)

1. **Proteção contra Injeção:**
   Qualquer texto selecionado enviado para a IA passa por `redact_secrets()` antes de ser exibido ou transmitido, eliminando o risco de vazamento de credenciais presentes na tela do terminal.
2. **Sem Bloqueio de Eventos:**
   O uso de `feed()` para desligar o mouse tracking do VTE não interfere na execução do processo filho; o OpenCode continua operando normalmente.
3. **Respeito aos Recursos do Sistema:**
   O popover flutuante é destruído ou ocultado assim que a seleção é desfeita, sem consumo residual de memória ou timers persistentes.
