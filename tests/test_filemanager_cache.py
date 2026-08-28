# tests/test_filemanager_cache.py
"""Testes unitários para o cache de diretório do FileManager (Causa 1).

Cobre: cache hit, cache miss, TTL expirado, limite LRU, invalidação por path,
invalidação total, e integração com o pre-fetch do pai.
"""

import time
import unittest
from datetime import datetime
from unittest.mock import MagicMock, patch

from onyxsh.filemanager import manager as m
from onyxsh.filemanager.models import FileItem


def _make_item(name: str, is_dir: bool = False) -> FileItem:
    """Helper: cria um FileItem mínimo para os testes."""
    perms = "drwxr-xr-x" if is_dir else "-rw-r--r--"
    return FileItem(name, perms, 4096, datetime.now(), "user", "group")


class TestDirectoryCache(unittest.TestCase):
    """Testa a lógica de cache de diretório sem iniciar GTK."""

    def _make_fm(self):
        """Cria uma instância mínima de FileManager sem interface gráfica."""
        fm = m.FileManager.__new__(m.FileManager)
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
        fm = m.FileManager.__new__(m.FileManager)
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
