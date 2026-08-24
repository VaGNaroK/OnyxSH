# Relatório Técnico Completo: Bug e Resolução da Navegação de Prompts Semânticos (OnyxSH)

> **Documento destinado à análise por Agentes de Inteligência Artificial e Engenheiros de Software.**
> **Data:** 23/08/2026  
> **Repositório:** `VaGNaroK/OnyxSH`  
> **Escopo:** Navegação entre prompts anteriores/seguintes via atalhos (<kbd>Alt + ↑</kbd>, <kbd>Alt + ↓</kbd>, <kbd>Ctrl + Shift + ↑</kbd>, <kbd>Ctrl + Shift + ↓</kbd>) e Menu de Contexto do Terminal.

---

## 1. Visão Geral da Funcionalidade

A navegação de prompts semânticos permite ao usuário saltar diretamente para os pontos de início dos comandos executados no terminal histórico, sem necessidade de rolagem manual contínua.

### 1.1. Arquitetura Esperada
```
┌─────────────────────────────────────────────────────────────┐
│                    Terminal (Vte.Terminal)                  │
│  - Recebe comandos do usuário e executa no PTY (bash/zsh)   │
│  - Emite sequências semânticas OSC 133                      │
└──────────────────────────────┬──────────────────────────────┘
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
┌───────────────────────┐             ┌─────────────────────────┐
│   SemanticTracker     │             │     Input & Actions     │
│ - OSC 133;A (Prompt)  │             │ - Alt+Up / Alt+Down     │
│ - OSC 133;C (Exec)    │             │ - Ctrl+Shift+Up/Down    │
│ - OSC 133;D (Finish)  │             │ - Menu de Contexto      │
│ - Lista `prompt_rows` │             │ - `WindowActions`       │
└───────────┬───────────┘             └────────────┬────────────┘
            │                                      │
            └──────────────────┬───────────────────┘
                               ▼
            ┌──────────────────────────────────────┐
            │  Cálculo de Deslocamento e Rolagem   │
            │  - Linha do prompt alvo (Linhas)     │
            │  - Conversão Linha -> Pixels         │
            │  - `Gtk.Adjustment.set_value(px)`    │
            └──────────────────────────────────────┘
```

---

## 2. Linha do Tempo de Tentativas e Diagnósticos

### Fase 1: Primeiras Modificações e Hipóteses Iniciais
- **Problema Observado:** Ao pressionar <kbd>Alt + Up</kbd>, nada acontecia na tela.
- **Tentativas Realizadas:**
  1. Alteração inicial nos cálculos de `Gtk.Adjustment`.
  2. Adição de manipuladores em `window.py` com `EventControllerKey` em fase `CAPTURE`.
  3. Criação de fallback via regex no buffer (`_scan_previous_prompt_in_buffer`).
- **Sintoma Persistente:** O atalho de teclado não surtia efeito visual.

---

### Fase 2: Diagnóstico dos Atalhos e Criação do Menu de Contexto
- **Problema Observado:** O usuário verificou via `xev` que o X11 e o Desktop entregavam o evento `<Alt>Up` com `state 0x18` (Alt + NumLock), mas o VTE consumia o evento antes dos controladores convencionais do GTK.
- **Medidas Implementadas:**
  1. **Migração para `Gtk.ShortcutController` (GTK4):** Instalado na janela e no próprio `Vte.Terminal` com escopo `MANAGED` para contornar o `IMContext` do VTE.
  2. **Atalhos Alternativos:** Adicionados `<Ctrl><Shift>Up`, `<Ctrl><Shift>Down`, `<Alt>Page_Up`, `<Alt>Page_Down`.
  3. **Camada de Menu de Contexto (Fallback Visual):** Adicionados itens *"Pular para Prompt Anterior"* e *"Pular para Próximo Prompt"* no menu do botão direito do mouse no terminal (`src/onyxsh/ui/menus.py`).
  4. **Ativação dos Logs Flatpak:** Forçado `log_to_file = True` e `console_log_level = "INFO"`.

---

### Fase 3: A Descoberta Crucial nos Logs do Terminal
Ao executar o Flatpak com rastreamento em nível `CRITICAL`, a saída do terminal capturou os dados reais de execução interna:

```text
22:01:07 | onyxsh.ui.actions | CRITICAL | [ACTION] jump_previous_prompt chamado. terminal=<Vte.Terminal ...>
22:01:07 | onyxsh.ui.actions | CRITICAL | [ACTION] adj direto no terminal=<Gtk.Adjustment ...>
22:01:07 | onyxsh.ui.actions | CRITICAL | [ACTION] adj.value=85428.0, upper=86415.0, page_size=987.0, cursor=(col=52, row=4114)
22:01:07 | onyxsh.ui.actions | CRITICAL | [ACTION] tracker_prompts=[0, 44068, 89124, 89168, 89498, 89542], target_row=89498, current_ref_row=89542
22:01:07 | onyxsh.ui.actions | CRITICAL | [ACTION] SUCESSO: Adjusted scroll value to 85428.0
```

---

## 3. Análise da Causa Raiz (The Root Causes)

A análise matemática dos dados brutos revelou **dois bugs lógicos estruturais**:

### 🔴 Causa Raiz 1: Incompatibilidade de Unidades (Pixels vs Linhas de Texto)

1. **Unidade do `Gtk.Adjustment` no GTK4:**
   - `adj.get_upper() = 86415.0` (Altura total do buffer em **pixels**).
   - `adj.get_page_size() = 987.0` (Altura visível da janela em **pixels**).
   - `max_scroll = 86415.0 - 987.0 = 85428.0` (Deslocamento máximo da barra em **pixels**).

2. **Unidade do Cursor no VTE:**
   - `terminal.get_cursor_position()` retorna `(col=52, row=4114)` (Posição em **linhas de texto**).
   - Altura de cada caractere/linha: `char_height = 86415 / 4114 ≈ 21.0 px`.

3. **O Bug da Soma de Pixels com Linhas:**
   No método `_get_absolute_row()` do `SemanticTracker`:
   ```python
   # CÓDIGO COM BUG:
   val = adj.get_value() # Retornava 85428.0 (pixels)
   return int(round(val)) + grid_row # 85428 (pixels) + 4114 (linhas) = 89542
   ```
   Isso gerava linhas fictícias astronômicas no tracker: `[0, 44068, 89124, 89168, 89498, 89542]`.

4. **O Efeito de Travamento no Fim da Tela:**
   Quando `jump_previous_prompt` calculava o novo scroll:
   ```python
   new_scroll = max(0.0, min(float(target_row), max_scroll))
   # min(89498.0, 85428.0) = 85428.0
   adj.set_value(85428.0)
   ```
   Como o valor atual já era `85428.0`, a tela **permanecia 100% estática** no fim do terminal.

---

### 🔴 Causa Raiz 2: Referência Estática de Busca em Comandos na Mesma Tela Visível

No teste seguinte, com as linhas corrigidas:
```text
CRITICAL | [SEMANTIC NAV] jump_previous_prompt: ref_line=2066, top_line=2020, cursor_line=2066, target_line=2064, prompts=[0, 2047, 2049, 2064, 2066], current_scroll_px=42420.0, max_scroll_px=42420.0
```

1. O viewport visível cobria as linhas `2020` até `2066` (47 linhas).
2. Os comandos `2047`, `2049`, `2064` e `2066` estavam todos **simultaneamente visíveis na tela**.
3. O comando anterior mais antigo (`0`) estava no topo do buffer (antes da saída do `ls -la /usr/bin`).
4. **O Bug da Referência Sem Estado:**
   - A cada clique em *"Pular para Prompt Anterior"*, a função recalculava a busca a partir de `cursor_line` (`2066`).
   - O resultado era invariavelmente `target_line = 2064`.
   - O usuário clicava repetidamente, mas a busca nunca retrocedia para `2049`, `2047` e finalmente `0`.

---

## 4. Soluções de Engenharia Implementadas

### 4.1. Conversão Bidirecional Precisa (Linhas ↔ Pixels)

#### No `SemanticTracker` (`src/onyxsh/terminal/semantic_tracker.py`):
```python
def _get_absolute_row(self, terminal: Vte.Terminal) -> int:
    """Retorna o índice de linha absoluto real no scrollback buffer."""
    col, row = terminal.get_cursor_position()
    char_height = 1.0
    if hasattr(terminal, "get_char_height"):
        try:
            ch = terminal.get_char_height()
            if isinstance(ch, (int, float)) and ch > 1.0:
                char_height = float(ch)
        except Exception:
            pass
    try:
        scrolled = terminal.get_parent()
        if scrolled is not None and isinstance(scrolled, Gtk.ScrolledWindow):
            adj = scrolled.get_vadjustment()
            if adj is not None:
                val = adj.get_value()
                if isinstance(val, (int, float)):
                    scroll_lines = int(round(val / char_height))
                    if row < scroll_lines:
                        return scroll_lines + int(row)
    except Exception:
        pass
    return int(row)
```

#### No `WindowActions` (`src/onyxsh/ui/actions.py`):
```python
# Conversão do índice da linha alvo de volta para pixels:
target_scroll_px = float(target_line) * char_height
new_scroll = max(0.0, min(target_scroll_px, max_scroll_px))
adj.set_value(new_scroll)
```

---

### 4.2. Rastreamento Sequencial Contínuo (`_last_nav_target`)

Implementado dicionário `_last_nav_target` indexado por terminal para manter o histórico de navegação ativa:

```python
# Em jump_previous_prompt:
last_target = getattr(self, "_last_nav_target", {}).get(resolved_terminal)
if last_target is not None:
    current_ref_line = last_target
elif current_scroll_px >= max_scroll_px - 1.0:
    current_ref_line = cursor_line if cursor_line >= current_top_line else (current_top_line + cursor_line)
else:
    current_ref_line = current_top_line

target_line = tracker.get_previous_prompt_row(resolved_terminal, current_ref_line)
if target_line is not None:
    self._last_nav_target[resolved_terminal] = target_line
    # Rolagem para a linha de destino
```

**Resultado do Ciclo com `[0, 2047, 2049, 2064, 2066]`:**
- **Clique 1:** `2066 -> 2064`
- **Clique 2:** `2064 -> 2049`
- **Clique 3:** `2049 -> 2047`
- **Clique 4:** `2047 -> 0` (Rola 2000 linhas até o topo do buffer)
- **Clique Próximo:** `0 -> 2047 -> 2049 -> 2064 -> 2066`

---

## 5. Matriz de Arquivos Modificados

| Arquivo | Responsabilidade Alterada |
|---|---|
| [`src/onyxsh/terminal/semantic_tracker.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/terminal/semantic_tracker.py) | Conversão correta de pixels para linhas em `_get_absolute_row`. |
| [`src/onyxsh/ui/actions.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/ui/actions.py) | Cálculo de scroll `line * char_height`, fallbacks robustos e rastreamento `_last_nav_target`. |
| [`src/onyxsh/window.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/window.py) | `Gtk.ShortcutController` com `MANAGED` scope e máscara de aceleradores padrão. |
| [`src/onyxsh/terminal/manager.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/terminal/manager.py) | `get_active_terminal` no manager e controller de atalhos no widget `Vte.Terminal`. |
| [`src/onyxsh/ui/menus.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/ui/menus.py) | Itens de menu com prefixo `"win."` e ícones `go-up-symbolic`/`go-down-symbolic`. |
| [`src/onyxsh/app.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/app.py) | Mapeamento de múltiplos aceleradores (`<Alt>Up`, `<Ctrl><Shift>Up`, etc.). |
| [`src/onyxsh/settings/config.py`](file:///home/vagnarok/zashterminal-Fork-main/src/onyxsh/settings/config.py) | Configurações padrão de atalhos e persistência de log. |
| [`tests/test_semantic_prompts.py`](file:///home/vagnarok/zashterminal-Fork-main/tests/test_semantic_prompts.py) | Testes unitários para cálculo de linha absoluta, fallback de buffer e execução das actions. |

---

## 6. Verificação e Testes Unitários

Todos os testes unitários do projeto foram executados e validados com 100% de sucesso:

```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

**Resultado:**
```text
Ran 217 tests in 2.277s
OK
```

---

## 7. Instruções para Validação Manual

1. **Recompilar o Flatpak:**
   ```bash
   ./scripts/build_flatpak.sh --reinstall
   ```
2. **Executar o OnyxSH:**
   ```bash
   flatpak run io.github.vagnarok.OnyxSH
   ```
3. **Gerar Histórico no Terminal:**
   ```bash
   ls -la /usr/bin
   uname -a
   curl -I https://google.com
   echo "Teste Final"
   ```
4. **Validar:**
   - Clique com o botão direito → *"Pular para Prompt Anterior"* (ou <kbd>Alt + ↑</kbd> / <kbd>Ctrl + Shift + ↑</kbd>).
   - Clique repetidamente até que a tela suba as 2000 linhas até o topo do primeiro comando.
   - Clique em *"Pular para Próximo Prompt"* (ou <kbd>Alt + ↓</kbd> / <kbd>Ctrl + Shift + ↓</kbd>) para retornar ao final.
