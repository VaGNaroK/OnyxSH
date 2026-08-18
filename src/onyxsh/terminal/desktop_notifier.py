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
        enabled = self.settings_manager.get("notify_long_commands", True)
        threshold = float(
            self.settings_manager.get("notify_long_commands_threshold", 10)
        )
        duration = cmd.duration if cmd.duration is not None else 0.0
        condition = self.settings_manager.get(
            "notify_long_commands_condition", "unfocused"
        )

        self.logger.debug(
            f"Evaluating should_notify: enabled={enabled}, duration={duration:.2f}s, threshold={threshold:.1f}s, condition={condition}"
        )

        if not enabled:
            return False

        if duration < threshold:
            return False

        if condition == "always":
            return True

        # Unfocused condition: check if window is minimized, in background, or tab/terminal is not focused
        if window:
            is_window_active = (
                window.is_active() if hasattr(window, "is_active") else True
            )
            is_window_mapped = (
                window.get_mapped() if hasattr(window, "get_mapped") else True
            )
            has_terminal_focus = (
                terminal.has_focus() if hasattr(terminal, "has_focus") else True
            )

            is_tab_active = True
            if hasattr(window, "tab_manager") and window.tab_manager:
                selected_term = (
                    window.tab_manager.get_selected_terminal()
                )
                if selected_term is not None and selected_term != terminal:
                    is_tab_active = False

            self.logger.debug(
                f"Focus state: window_active={is_window_active}, window_mapped={is_window_mapped}, tab_active={is_tab_active}, term_focus={has_terminal_focus}"
            )

            # If the window is minimized, not active, tab is not selected, or terminal lost focus
            if (
                not is_window_active
                or not is_window_mapped
                or not is_tab_active
                or not has_terminal_focus
            ):
                return True

            # If user is actively typing/looking at this specific terminal, suppress
            return False

        return True

    def send_test_notification(self, window: Optional[Any] = None) -> bool:
        """Sends an immediate test notification."""
        print("[NOTIF-DEBUG] Manual test notification triggered!", flush=True)
        app = Gio.Application.get_default()
        if not app and window and hasattr(window, "get_application"):
            app = window.get_application()

        title = f"✅ {_('Command Completed')} (0)"
        body = f"sleep 11\n⏱ 11.2s • test"

        # 1. Gio.Notification
        if app:
            notif = Gio.Notification.new(title)
            notif.set_body(body)
            notif.set_icon(
                Gio.ThemedIcon.new_with_default_fallbacks(
                    "utilities-terminal-symbolic"
                )
            )
            notif_id = f"onyxsh-test-{int(GLib.get_monotonic_time() / 1000)}"
            try:
                app.send_notification(notif_id, notif)
                self.logger.info(f"Gio.Notification test sent: {notif_id}")
            except Exception as e:
                self.logger.warning(f"Gio.Notification test error: {e}")

        # 2. notify-send
        try:
            import shutil
            import subprocess
            from ..utils.platform import is_flatpak_sandbox

            if is_flatpak_sandbox() and shutil.which("flatpak-spawn"):
                subprocess.Popen(
                    [
                        "flatpak-spawn",
                        "--host",
                        "notify-send",
                        "-a",
                        "OnyxSH",
                        "-i",
                        "utilities-terminal",
                        "-u",
                        "normal",
                        title,
                        body,
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                self.logger.info("flatpak-spawn notify-send test dispatched to host")
            elif shutil.which("notify-send"):
                subprocess.Popen(
                    [
                        "notify-send",
                        "-a",
                        "OnyxSH",
                        "-i",
                        "utilities-terminal",
                        "-u",
                        "normal",
                        title,
                        body,
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                self.logger.info("direct notify-send test dispatched")
        except Exception as e:
            self.logger.warning(f"notify-send test error: {e}")
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
            self.logger.debug(
                f"notify_command_finished called: command='{cmd.command_text}', duration={cmd.duration:.2f}s, exit_code={cmd.exit_code}"
            )
            if not self.should_notify(terminal, cmd, window):
                return False

            app = Gio.Application.get_default()
            if not app and window and hasattr(window, "get_application"):
                app = window.get_application()
            if not app:
                self.logger.warning("No Gio.Application found for notification")

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
                clean_cwd = cmd.cwd.split("?")[0].rstrip("/")
                home_dir = str(pathlib.Path.home()).rstrip("/")
                if clean_cwd == home_dir:
                    dir_name = "~"
                elif clean_cwd.startswith(home_dir + "/"):
                    dir_name = f"~/{clean_cwd[len(home_dir)+1:]}"
                else:
                    dir_name = clean_cwd.split("/")[-1] if clean_cwd else "/"
                location_info = f" • {dir_name}"

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
            try:
                notification.set_default_action_and_target(
                    "app.focus-terminal",
                    GLib.Variant("s", str(terminal_id)),
                )
            except Exception as e:
                self.logger.debug(f"Action target setup warning: {e}")

            # 4. Dispatch via Application Portal / D-Bus with timestamp to force visual banner
            notif_id = f"onyxsh-cmd-{terminal_id}-{int(GLib.get_monotonic_time() / 1000)}"
            if app:
                try:
                    app.send_notification(notif_id, notification)
                    self.logger.info(
                        f"Dispatched long command desktop notification: {title} ({duration_str})"
                    )
                except Exception as e:
                    self.logger.warning(f"Gio.Notification error: {e}")

            # Also dispatch via notify-send for guaranteed visible desktop popup banner
            try:
                import shutil
                import subprocess
                from ..utils.platform import is_flatpak_sandbox

                urgency = "normal" if cmd.is_success else "critical"
                if is_flatpak_sandbox() and shutil.which("flatpak-spawn"):
                    subprocess.Popen(
                        [
                            "flatpak-spawn",
                            "--host",
                            "notify-send",
                            "-a",
                            "OnyxSH",
                            "-i",
                            "utilities-terminal",
                            "-u",
                            urgency,
                            title,
                            body,
                        ],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,
                    )
                elif shutil.which("notify-send"):
                    subprocess.Popen(
                        [
                            "notify-send",
                            "-a",
                            "OnyxSH",
                            "-i",
                            "utilities-terminal",
                            "-u",
                            urgency,
                            title,
                            body,
                        ],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,
                    )
            except Exception as e:
                self.logger.debug(f"notify-send dispatch error: {e}")

            # 5. Sound alert if configured
            if self.settings_manager.get("notify_long_commands_sound", True):
                if hasattr(terminal, "emit"):
                    try:
                        terminal.emit("bell")
                    except Exception:
                        pass

            return True
        except Exception as e:
            print(f"[NOTIF-DEBUG] General error in notify_command_finished: {e}", flush=True)
            return False


_global_desktop_notifier: Optional[DesktopNotifier] = None


def get_desktop_notifier() -> DesktopNotifier:
    """Returns singleton instance of DesktopNotifier."""
    global _global_desktop_notifier
    if _global_desktop_notifier is None:
        _global_desktop_notifier = DesktopNotifier()
    return _global_desktop_notifier
