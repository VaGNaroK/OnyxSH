import unittest
from unittest.mock import MagicMock, patch
from gi.repository import GLib

from onyxsh.utils.translation_utils import _
from onyxsh.window import CommTerminalWindow


class TestTerminalSearch(unittest.TestCase):
    """Test suite for terminal search logic, debounce, wrap-around, and occurrence counting."""

    def setUp(self):
        # Create a lightweight mocked CommTerminalWindow without full GTK hierarchy instantiation
        self.window = MagicMock(spec=CommTerminalWindow)

        # Attach real search methods to our mock instance
        self.window._search_debounce_source_id = None
        self.window.search_active = False
        self.window.current_search_terminal = None
        self.window.search_current_occurrence = 0
        self.window.search_total_occurrences = 0
        self.window.search_occurrences_overflow = False

        self.window._cancel_search_debounce = CommTerminalWindow._cancel_search_debounce.__get__(
            self.window, CommTerminalWindow
        )
        self.window._flush_pending_search = CommTerminalWindow._flush_pending_search.__get__(
            self.window, CommTerminalWindow
        )
        self.window._clear_occurrence_counts = CommTerminalWindow._clear_occurrence_counts.__get__(
            self.window, CommTerminalWindow
        )
        self.window._clear_search_state = CommTerminalWindow._clear_search_state.__get__(
            self.window, CommTerminalWindow
        )
        self.window._update_search_occurrence_display = (
            CommTerminalWindow._update_search_occurrence_display.__get__(
                self.window, CommTerminalWindow
            )
        )
        self.window._count_occurrences_safe = (
            CommTerminalWindow._count_occurrences_safe.__get__(self.window, CommTerminalWindow)
        )
        self.window._on_search_text_changed = (
            CommTerminalWindow._on_search_text_changed.__get__(self.window, CommTerminalWindow)
        )
        self.window._perform_debounced_search = (
            CommTerminalWindow._perform_debounced_search.__get__(self.window, CommTerminalWindow)
        )
        self.window._perform_search = CommTerminalWindow._perform_search.__get__(
            self.window, CommTerminalWindow
        )
        self.window._on_search_next = CommTerminalWindow._on_search_next.__get__(
            self.window, CommTerminalWindow
        )
        self.window._on_search_previous = CommTerminalWindow._on_search_previous.__get__(
            self.window, CommTerminalWindow
        )
        self.window._on_search_stop = CommTerminalWindow._on_search_stop.__get__(
            self.window, CommTerminalWindow
        )
        self.window._on_case_sensitive_changed = (
            CommTerminalWindow._on_case_sensitive_changed.__get__(self.window, CommTerminalWindow)
        )
        self.window._on_regex_changed = CommTerminalWindow._on_regex_changed.__get__(
            self.window, CommTerminalWindow
        )
        self.window._on_whole_word_changed = (
            CommTerminalWindow._on_whole_word_changed.__get__(self.window, CommTerminalWindow)
        )

        # Mock UI widgets
        self.window.terminal_search_entry = MagicMock()
        self.window.search_occurrence_label = MagicMock()
        self.window.search_bar = MagicMock()
        self.window.toast_overlay = MagicMock()
        self.window.settings_manager = MagicMock()
        self.window.settings_manager.get.side_effect = lambda key, default=None: {
            "search_case_sensitive": False,
            "search_use_regex": False,
            "search_whole_word": False,
        }.get(key, default)
        self.window.tab_manager = MagicMock()
        self.mock_terminal = MagicMock()
        self.window.tab_manager.get_selected_terminal.return_value = self.mock_terminal
        self.window.logger = MagicMock()

    def test_search_debounce_scheduling_and_cancellation(self):
        """Typing schedules a 200ms debounce timer and cancels previous timer."""
        entry = MagicMock()
        entry.get_text.return_value = "docker"

        with patch("gi.repository.GLib.timeout_add", return_value=123) as mock_timeout, \
             patch("gi.repository.GLib.source_remove") as mock_remove:
            self.window._on_search_text_changed(entry)
            self.assertEqual(self.window._search_debounce_source_id, 123)
            mock_timeout.assert_called_once_with(
                200, self.window._perform_debounced_search, "docker"
            )

            # Typing another character replaces the debounce timer
            entry.get_text.return_value = "docker build"
            mock_timeout.return_value = 456
            self.window._on_search_text_changed(entry)
            mock_remove.assert_called_once_with(123)
            self.assertEqual(self.window._search_debounce_source_id, 456)

    def test_flush_pending_search_on_enter_or_action(self):
        """Flushing executes search immediately and cancels pending debounce."""
        self.window._search_debounce_source_id = 999
        self.window.terminal_search_entry.get_text.return_value = "hello"

        with patch.object(self.window, "_perform_search") as mock_search, \
             patch("gi.repository.GLib.source_remove") as mock_remove:
            self.window._flush_pending_search()
            mock_remove.assert_called_once_with(999)
            self.assertIsNone(self.window._search_debounce_source_id)
            mock_search.assert_called_once_with("hello")

    def test_empty_search_text_clears_state_immediately(self):
        """Empty search text clears terminal regex and state without debounce."""
        entry = MagicMock()
        entry.get_text.return_value = ""

        self.window._search_debounce_source_id = 888
        with patch("gi.repository.GLib.source_remove") as mock_remove:
            self.window._on_search_text_changed(entry)
            mock_remove.assert_called_once_with(888)
            self.assertFalse(self.window.search_active)
            self.mock_terminal.search_set_regex.assert_called_with(None, 0)
            self.assertEqual(self.window.search_total_occurrences, 0)

    def test_count_occurrences_safe_normal(self):
        """Safely counts normal occurrences without instantiating list of matches."""
        self.mock_terminal.get_text_format.return_value = (
            "alpha line 1\nbeta line 2\nalpha line 3\nalpha line 4"
        )
        self.window._count_occurrences_safe(
            self.mock_terminal, r"alpha", case_sensitive=False, found_in_vte=True
        )
        self.assertEqual(self.window.search_total_occurrences, 3)
        self.assertFalse(self.window.search_occurrences_overflow)

    def test_count_occurrences_safe_overflow_ceiling(self):
        """Large number of matches caps at ceiling of 1000 and sets overflow flag."""
        # Generate string with 1500 'word' occurrences
        large_buffer = "word " * 1500
        self.mock_terminal.get_text_format.return_value = large_buffer
        self.window._count_occurrences_safe(
            self.mock_terminal, r"word", case_sensitive=False, found_in_vte=True
        )
        self.assertEqual(self.window.search_total_occurrences, 1000)
        self.assertTrue(self.window.search_occurrences_overflow)

    def test_occurrence_display_formatting(self):
        """Occurrence label formats normal counts, overflow counts, and no matches."""
        self.window.search_active = True
        self.window.search_total_occurrences = 10
        self.window.search_current_occurrence = 2
        self.window.search_occurrences_overflow = False

        self.window._update_search_occurrence_display()
        self.window.search_occurrence_label.set_text.assert_called_with("2/10")

        # Test overflow format
        self.window.search_total_occurrences = 1000
        self.window.search_current_occurrence = 1
        self.window.search_occurrences_overflow = True
        self.window._update_search_occurrence_display()
        self.window.search_occurrence_label.set_text.assert_called_with("1/1000+")

        # Test no matches
        self.window.search_total_occurrences = 0
        self.window.search_current_occurrence = 0
        self.window.search_occurrences_overflow = False
        self.window._update_search_occurrence_display()
        self.window.search_occurrence_label.set_text.assert_called_with(_("No matches"))

    def test_search_navigation_next_and_previous(self):
        """Navigation uses VTE search and cycles occurrences seamlessly."""
        self.window.terminal_search_entry.get_text.return_value = "grep"
        self.window.search_active = True
        self.window.search_total_occurrences = 3
        self.window.search_current_occurrence = 1
        self.mock_terminal.search_find_next.return_value = True
        self.mock_terminal.search_find_previous.return_value = True

        # Click Next
        self.window._on_search_next()
        self.assertEqual(self.window.search_current_occurrence, 2)

        # Click Next again
        self.window._on_search_next()
        self.assertEqual(self.window.search_current_occurrence, 3)

        # Click Next wraps around to 1
        self.window._on_search_next()
        self.assertEqual(self.window.search_current_occurrence, 1)

        # Click Previous wraps around to 3
        self.window._on_search_previous()
        self.assertEqual(self.window.search_current_occurrence, 3)

        # Click Previous moves to 2
        self.window._on_search_previous()
        self.assertEqual(self.window.search_current_occurrence, 2)

    def test_perform_search_enables_wrap_around_and_sets_regex(self):
        """_perform_search enables native wrap_around on the terminal."""
        self.mock_terminal.get_text_format.return_value = "match1 match2"
        self.mock_terminal.search_find_next.return_value = True

        self.window._perform_search("match")

        self.mock_terminal.search_set_wrap_around.assert_called_once_with(True)
        self.assertTrue(self.window.search_active)
        self.assertEqual(self.window.search_current_occurrence, 1)
        self.assertEqual(self.window.search_total_occurrences, 2)

    def test_search_stop_cleans_up_completely(self):
        """_on_search_stop cancels debounce and cleans up terminal regex and state."""
        self.window._search_debounce_source_id = 777
        self.window.search_active = True

        with patch("gi.repository.GLib.source_remove") as mock_remove:
            self.window._on_search_stop(self.window.terminal_search_entry)
            mock_remove.assert_called_once_with(777)
            self.assertFalse(self.window.search_active)
            self.assertIsNone(self.window.current_search_terminal)
            self.mock_terminal.search_set_regex.assert_called_with(None, 0)
            self.mock_terminal.grab_focus.assert_called_once()
            self.assertEqual(self.window.search_total_occurrences, 0)

    def test_toggle_options_trigger_immediate_search(self):
        """Toggling case sensitive, regex or whole word flushes debounce and searches immediately."""
        self.window.terminal_search_entry.get_text.return_value = "target"
        toggle = MagicMock()
        toggle.get_active.return_value = True

        with patch.object(self.window, "_perform_search") as mock_search:
            self.window._on_case_sensitive_changed(toggle, None)
            mock_search.assert_called_with("target")

            mock_search.reset_mock()
            self.window._on_regex_changed(toggle, None)
            mock_search.assert_called_with("target")

            mock_search.reset_mock()
            self.window._on_whole_word_changed(toggle, None)
            mock_search.assert_called_with("target")


if __name__ == "__main__":
    unittest.main()
