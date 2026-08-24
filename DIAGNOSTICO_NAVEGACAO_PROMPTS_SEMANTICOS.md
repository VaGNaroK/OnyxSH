# Diagnóstico Detalhado: Navegação entre Prompts Semânticos (Alt + Up / Alt + Down) no OnyxSH

## 1. Visão Geral da Funcionalidade

A funcionalidade de **Navegação Semântica de Prompts** foi projetada para permitir que o usuário navegue rapidamente pelo histórico do terminal utilizando os atalhos de teclado:
- **<kbd>Alt</kbd> + <kbd>↑</kbd> (Seta para Cima):** Rola a tela do terminal para a posição exata de início do prompt anterior.
- **<kbd>Alt</kbd> + <kbd>↓</kbd> (Seta para Baixo):** Rola a tela do terminal para o próximo prompt abaixo (ou de volta ao final do terminal).

### Arquitetura Esperada
1. **Rastreamento Semântico (OSC 133 / OSC 7 / OSC 0 / Fallback via Buffer):**
   - O shell (bash/zsh) emite sequências de escape semânticas antes e depois de cada comando:
     - `OSC 133;A`: Início do Prompt
     - `OSC 133;B`: Início da Digitação do Comando
     - `OSC 133;C`: Início da Execução do Comando
     - `OSC 133;D;<exit_code>`: Fim da Execução do Comando
   - O `SemanticTracker` (`src/onyxsh/terminal/semantic_tracker.py`) armazena a lista de linhas de prompt (`prompt_rows`) para cada terminal.
   - Caso o shell não suporte ou não tenha emitido OSC 133, um mecanismo de fallback via regex escaneia o buffer do VTE (`_scan_previous_prompt_in_buffer` e `_scan_next_prompt_in_buffer` em `src/onyxsh/ui/actions.py`).

2. **Interceptação de Teclas:**
   - **Nível da Janela (`CommTerminalWindow` em `src/onyxsh/window.py`):** `Gtk.EventControllerKey` com `PropagationPhase.CAPTURE` intercepta `<Alt>Up` e `<Alt>Down`.
   - **Nível do Terminal (`TerminalManager` em `src/onyxsh/terminal/manager.py`):** `Gtk.EventControllerKey` com `PropagationPhase.CAPTURE` intercepta a tecla diretamente no widget `Vte.Terminal`.
   - **Acelerador Global GTK (`Gtk.Application` em `src/onyxsh/app.py`):** Mapeamento nativo de `win.jump-previous-prompt` e `win.jump-next-prompt`.

3. **Cálculo de Deslocamento e Rolagem:**
   - As ações `jump_previous_prompt` e `jump_next_prompt` em `src/onyxsh/ui/actions.py` calculam a linha de referência no buffer scrollback do VTE e ajustam o valor do `Gtk.Adjustment` vertical (`adj.set_value(target_row)`).

---

## 2. Histórico de Todas as Tentativas e Alterações Implementadas

### Tentativa 1: Correção da Unidade de Rolagem e Coordenadas Absolutas
- **Problema Inicial:** O código original multiplicava o número da linha pela altura do caractere em pixels (`target_row * char_height`), mas o `Gtk.Adjustment` do VTE em GTK4 opera em **linhas/linhas de texto**, e não em pixels (`get_scroll_unit_is_pixels() == False`). Além disso, `terminal.get_cursor_position()` retornava coordenadas relativas à grade visível (`0` a `rows-1`), perdendo o offset de scrollback.
- **Alterações Feitas:**
  - Em `src/onyxsh/terminal/semantic_tracker.py`: Criado o método `_get_absolute_row(terminal)` para somar o deslocamento da barra de rolagem (`scroll_offset + grid_row`).
  - Em `src/onyxsh/ui/actions.py`: Removida a multiplicação por `char_height`, aplicando diretamente `adj.set_value(max(0.0, min(float(target_row), max_scroll)))`.
  - Em `src/onyxsh/terminal/manager.py`: Adicionada interceptação em `_on_terminal_key_pressed_for_detection`.
- **Resultado:** Testes unitários passaram, mas o usuário reportou que nos testes práticos nada acontecia ao pressionar <kbd>Alt</kbd> + <kbd>Up</kbd>.

### Tentativa 2: Resolução do Terminal Ativo e Navegação Contínua
- **Problema Descoberto:** Em `src/onyxsh/ui/actions.py`, a função chamava `self.window.terminal_manager.get_active_terminal()`, porém o método `get_active_terminal()` não existia no `TerminalManager` (existia apenas `get_selected_terminal()` no `TabManager`). Isso fazia com que `terminal` resultasse em `None` e a função retornasse silenciosamente na linha `if not terminal: return`.
- **Alterações Feitas:**
  - Em `src/onyxsh/terminal/manager.py`: Implementado `get_active_terminal(self)` delegando para `tab_manager.get_selected_terminal()`. Passada a referência direta `terminal` para `jump_previous_prompt(terminal)`.
  - Em `src/onyxsh/ui/actions.py`: Criado `_get_active_terminal(self, terminal=None)` para resolver o terminal com múltiplos fallbacks.
  - Implementada a lógica de navegação contínua: se o usuário já estiver rolado acima do final, a linha de referência passa a ser a linha superior visível (`current_scroll_val`), permitindo saltos consecutivos para prompts mais antigos.
- **Resultado:** Testes unitários atualizados e aprovados, mas no Flatpak ainda não houve resposta visual.

### Tentativa 3: Logs de Diagnóstico, Máscara de Modificadores e Aceleradores GTK
- **Problema Descoberto:** O log do terminal parava de ser gravado após a inicialização porque `DEFAULT_SETTINGS` continha `"log_to_file": False` e `"console_log_level": "ERROR"`, silenciando os logs no boot. Além disso, o atalho não estava registrado na lista oficial de aceleradores do `Gtk.Application`.
- **Alterações Feitas:**
  - Em `src/onyxsh/settings/config.py`: Definidos `"log_to_file": True` e `"console_log_level": "INFO"` como padrão. Registrados `"jump-previous-prompt": "<Alt>Up"` e `"jump-next-prompt": "<Alt>Down"` nos atalhos padrão.
  - Em `src/onyxsh/settings/manager.py`: Ajustado `_apply_log_settings` para manter o log ativo.
  - Em `src/onyxsh/app.py`: Adicionados `"jump-previous-prompt"` e `"jump-next-prompt"` em `_update_window_shortcuts` com `<Alt>Up`, `<Alt>KP_Up`, `<Alt>Down`, `<Alt>KP_Down`.
  - Em `src/onyxsh/window.py` e `src/onyxsh/terminal/manager.py`: Adicionados logs com prefixo `[KEY EVENT]` e `[SEMANTIC NAV]` para capturar qualquer evento de tecla com modificador <kbd>Alt</kbd> ou <kbd>Ctrl</kbd>.
  - Em `src/onyxsh/ui/dialogs/shortcuts_dialog.py`: Adicionadas as opções na interface de Preferências de Atalhos.
- **Resultado:** 217 testes unitários passaram com 100% de sucesso. No entanto, ao executar o Flatpak, o log do terminal ainda não registra nenhum evento ao pressionar as teclas.

---

## 3. Código Atual das Camadas Envolvidas

### 3.1. Rastreamento Semântico (`src/onyxsh/terminal/semantic_tracker.py`)
```python
    def _get_absolute_row(self, terminal: Vte.Terminal) -> int:
        col, grid_row = terminal.get_cursor_position()
        try:
            scrolled = terminal.get_parent()
            if scrolled is not None and isinstance(scrolled, Gtk.ScrolledWindow):
                adj = scrolled.get_vadjustment()
                if adj is not None:
                    val = adj.get_value()
                    if isinstance(val, (int, float)):
                        return int(round(val)) + grid_row
        except Exception:
            pass
        return grid_row

    def get_previous_prompt_row(self, terminal: Vte.Terminal, current_row: int) -> Optional[int]:
        with self._lock:
            state = self._terminals.get(terminal)
            if not state or not state.prompt_rows:
                return None
            for row in reversed(state.prompt_rows):
                if row < current_row:
                    return row
            return None
```

### 3.2. Ações de Pulo de Prompt (`src/onyxsh/ui/actions.py`)
```python
    def jump_previous_prompt(self, terminal=None, *args):
        try:
            terminal = self._get_active_terminal(terminal)
            if not terminal:
                self.logger.warning("[SEMANTIC NAV] jump_previous_prompt: No active terminal found")
                return

            adj = terminal.get_vadjustment() if hasattr(terminal, "get_vadjustment") else None
            if not adj:
                scrolled = terminal.get_parent()
                if scrolled and hasattr(scrolled, "get_vadjustment"):
                    adj = scrolled.get_vadjustment()
            if not adj:
                self.logger.warning("[SEMANTIC NAV] jump_previous_prompt: No vertical adjustment found on terminal")
                return

            current_scroll_val = adj.get_value()
            max_scroll = max(0.0, adj.get_upper() - adj.get_page_size())
            col, row = terminal.get_cursor_position()

            if current_scroll_val >= max_scroll - 1.0:
                current_ref_row = int(round(current_scroll_val)) + row
            else:
                current_ref_row = int(round(current_scroll_val))

            tracker = self.window.terminal_manager.semantic_tracker if hasattr(self.window, "terminal_manager") else None
            target_row = tracker.get_previous_prompt_row(terminal, current_ref_row) if tracker else None

            if target_row is None or target_row >= current_ref_row:
                target_row = self._scan_previous_prompt_in_buffer(terminal, current_ref_row)

            state = tracker.get_or_create_state(terminal) if tracker else None
            prompts_list = list(state.prompt_rows) if state else []
            self.logger.info(
                f"[SEMANTIC NAV] jump_previous_prompt: ref_row={current_ref_row}, current_scroll={current_scroll_val:.1f}, "
                f"max_scroll={max_scroll:.1f}, tracker_prompts={prompts_list}, target_row={target_row}"
            )

            if target_row is not None:
                new_scroll = max(0.0, min(float(target_row), max_scroll))
                adj.set_value(new_scroll)
                self.logger.info(f"[SEMANTIC NAV] Adjusted scroll value to {new_scroll:.1f}")
            else:
                self.logger.info("[SEMANTIC NAV] No earlier prompt found above current position")
        except Exception as e:
            self.logger.error(f"[SEMANTIC NAV] Error jumping to previous prompt: {e}")
```

### 3.3. Interceptação de Teclas na Janela (`src/onyxsh/window.py`)
```python
        # Em _on_key_pressed:
        effective_state = state & Gtk.accelerator_get_default_mod_mask()
        accel_string = Gtk.accelerator_name(keyval, effective_state)

        if effective_state & (Gdk.ModifierType.ALT_MASK | Gdk.ModifierType.CONTROL_MASK):
            self.logger.info(
                f"[KEY EVENT] Window: keyval={keyval} ({Gdk.keyval_name(keyval) or 'unknown'}), "
                f"accel={accel_string}, state={int(state)}"
            )

        if (
            (accel_string and accel_string in ("<Alt>Up", "<Alt>KP_Up", "<Alt>uparrow"))
            or (
                (effective_state & Gdk.ModifierType.ALT_MASK)
                and not (effective_state & Gdk.ModifierType.CONTROL_MASK)
                and keyval in (Gdk.KEY_Up, Gdk.KEY_KP_Up)
            )
        ):
            self.logger.info("[KEY EVENT] Window: Alt+Up detected, invoking jump_previous_prompt")
            active_term = self.tab_manager.get_selected_terminal() if self.tab_manager else None
            self.action_handler.jump_previous_prompt(active_term)
            return Gdk.EVENT_STOP
```

### 3.4. Interceptação de Teclas no Terminal (`src/onyxsh/terminal/manager.py`)
```python
        # Em _on_terminal_key_pressed_for_detection:
        effective_state = state & Gtk.accelerator_get_default_mod_mask()
        accel_name = Gtk.accelerator_name(keyval, effective_state)
        if effective_state & (Gdk.ModifierType.ALT_MASK | Gdk.ModifierType.CONTROL_MASK):
            self.logger.info(
                f"[KEY EVENT] Terminal {terminal_id}: keyval={keyval} ({Gdk.keyval_name(keyval) or 'unknown'}), "
                f"accel={accel_name}, state={int(state)}"
            )
        if (
            accel_name in ("<Alt>Up", "<Alt>KP_Up", "<Alt>uparrow")
            or (
                (effective_state & Gdk.ModifierType.ALT_MASK)
                and not (effective_state & Gdk.ModifierType.CONTROL_MASK)
                and keyval in (Gdk.KEY_Up, Gdk.KEY_KP_Up)
            )
        ):
            self.logger.info(
                f"[KEY EVENT] Terminal {terminal_id}: Alt+Up matched, invoking jump_previous_prompt"
            )
            if hasattr(self.parent_window, "action_handler"):
                self.parent_window.action_handler.jump_previous_prompt(terminal)
                return Gdk.EVENT_STOP
```

---

## 4. Análise de Causas Raiz e Hipóteses do Bug Atual

Apesar de a lógica interna estar 100% testada e aprovada em testes unitários, o log do sistema não registra nenhuma linha `[KEY EVENT]` quando o usuário pressiona as teclas no aplicativo em execução. Isso aponta para causas nas camadas externas do GTK / Flatpak / VTE / Sistema Operacional:

### Hipótese 1: O VTE Widget ou o IMContext do GTK4 consome o evento de tecla antes dos Controllers
- No GTK4, o widget `Vte.Terminal` possui tratamento interno de teclado e Input Method (IMContext).
- Em alguns ambientes, quando um `Gtk.EventControllerKey` está conectado em `PropagationPhase.CAPTURE` no terminal, o VTE pode interpretar sequências com <kbd>Alt</kbd> (Meta key) como sequências de escape do shell (por exemplo, `\e\e[A` ou `\e[1;3A`) antes de disparar o controller ou repassando diretamente ao PTY do shell.

### Hipótese 2: Conflito de Atalho Global do Ambiente Desktop (Cinnamon / GNOME / XFCE)
- No ambiente desktop Linux do usuário (Linux Mint Cinnamon / GNOME), a combinação <kbd>Alt</kbd> + <kbd>Up</kbd> ou <kbd>Alt</kbd> + <kbd>Down</kbd> pode estar reservada pelo gerenciador de janelas (Muffin / Mutter / XFWM) ou pelo sistema operacional (ex.: navegação de pastas no gerenciador de arquivos, manipulação de janelas ou tiling), de modo que o evento de teclado sequer chega ao processo do Flatpak.

### Hipótese 3: Tratamento de Atalhos no Flatpak via Portal XDG / Wayland / X11
- Sob sandbox Flatpak, aceleradores de teclado globais dependem do foco correto da janela raiz (`Adw.ApplicationWindow`). Se o foco estiver no widget interno do VTE e o atalho estiver registrado apenas via `app.set_accels_for_action`, o GTK4 pode delegar para a ação somente se a tecla não for consumida pelo foco filho.

### Hipótese 4: Rastreamento Semântico via Shell Hooks (bash / zsh)
- Ao abrir o terminal local no Flatpak (`flatpak-spawn --host`), se o arquivo de inicialização de shell (`~/.cache/onyxsh/shell_init/onyxsh_bashrc`) não tiver sido carregado ou sobrescrito pelo `.bashrc` do host, os eventos OSC 133 (`OSC 133;A` e `OSC 133;D`) não serão emitidos, fazendo com que `tracker_prompts` fique vazio (`[]`). O fallback de buffer regex deveria atuar nesse caso, mas depende da chegada do evento de tecla.

---

## 5. Próximos Passos Recomendados para Resolução

1. **Adicionar Atalho Alternativo / Customizável:**
   - Adicionar uma combinação alternativa que não conflite com ambientes de desktop, como <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>↑</kbd> e <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>↓</kbd> (ou <kbd>Alt</kbd> + <kbd>Page_Up</kbd> / <kbd>Page_Down</kbd>), além de permitir a reconfiguração via tela de Preferências (**F2**).

2. **Captura via `Gtk.ShortcutController` Global na Janela:**
   - Adicionar um `Gtk.ShortcutController` com `Gtk.ShortcutTrigger` direto na `CommTerminalWindow` e no `Vte.Terminal`, mapeando explicitamente `Gtk.ShortcutAction` para invocar a ação `win.jump-previous-prompt`.

3. **Verificação de Eventos de Tecla com `xev` / Ferramenta de Teste:**
   - Testar se o ambiente do usuário envia os keycodes esperados quando <kbd>Alt</kbd> + <kbd>Up</kbd> é pressionado fora e dentro da janela.

4. **Botões de Navegação Visual na Barra Superior ou Menu de Contexto:**
   - Incluir opções "Pular para Prompt Anterior" e "Pular para Próximo Prompt" no menu de contexto do botão direito do terminal e na Command Palette (<kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd>), permitindo acioná-las diretamente mesmo se o atalho de teclado for interceptado pelo gerenciador de janelas do sistema.
