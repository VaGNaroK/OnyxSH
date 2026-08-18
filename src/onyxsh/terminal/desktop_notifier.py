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

        print(
            f"[NOTIF-DEBUG] Evaluating should_notify: enabled={enabled}, duration={duration:.2f}s, threshold={threshold:.1f}s, condition={condition}",
            flush=True,
        )

        if not enabled:
            print("[NOTIF-DEBUG] should_notify -> False (disabled in settings)", flush=True)
            return False

        if duration < threshold:
            print(
                f"[NOTIF-DEBUG] should_notify -> False (duration {duration:.2f}s < threshold {threshold:.1f}s)",
                flush=True,
            )
            return False

        if condition == "always":
            print("[NOTIF-DEBUG] should_notify -> True (condition == always)", flush=True)
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

            print(
                f"[NOTIF-DEBUG] Focus state: window_active={is_window_active}, window_mapped={is_window_mapped}, tab_active={is_tab_active}, term_focus={has_terminal_focus}",
                flush=True,
            )

            # If the window is minimized, not active, tab is not selected, or terminal lost focus
            if (
                not is_window_active
                or not is_window_mapped
                or not is_tab_active
                or not has_terminal_focus
            ):
                print("[NOTIF-DEBUG] should_notify -> True (unfocused condition met)", flush=True)
                return True

            # If user is actively typing/looking at this specific terminal, suppress
            print("[NOTIF-DEBUG] should_notify -> False (terminal is actively focused and window is active)", flush=True)
            return False

        print("[NOTIF-DEBUG] should_notify -> True (no window reference)", flush=True)
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
                print(
                    f"[NOTIF-DEBUG] Gio.Notification test sent successfully: {notif_id}",
                    flush=True,
                )
            except Exception as e:
                print(
                    f"[NOTIF-DEBUG] Gio.Notification test error: {e}",
                    flush=True,
                )

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
                print(
                    "[NOTIF-DEBUG] flatpak-spawn notify-send test dispatched to host!",
                    flush=True,
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
                        "normal",
                        title,
                        body,
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                print(
                    "[NOTIF-DEBUG] direct notify-send test dispatched!",
                    flush=True,
                )
        except Exception as e:
            print(f"[NOTIF-DEBUG] notify-send error: {e}", flush=True)
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
            print(
                f"[NOTIF-DEBUG] notify_command_finished called: command='{cmd.command_text}', duration={cmd.duration:.2f}s, exit_code={cmd.exit_code}",
                flush=True,
            )
            if not self.should_notify(terminal, cmd, window):
                return False

            app = Gio.Application.get_default()
            if not app and window and hasattr(window, "get_application"):
                app = window.get_application()
            if not app:
                print("[NOTIF-DEBUG] Warning: No Gio.Application found", flush=True)

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

            # 4. Dispatch via Application Portal / D-Bus with timestamp to force visual banner
            notif_id = f"onyxsh-cmd-{terminal_id}-{int(GLib.get_monotonic_time() / 1000)}"
            if app:
                try:
                    app.send_notification(notif_id, notification)
                    print(
                        f"[NOTIF-DEBUG] Gio.Notification sent successfully: {notif_id}",
                        flush=True,
                    )
                except Exception as e:
                    print(f"[NOTIF-DEBUG] Gio.Notification error: {e}", flush=True)

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
                    print(
                        f"[NOTIF-DEBUG] flatpak-spawn notify-send dispatched to host: title='{title}'",
                        flush=True,
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
                    print(
                        f"[NOTIF-DEBUG] direct notify-send dispatched: title='{title}'",
                        flush=True,
                    )
            except Exception as e:
                print(f"[NOTIF-DEBUG] notify-send dispatch error: {e}", flush=True)

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
