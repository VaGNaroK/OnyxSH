# FILEMANAGER_PERFORMANCE_FIX.md
# Documento de Correção de Performance — Gerenciador de Arquivos OnyxSH
> **Para agente de IA:** Este documento contém instruções precisas e completas para implementar
> otimizações de performance no gerenciador de arquivos. Leia cada seção inteiramente antes de
> escrever qualquer código. Execute os testes após **cada** modificação.

---

## 1. Contexto e Objetivo

O gerenciador de arquivos localizado em:
```
src/onyxsh/filemanager/manager.py   (5210 linhas)
```

Apresenta **delay perceptível** (~80–300ms) ao:
- Clicar em um diretório para entrar nele
- Pressionar `..` para voltar ao diretório pai

O objetivo é reduzir esse delay para **≤ 30ms** em diretórios locais já visitados, através de
5 otimizações independentes e seguras que serão detalhadas a seguir.

---

## 2. Regra Obrigatória de Testes

Após **cada bloco de mudança** implementado, execute obrigatoriamente:
```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```
Só prossiga para a próxima mudança se **todos os testes passarem**.

---

## 3. Análise das Causas Raiz

### Causa 1 — Sem Cache de Diretório (IMPACTO: 🔴 Alto)

**Localização:** método `_list_local_files()`, linha ~2388.

**Problema:** Toda navegação — inclusive `..` (voltar ao pai) — executa um `os.scandir()` completo
no disco, sem reutilizar resultados de leituras anteriores. Diretórios com centenas de arquivos
geram I/O de disco desnecessário em cada visita.

**Código atual (linhas 2388–2476):**
```python
def _list_local_files(self, requested_path: str, source: str = "filemanager"):
    """Ultra-fast native directory listing using os.scandir and POSIX stat."""
    try:
        if self._is_destroyed or requested_path != self.current_path:
            return
        if not os.path.exists(requested_path) or not os.path.isdir(requested_path):
            # ... erro
            return
        # ... os.scandir() SEMPRE executado
```

---

### Causa 2 — `store.splice()` com Filtro Ativo (IMPACTO: 🟠 Médio)

**Localização:** método `_set_store_items()`, linha ~2669.

**Problema:** `self.store.splice(0, n, items)` substitui todos os itens do `Gio.ListStore` enquanto
o `Gtk.FilterListModel` está conectado. O GTK recalcula o filtro para **cada item inserido** durante
o splice, causando N chamadas à `_filter_files()` ao invés de 1 recálculo em lote.

**Código atual (linha 2669):**
```python
self.store.splice(0, self.store.get_n_items(), items)
```

---

### Causa 3 — `_update_breadcrumb()` Reconstrói Todos os Widgets (IMPACTO: 🟡 Médio)

**Localização:** método `_update_breadcrumb()`, linha ~890.

**Problema:** A cada navegação, **todos** os botões do breadcrumb são destruídos com `remove()` e
recriados com `append()`. Quando o usuário navega de `/home/user/projetos` para
`/home/user/projetos/src`, apenas o último segmento mudou — mas todos os 4 botões são recriados.

**Código atual (linhas 890–924):**
```python
def _update_breadcrumb(self):
    child = self.breadcrumb_box.get_first_child()
    while child:
        self.breadcrumb_box.remove(child)   # Destrói TODOS
        child = self.breadcrumb_box.get_first_child()
    # ... recria TODOS do zero
```

---

### Causa 4 — `shutil.disk_usage()` Chamado na UI Thread (IMPACTO: 🟡 Médio)

**Localização:** método `_get_free_disk_space_text()`, linha ~2882, chamado por
`_update_status_bar()` (linha ~2719) que roda na UI thread via `idle_add`.

**Problema:** `shutil.disk_usage()` é uma chamada de sistema (`statvfs`) que pode bloquear a UI
thread em filesystems remotos (NFS, FUSE, sshfs) ou lentos. O cache de 10 segundos só funciona
para o **mesmo caminho** — ao navegar para um caminho diferente, o cache não é usado.

**Código atual (linhas 2888–2911):**
```python
def _get_free_disk_space_text(self) -> str:
    ...
    cache_key = path_to_check
    if cache_key in self._disk_usage_cache:
        cached_time, cached_val = self._disk_usage_cache[cache_key]
        if now - cached_time < 10.0:
            return cached_val
    # Se cache miss: chama disk_usage() na UI thread → bloqueia
    usage = shutil.disk_usage(path_to_check)
```

---

### Causa 5 — `_update_breadcrumb()` Chamado Antes de Confirmar Caminho Válido (IMPACTO: 🟡 Baixo)

**Localização:** método `refresh()`, linha 2377.

**Problema:** Em `_on_row_activated()` (linha 2342–2346), o breadcrumb é atualizado
otimisticamente via `refresh()` antes de saber se o diretório é acessível. Se o diretório
não for acessível (permissão negada), o breadcrumb fica temporariamente desatualizado até o
fallback. Isso é aceitável, mas desnecessário em local sessions onde podemos checar antes.

---

## 4. Implementação Detalhada

### 4.1 — Mudança A: Variáveis de Estado no `__init__`

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o bloco de inicialização do `__init__` onde estão as variáveis de cache existentes
(procure por `self._dir_size_cache`, por volta da linha 132). Logo abaixo delas, adicione:

```python
# ── Directory listing cache (Causa 1) ─────────────────────────────────────
# Chave: caminho absoluto normalizado (str)
# Valor: (monotonic_timestamp: float, items: list[FileItem])
self._dir_cache: Dict[str, Tuple[float, list]] = {}
self._DIR_CACHE_TTL: float = 3.0   # segundos — janela segura para sessões locais
self._DIR_CACHE_MAX: int = 30      # máximo de entradas (LRU simples)
# ── Breadcrumb incremental state (Causa 3) ────────────────────────────────
self._last_breadcrumb_path: str = ""
# ── Disk usage async state (Causa 4) ─────────────────────────────────────
self._disk_space_calculating: bool = False
```

**Contexto de onde inserir (linhas 132–135):**
```python
        self._dir_size_cache: Dict[str, int] = {}
        self._dir_size_calculating: Set[str] = set()
        self._disk_usage_cache: Dict[str, Tuple[float, str]] = {}
        self._quick_jump_needs_update = True
        # ← INSERIR AQUI
```

---

### 4.2 — Mudança B: Cache de Diretório em `_list_local_files()`

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o método `_list_local_files()` (linha ~2388). Substitua o **início** do método
(até o `try`/`os.path.exists`) pelo código abaixo. A lógica do `os.scandir` existente
(linhas 2426–2470) permanece **inalterada** — apenas adicionar a verificação de cache no topo
e o salvamento no cache ao final.

**Código a adicionar no início do `try:` em `_list_local_files` (antes do `os.path.exists`):**
```python
        # ── Cache check (Causa 1) ────────────────────────────────────────
        now = time.monotonic()
        cached = self._dir_cache.get(requested_path)
        if cached is not None:
            cached_time, cached_items = cached
            if now - cached_time <= self._DIR_CACHE_TTL:
                # Cache hit — retornar imediatamente sem I/O
                GLib.idle_add(self._set_store_items, cached_items, requested_path, source)
                return
        # ─────────────────────────────────────────────────────────────────
```

**Código a adicionar imediatamente antes do `GLib.idle_add(self._set_store_items, ...)` existente
(linha ~2471):**
```python
            # ── Popular cache (Causa 1) ────────────────────────────────────
            # Política LRU simples: se atingiu o limite, remover a entrada mais antiga
            if len(self._dir_cache) >= self._DIR_CACHE_MAX:
                oldest_key = min(self._dir_cache, key=lambda k: self._dir_cache[k][0])
                del self._dir_cache[oldest_key]
            self._dir_cache[requested_path] = (time.monotonic(), list(all_items))
            # ──────────────────────────────────────────────────────────────
```

**Atenção:** O `time` já está importado no topo do arquivo (linha 16). Não é necessário adicionar
import.

---

### 4.3 — Mudança C: Método `_invalidate_dir_cache()` (novo método privado)

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Adicione este método novo logo após `_set_store_items()` (por volta da linha 2678):

```python
    def _invalidate_dir_cache(self, path: str = None) -> None:
        """Invalida entradas do cache de diretório após operações de arquivo.

        Se 'path' for fornecido, invalida apenas aquele caminho e seu pai.
        Se 'path' for None, limpa todo o cache (usado em refresh manual).

        Args:
            path: Caminho absoluto do diretório afetado, ou None para limpeza total.
        """
        if path is None:
            self._dir_cache.clear()
            return

        # Invalidar o diretório afetado
        self._dir_cache.pop(path, None)

        # Invalidar o pai também (pois o conteúdo do pai lista este diretório)
        parent = str(Path(path).parent)
        if parent and parent != path:
            self._dir_cache.pop(parent, None)
```

---

### 4.4 — Mudança D: Invalidar Cache nas Chamadas de `refresh()`

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o método `refresh()` (linha 2370). Modifique-o para invalidar o cache quando o
refresh for explícito (botão refresh, após operações de arquivo). **Não** invalidar em
navegações normais (source="filemanager" + mudança de path).

**Código atual do `refresh()` (linhas 2370–2386):**
```python
    def refresh(
        self, path: str = None, source: str = "filemanager", clear_search: bool = True
    ):
        if hasattr(self, "search_entry") and clear_search:
            self.search_entry.set_text("")
        if path:
            self.current_path = path
        self._update_breadcrumb()

        if hasattr(self, "search_entry"):
            self.search_entry.set_sensitive(False)
            self.search_entry.set_placeholder_text(_("Loading..."))

        # Use global AsyncTaskManager for I/O-bound file listing
        AsyncTaskManager.get().submit_io(
            self._list_files_thread, self.current_path, source
        )
```

**Substitua por:**
```python
    def refresh(
        self, path: str = None, source: str = "filemanager", clear_search: bool = True
    ):
        if hasattr(self, "search_entry") and clear_search:
            self.search_entry.set_text("")
        if path:
            self.current_path = path
        self._update_breadcrumb()

        # Invalidar cache quando o usuário forçar refresh explícito (sem mudança de path)
        # Navegações normais (com path diferente) reusam o cache via _list_local_files
        if path is None and hasattr(self, "_dir_cache"):
            self._invalidate_dir_cache(self.current_path)

        if hasattr(self, "search_entry"):
            self.search_entry.set_sensitive(False)
            self.search_entry.set_placeholder_text(_("Loading..."))

        # Use global AsyncTaskManager for I/O-bound file listing
        AsyncTaskManager.get().submit_io(
            self._list_files_thread, self.current_path, source
        )
```

---

### 4.5 — Mudança E: Suspender Filtro Durante `store.splice()` (Causa 2)

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o método `_set_store_items()` (linha ~2651). Modifique a parte do `splice`.

**Código atual (linhas 2667–2669):**
```python
        if self.store is not None:
            # Single splice replaces all items - more efficient than multiple operations
            self.store.splice(0, self.store.get_n_items(), items)
```

**Substitua por:**
```python
        if self.store is not None:
            # Suspender o filtro durante o splice para evitar N recálculos.
            # O filtro é temporariamente removido do FilterListModel, o splice
            # é feito no store bruto, e o filtro é reconectado em seguida.
            # O GTK então faz um único recálculo em lote — muito mais eficiente.
            current_filter = None
            if hasattr(self, "filtered_store") and self.filtered_store is not None:
                current_filter = self.filtered_store.get_filter()
                self.filtered_store.set_filter(None)
            try:
                self.store.splice(0, self.store.get_n_items(), items)
            finally:
                if current_filter is not None and hasattr(self, "filtered_store") \
                        and self.filtered_store is not None:
                    self.filtered_store.set_filter(current_filter)
```

---

### 4.6 — Mudança F: Pre-fetch do Diretório Pai em `_set_store_items()` (extensão da Causa 1)

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Adicione o pre-fetch **ao final** de `_set_store_items()`, antes do `return False`.

**Código atual (final de `_set_store_items()`, linhas 2671–2678):**
```python
        # Track this as the last successfully listed path (for permission denied fallback)
        self._last_successful_path = requested_path

        self._showing_recursive_results = False
        self._recursive_search_in_progress = False
        self._restore_search_entry(source)
        self._update_status_bar()
        return False
```

**Substitua por:**
```python
        # Track this as the last successfully listed path (for permission denied fallback)
        self._last_successful_path = requested_path

        self._showing_recursive_results = False
        self._recursive_search_in_progress = False
        self._restore_search_entry(source)
        self._update_status_bar()

        # ── Pre-fetch do diretório pai (extensão da Causa 1) ──────────────────
        # Pré-popular o cache com o diretório pai em background, de modo que
        # ao pressionar "..", o resultado já esteja disponível instantaneamente.
        if (
            requested_path != "/"
            and not self._is_destroyed
            and not self._is_remote_session()
            and not self._recursive_search_in_progress
        ):
            parent_path = str(Path(requested_path).parent)
            if parent_path not in self._dir_cache:
                # Submeter com prioridade baixa: a thread do pool já está disponível
                AsyncTaskManager.get().submit_io(
                    self._list_local_files, parent_path, "prefetch"
                )
        # ─────────────────────────────────────────────────────────────────────

        return False
```

**ATENÇÃO:** O pre-fetch chama `_list_local_files` com `source="prefetch"`. Para que isso não
cause problemas, verifique que `_list_local_files` já tem a verificação
`if requested_path != self.current_path: return` **antes** de qualquer `GLib.idle_add`. Isso
garante que um pre-fetch para o pai não vai substituir a listagem atual.

Confirme que a verificação nas linhas ~2391–2392 existe:
```python
        if self._is_destroyed or requested_path != self.current_path:
            return
```

**Essa verificação é a proteção do pre-fetch.** Para o pre-fetch funcionar corretamente, ele
precisa apenas popular o `self._dir_cache` — mas a verificação acima vai impedir o
`GLib.idle_add(self._set_store_items, ...)` de ser chamado para o pai, porque o `current_path`
no momento da execução será o filho.

**Solução:** Modifique `_list_local_files` para separar a checagem de "executar cache" da
checagem de "chamar _set_store_items". Substitua a checagem existente no início:

```python
        # Verificação de destruição
        if self._is_destroyed:
            return

        # Para pre-fetches (source="prefetch"), apenas popular o cache — nunca atualizar UI
        is_prefetch = (source == "prefetch")

        # Para operações normais, verificar se ainda estamos no path correto
        if not is_prefetch and requested_path != self.current_path:
            return
```

E antes do `GLib.idle_add(self._set_store_items, ...)` (linha ~2471), adicionar a guarda:
```python
            # Não chamar _set_store_items para pre-fetches — apenas o cache é suficiente
            if not is_prefetch:
                GLib.idle_add(self._set_store_items, all_items, requested_path, source)
            # O cache já foi populado acima
```

---

### 4.7 — Mudança G: Breadcrumb Incremental (Causa 3)

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o método `_update_breadcrumb()` (linha ~890). Substitua o método **inteiro** pela
versão otimizada abaixo:

```python
    def _update_breadcrumb(self):
        """Atualiza o breadcrumb de forma incremental, reutilizando widgets existentes.

        Ao invés de destruir e recriar todos os botões a cada navegação, apenas
        reconstrói os widgets quando o caminho realmente mudou. Isso reduz
        alocações GTK e tempo de layout em navegações consecutivas.
        """
        path_str = self.current_path or ""

        # ── Otimização incremental: pular se o caminho não mudou ──────────────
        if path_str == getattr(self, "_last_breadcrumb_path", ""):
            # Caminho idêntico — apenas atualizar o estado visual do bookmark
            self._update_bookmark_star_ui()
            return
        self._last_breadcrumb_path = path_str
        # ─────────────────────────────────────────────────────────────────────

        # Reconstrução completa quando o caminho mudou
        child = self.breadcrumb_box.get_first_child()
        while child:
            self.breadcrumb_box.remove(child)
            child = self.breadcrumb_box.get_first_child()

        path = Path(path_str)

        if not path.parts or path.parts == ("/",):
            btn = Gtk.Button(label="/")
            btn.add_css_class("flat")
            btn.connect("clicked", self._on_breadcrumb_button_clicked, "/")
            self.breadcrumb_box.append(btn)
            self._update_bookmark_star_ui()
            self._quick_jump_needs_update = True
            return

        accumulated_path = Path()
        for i, part in enumerate(path.parts):
            display_name = part if i > 0 else "/"
            if i == 0 and part == "/":
                accumulated_path = Path(part)
            else:
                accumulated_path = accumulated_path / part
                separator = Gtk.Label(label="›")
                separator.add_css_class("dim-label")
                self.breadcrumb_box.append(separator)

            btn = Gtk.Button(label=display_name)
            btn.add_css_class("flat")
            btn.connect(
                "clicked", self._on_breadcrumb_button_clicked, str(accumulated_path)
            )
            self.breadcrumb_box.append(btn)

        self._update_bookmark_star_ui()
        self._quick_jump_needs_update = True
```

---

### 4.8 — Mudança H: `disk_usage` Assíncrono (Causa 4)

**Arquivo:** `src/onyxsh/filemanager/manager.py`

Localize o método `_get_free_disk_space_text()` (linha ~2882). Substitua o método **inteiro**
pela versão abaixo que **nunca bloqueia a UI thread**:

```python
    def _get_free_disk_space_text(self) -> str:
        """Retorna o espaço livre formatado para o caminho atual, com cache.

        Esta versão é 100% não-bloqueante: retorna sempre o valor em cache
        (ou string vazia se ainda não disponível) e agenda cálculo assíncrono
        quando o cache expirou ou o caminho mudou.
        """
        try:
            now = time.monotonic()
            if not self.session_item or self.session_item.is_local():
                path_to_check = self.current_path or os.path.expanduser("~")
                cache_key = path_to_check

                if cache_key in self._disk_usage_cache:
                    cached_time, cached_val = self._disk_usage_cache[cache_key]
                    if now - cached_time < 30.0:   # TTL aumentado para 30s
                        return cached_val
                    # Cache expirado — agendar recálculo assíncrono se não em progresso

                # Cache miss ou expirado — agendar cálculo em background
                if not getattr(self, "_disk_space_calculating", False):
                    self._disk_space_calculating = True
                    path_snapshot = path_to_check  # capturar por closure

                    def _calculate():
                        try:
                            p = path_snapshot if os.path.exists(path_snapshot) else "/"
                            usage = shutil.disk_usage(p)
                            formatted = self._format_bytes(usage.free)
                            self._disk_usage_cache[path_snapshot] = (
                                time.monotonic(), formatted
                            )

                            def _update_ui():
                                self._disk_space_calculating = False
                                if not self._is_destroyed:
                                    self._update_status_bar()
                                return False

                            GLib.idle_add(_update_ui)
                        except Exception:
                            self._disk_space_calculating = False

                    AsyncTaskManager.get().submit_io(_calculate)

                # Retornar valor em cache expirado (melhor que "" durante o cálculo)
                if cache_key in self._disk_usage_cache:
                    return self._disk_usage_cache[cache_key][1]
                return ""  # Nenhum valor disponível ainda
            else:
                # Sessões remotas: usar cache existente sem bloquear
                if (
                    hasattr(self, "_remote_free_space_cache")
                    and self.current_path in self._remote_free_space_cache
                ):
                    return self._remote_free_space_cache[self.current_path]
                return ""
        except Exception:
            return ""
```

---

## 5. Invalidação de Cache em Operações de Arquivo

Para garantir que o cache não fique stale após o usuário criar, renomear ou deletar arquivos,
é necessário chamar `_invalidate_dir_cache()` nos pontos certos.

**Busque todas as chamadas de `self.refresh(source="filemanager")` que ocorrem após operações
de escrita** (não após navegação). Os locais são:

| Linha (aprox.) | Contexto | Ação |
|---|---|---|
| 2321 | `_execute_verified_command` para não-cd (create, delete, rename) | Adicionar `self._invalidate_dir_cache(self.current_path)` antes |
| 4221 | Download concluído para diretório local | Adicionar `self._invalidate_dir_cache(self.current_path)` antes |
| 4419 | Upload concluído para sessão remota | N/A (sessão remota, cache não se aplica) |
| 3309 | `on_file_saved` no QuickLook | Adicionar `self._invalidate_dir_cache(self.current_path)` antes |

**Padrão de invalidação a seguir em cada um desses pontos:**
```python
# ANTES de chamar self.refresh()
if hasattr(self, "_invalidate_dir_cache"):
    self._invalidate_dir_cache(self.current_path)
self.refresh(source="filemanager")
```

**Para a linha 2321 especificamente** (dentro de `_execute_verified_command`):
```python
        # Para não-cd commands, success is confirmed by the refresh completing
        if command_type != "cd":
            def _invalidate_and_refresh():
                self._invalidate_dir_cache(self.current_path)
                return self.refresh(source="filemanager")
            GLib.timeout_add(15, _invalidate_and_refresh)
```

---

## 6. Novos Testes Unitários a Criar

Crie o arquivo `tests/test_filemanager_cache.py` com o seguinte conteúdo:

```python
# tests/test_filemanager_cache.py
"""Testes unitários para o cache de diretório do FileManager (Causa 1).

Cobre: cache hit, cache miss, TTL expirado, limite LRU, invalidação por path,
invalidação total, e integração com o pre-fetch do pai.
"""

import time
import unittest
from datetime import datetime
from pathlib import Path
from unittest.mock import MagicMock, patch, call

from onyxsh.filemanager.models import FileItem


def _make_item(name: str, is_dir: bool = False) -> FileItem:
    """Helper: cria um FileItem mínimo para os testes."""
    perms = "drwxr-xr-x" if is_dir else "-rw-r--r--"
    return FileItem(name, perms, 4096, datetime.now(), "user", "group")


class TestDirectoryCache(unittest.TestCase):
    """Testa a lógica de cache de diretório sem iniciar GTK."""

    def _make_fm(self):
        """Cria uma instância mínima de FileManager sem interface gráfica."""
        from onyxsh.filemanager import manager as m

        fm = object.__new__(m.FileManager)
        # Estado mínimo necessário
        fm._dir_cache = {}
        fm._DIR_CACHE_TTL = 3.0
        fm._DIR_CACHE_MAX = 5
        fm._is_destroyed = False
        fm._last_breadcrumb_path = ""
        fm._disk_space_calculating = False
        fm._disk_usage_cache = {}
        fm.current_path = "/home/test"
        fm.session_item = MagicMock()
        fm.session_item.is_local.return_value = True
        fm.logger = MagicMock()
        return fm

    # ── Testes de cache básico ─────────────────────────────────────────────

    def test_cache_starts_empty(self):
        """Cache começa vazio na inicialização."""
        fm = self._make_fm()
        self.assertEqual(len(fm._dir_cache), 0)

    def test_invalidate_specific_path_removes_entry(self):
        """_invalidate_dir_cache(path) remove apenas aquele caminho e seu pai."""
        fm = self._make_fm()
        items = [_make_item("file.txt")]
        now = time.monotonic()
        fm._dir_cache["/home/test"] = (now, items)
        fm._dir_cache["/home"] = (now, items)
        fm._dir_cache["/other"] = (now, items)

        fm._invalidate_dir_cache("/home/test")

        self.assertNotIn("/home/test", fm._dir_cache)
        self.assertNotIn("/home", fm._dir_cache)   # pai também invalidado
        self.assertIn("/other", fm._dir_cache)      # outros não afetados

    def test_invalidate_none_clears_all(self):
        """_invalidate_dir_cache(None) limpa o cache inteiro."""
        fm = self._make_fm()
        now = time.monotonic()
        fm._dir_cache["/home/test"] = (now, [])
        fm._dir_cache["/tmp"] = (now, [])

        fm._invalidate_dir_cache(None)

        self.assertEqual(len(fm._dir_cache), 0)

    def test_invalidate_nonexistent_path_is_safe(self):
        """Invalidar um caminho que não está no cache não deve lançar exceção."""
        fm = self._make_fm()
        try:
            fm._invalidate_dir_cache("/nonexistent/path")
        except Exception as e:
            self.fail(f"_invalidate_dir_cache lançou exceção inesperada: {e}")

    # ── Testes de TTL ──────────────────────────────────────────────────────

    def test_cache_hit_within_ttl(self):
        """Cache hit deve ocorrer quando o timestamp é mais recente que o TTL."""
        fm = self._make_fm()
        items = [_make_item("file.txt")]
        fm._dir_cache["/home/test"] = (time.monotonic(), items)

        cached = fm._dir_cache.get("/home/test")
        self.assertIsNotNone(cached)
        cached_time, cached_items = cached
        self.assertLessEqual(time.monotonic() - cached_time, fm._DIR_CACHE_TTL)
        self.assertEqual(cached_items, items)

    def test_cache_miss_when_ttl_expired(self):
        """Cache miss deve ocorrer quando o timestamp é mais antigo que o TTL."""
        fm = self._make_fm()
        items = [_make_item("file.txt")]
        # Simular entrada com timestamp expirado
        expired_time = time.monotonic() - (fm._DIR_CACHE_TTL + 1.0)
        fm._dir_cache["/home/test"] = (expired_time, items)

        cached = fm._dir_cache.get("/home/test")
        self.assertIsNotNone(cached)
        cached_time, _ = cached
        # O tempo transcorrido deve ser maior que o TTL
        self.assertGreater(time.monotonic() - cached_time, fm._DIR_CACHE_TTL)

    # ── Testes de limite LRU ───────────────────────────────────────────────

    def test_lru_evicts_oldest_entry_when_full(self):
        """Quando o cache atinge _DIR_CACHE_MAX, a entrada mais antiga deve ser removida."""
        fm = self._make_fm()
        now = time.monotonic()

        # Popular o cache até o limite
        for i in range(fm._DIR_CACHE_MAX):
            path = f"/dir_{i}"
            fm._dir_cache[path] = (now + i, [_make_item(f"file_{i}.txt")])

        self.assertEqual(len(fm._dir_cache), fm._DIR_CACHE_MAX)

        # Simular adição de nova entrada com lógica LRU
        new_path = "/new_dir"
        if len(fm._dir_cache) >= fm._DIR_CACHE_MAX:
            oldest_key = min(fm._dir_cache, key=lambda k: fm._dir_cache[k][0])
            del fm._dir_cache[oldest_key]

        fm._dir_cache[new_path] = (time.monotonic(), [])

        # Cache não deve exceder o limite
        self.assertEqual(len(fm._dir_cache), fm._DIR_CACHE_MAX)
        # A entrada mais antiga (/dir_0, com now+0) deve ter sido removida
        self.assertNotIn("/dir_0", fm._dir_cache)
        # A nova entrada deve estar presente
        self.assertIn(new_path, fm._dir_cache)

    def test_cache_does_not_exceed_max_size(self):
        """Cache nunca deve ter mais entradas que _DIR_CACHE_MAX após múltiplas inserções."""
        fm = self._make_fm()
        now = time.monotonic()

        # Inserir mais entradas do que o limite, simulando a lógica LRU
        for i in range(fm._DIR_CACHE_MAX + 10):
            path = f"/dir_{i}"
            if len(fm._dir_cache) >= fm._DIR_CACHE_MAX:
                oldest_key = min(fm._dir_cache, key=lambda k: fm._dir_cache[k][0])
                del fm._dir_cache[oldest_key]
            fm._dir_cache[path] = (now + i, [])

        self.assertLessEqual(len(fm._dir_cache), fm._DIR_CACHE_MAX)

    # ── Testes de invalidação por operações de arquivo ─────────────────────

    def test_invalidate_removes_child_and_parent(self):
        """Invalidar /home/test deve remover /home/test e /home do cache."""
        fm = self._make_fm()
        now = time.monotonic()
        fm._dir_cache["/home/test"] = (now, [])
        fm._dir_cache["/home"] = (now, [])
        fm._dir_cache["/home/other"] = (now, [])

        fm._invalidate_dir_cache("/home/test")

        self.assertNotIn("/home/test", fm._dir_cache)
        self.assertNotIn("/home", fm._dir_cache)
        self.assertIn("/home/other", fm._dir_cache)

    def test_invalidate_root_does_not_raise(self):
        """Invalidar '/' não deve causar erro (pai de '/' é '/')."""
        fm = self._make_fm()
        fm._dir_cache["/"] = (time.monotonic(), [])
        try:
            fm._invalidate_dir_cache("/")
        except Exception as e:
            self.fail(f"Exceção ao invalidar raiz: {e}")

    # ── Testes de breadcrumb incremental ──────────────────────────────────

    def test_last_breadcrumb_path_starts_empty(self):
        """_last_breadcrumb_path deve começar como string vazia."""
        fm = self._make_fm()
        self.assertEqual(fm._last_breadcrumb_path, "")

    def test_breadcrumb_path_tracking(self):
        """_last_breadcrumb_path deve ser atualizado após navegação."""
        fm = self._make_fm()
        fm._last_breadcrumb_path = "/home/test"
        self.assertEqual(fm._last_breadcrumb_path, "/home/test")

    def test_breadcrumb_skip_condition(self):
        """Deve retornar True ao tentar navegar para o mesmo caminho (sem-op)."""
        fm = self._make_fm()
        fm._last_breadcrumb_path = "/home/test"
        same_path = "/home/test"
        should_skip = (same_path == fm._last_breadcrumb_path)
        self.assertTrue(should_skip)

    def test_breadcrumb_rebuild_condition(self):
        """Deve retornar False quando o caminho muda (exige rebuild)."""
        fm = self._make_fm()
        fm._last_breadcrumb_path = "/home/test"
        new_path = "/home/test/src"
        should_skip = (new_path == fm._last_breadcrumb_path)
        self.assertFalse(should_skip)


class TestDiskSpaceAsync(unittest.TestCase):
    """Testa que _get_free_disk_space_text() não bloqueia a UI thread."""

    def _make_fm(self):
        from onyxsh.filemanager import manager as m
        fm = object.__new__(m.FileManager)
        fm._disk_usage_cache = {}
        fm._disk_space_calculating = False
        fm._is_destroyed = False
        fm.current_path = "/home/test"
        fm.session_item = MagicMock()
        fm.session_item.is_local.return_value = True
        fm.logger = MagicMock()
        return fm

    def test_returns_empty_string_when_cache_empty(self):
        """Deve retornar '' quando cache está vazio e cálculo ainda não iniciou."""
        fm = self._make_fm()
        # Monkeypatching AsyncTaskManager para não submeter tarefas reais
        with patch("onyxsh.filemanager.manager.AsyncTaskManager") as mock_atm:
            mock_atm.get.return_value.submit_io = MagicMock()
            result = fm._get_free_disk_space_text()
        # Deve retornar string vazia (não bloquear esperando I/O)
        self.assertIsInstance(result, str)

    def test_returns_cached_value_within_ttl(self):
        """Deve retornar o valor em cache sem agendar novo cálculo."""
        fm = self._make_fm()
        fm._disk_usage_cache["/home/test"] = (time.monotonic(), "42.0 GB")

        with patch("onyxsh.filemanager.manager.AsyncTaskManager") as mock_atm:
            mock_atm.get.return_value.submit_io = MagicMock()
            result = fm._get_free_disk_space_text()
            # submit_io NÃO deve ter sido chamado (cache hit)
            mock_atm.get.return_value.submit_io.assert_not_called()

        self.assertEqual(result, "42.0 GB")

    def test_does_not_block_ui_thread(self):
        """_get_free_disk_space_text() deve retornar imediatamente (< 5ms)."""
        fm = self._make_fm()
        with patch("onyxsh.filemanager.manager.AsyncTaskManager") as mock_atm:
            mock_atm.get.return_value.submit_io = MagicMock()
            start = time.monotonic()
            fm._get_free_disk_space_text()
            elapsed_ms = (time.monotonic() - start) * 1000
        self.assertLess(elapsed_ms, 5.0, "Método bloqueou a UI thread por mais de 5ms")


if __name__ == "__main__":
    unittest.main()
```

---

## 7. Checklist de Verificação Final

Após implementar todas as mudanças, execute:

```bash
PYTHONPATH=src python3 -m unittest discover -s tests -v 2>&1 | tail -20
```

Resultado esperado:
```
...
OK
Ran XX tests in X.XXXs
```

### Verificações Manuais

1. **Cache funciona:** Navegar para `/usr/lib`, voltar com `..`, entrar novamente em `/usr/lib` →
   a segunda entrada deve ser visivelmente instantânea.

2. **Pre-fetch funciona:** Navegar para qualquer diretório com subdiretórios → ao pressionar `..`
   imediatamente, deve aparecer sem delay.

3. **Invalidação funciona:** Criar um arquivo via contexto, verificar que ele aparece na listagem
   sem precisar de refresh manual.

4. **UI não trava:** Navegar para um diretório no root filesystem (`/proc`, `/sys`) → a barra de
   status não deve congelar.

5. **Testes novos passam:** `test_filemanager_cache.py` deve ter 100% de aprovação.

---

## 8. Arquivos Modificados (Resumo)

| Arquivo | Tipo | Mudanças |
|---|---|---|
| `src/onyxsh/filemanager/manager.py` | MODIFY | Mudanças A–H (cache, splice, breadcrumb, disk_usage) |
| `tests/test_filemanager_cache.py` | NEW | Testes unitários para cache, LRU, TTL, breadcrumb |

---

## 9. Restrições e Cuidados

- **NÃO** habilitar cache para sessões remotas (`_is_remote_session() == True`). O pre-fetch e
  o cache do `_list_local_files` já são protegidos pela verificação `not self._is_remote_session()`.

- **NÃO** modificar o comportamento do `_list_files_thread` para sessões remotas (SSH/SFTP) —
  apenas o caminho local (`_list_local_files`) deve ter cache.

- **NÃO** alterar a assinatura pública de nenhum método existente — os testes existentes devem
  continuar passando sem modificação.

- O `Path` já está importado (`from pathlib import Path`) — não reimportar.

- O `time` já está importado (`import time`) — não reimportar.

- O `shutil` já está importado (`import shutil`) — não reimportar.
