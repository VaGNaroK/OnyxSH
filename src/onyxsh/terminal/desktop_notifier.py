# onyxsh/terminal/desktop_notifier.py
"""
Desktop Notifier for Long-Running Commands.
Dispatches native OS notifications (via Gio.Notification / D-Bus portal)
when background or inactive tab commands finish after a configurable duration.
"""

from typing import Any, Optional

import gi

gi.require_version("Gio", "2.0")
gi.require_version("GLib", "2.0")
from gi.repository import Gio, GLib

from ..settings.manager import SettingsManager, get_settings_manager
from ..utils.logger import get_logger
from ..utils.translation_utils import _
from .semantic_tracker import SemanticCommand


class DesktopNotifier:
    """
    Evaluates command completion events and sends native desktop notifications.
    """

    def __init__(self, settings_manager: Optional[SettingsManager] = None):
        self.logger = get_logger("onyxsh.terminal.desktop_notifier")
        self.settings_manager = settings_manager or get_settings_manager()

    def should_notify(
        self,
        terminal: Any,
        cmd: SemanticCommand,
        window: Optional[Any] = None,
    ) -> bool:
        """
        Determines whether a desktop notification should be dispatched based
        on user settings, execution duration, and window/tab focus state.
        """
        if not self.settings_manager.get("notify_long_commands", True):
            return False

        # Check execution duration against threshold (default: 10 seconds)
        threshold = float(
            self.settings_manager.get("notify_long_commands_threshold", 10)
        )
        duration = cmd.duration if cmd.duration is not None else 0.0

        if duration < threshold:
            return False

        condition = self.settings_manager.get(
            "notify_long_commands_condition", "unfocused"
        )
        if condition == "always":
            return True

        # Unfocused condition: check if window is in background OR tab is inactive
        if window:
            is_window_active = (
                window.is_active() if hasattr(window, "is_active") else True
            )

            is_tab_active = True
            if hasattr(window, "tab_manager") and window.tab_manager:
                selected_term = (
                    window.tab_manager.get_selected_terminal()
                )
                if selected_term is not None and selected_term != terminal:
                    is_tab_active = False

            # If the window is not focused or the tab is not the active visible tab
            if not is_window_active or not is_tab_active:
                return True

            # If user is actively looking at the window AND the tab is focused, no need to spam
            return False

        return True

    def notify_command_finished(
        self,
        terminal: Any,
        cmd: SemanticCommand,
        window: Optional[Any] = None,
    ) -> bool:
        """
        Builds and sends a native desktop notification for a completed command.
        """
        try:
            if not self.should_notify(terminal, cmd, window):
                return False

            app = Gio.Application.get_default()
            if not app:
                return False

            terminal_id = getattr(terminal, "terminal_id", 0)

            # 1. Format Title
            if cmd.is_success:
                title = f"✅ {_('Command Completed')} (0)"
            else:
                exit_str = (
                    str(cmd.exit_code) if cmd.exit_code is not None else "?"
                )
                title = f"❌ {_('Command Failed')} ({exit_str})"

            # 2. Format Body
            cmd_text = (cmd.command_text or _("Terminal command")).strip()
            if len(cmd_text) > 65:
                cmd_text = f"{cmd_text[:62]}..."

            duration_str = cmd.formatted_duration or f"{cmd.duration:.1f}s"

            location_info = ""
            if hasattr(terminal, "onyxsh_session"):
                sess = getattr(terminal, "onyxsh_session", None)
                if sess and getattr(sess, "host", None):
                    location_info = f" • {sess.host}"
            if not location_info and cmd.cwd:
                location_info = f" • {cmd.cwd.split('/')[-1] or '/'}"

            body = f"{cmd_text}\n⏱ {duration_str}{location_info}"

            # 3. Create Notification
            notification = Gio.Notification.new(title)
            notification.set_body(body)
            notification.set_icon(
                Gio.ThemedIcon.new_with_default_fallbacks(
                    "utilities-terminal-symbolic"
                )
            )

            # Set interactive click action to focus the terminal/tab
            notification.set_default_action_and_target_value(
                "app.focus-terminal",
                GLib.Variant("s", str(terminal_id)),
            )

            # 4. Dispatch via Application Portal / D-Bus
            notif_id = f"onyxsh-cmd-{terminal_id}"
            app.send_notification(notif_id, notification)
            self.logger.info(
                f"Dispatched long command desktop notification: {title} ({duration_str})"
            )

            # 5. Sound alert if configured
            if self.settings_manager.get("notify_long_commands_sound", True):
                if hasattr(terminal, "emit"):
                    # Trigger terminal bell/sound
                    try:
                        terminal.emit("bell")
                    except Exception:
                        pass

            return True
        except Exception as e:
            self.logger.warning(
                f"Failed to dispatch desktop notification: {e}"
            )
            return False


_global_desktop_notifier: Optional[DesktopNotifier] = None


def get_desktop_notifier() -> DesktopNotifier:
    """Returns singleton instance of DesktopNotifier."""
    global _global_desktop_notifier
    if _global_desktop_notifier is None:
        _global_desktop_notifier = DesktopNotifier()
    return _global_desktop_notifier
