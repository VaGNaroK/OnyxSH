# tests/test_filemanager_models.py
"""Comprehensive unit tests for onyxsh.filemanager.models.FileItem."""

import unittest
from datetime import datetime

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GObject

from onyxsh.filemanager.models import FileItem


class TestFileItemModels(unittest.TestCase):
    """Tests covering FileItem properties, parsing, and type classifications."""

    def test_file_item_basic_properties(self):
        dt = datetime(2026, 8, 25, 14, 30, 0)
        item = FileItem(
            name="document.pdf",
            perms="-rw-r--r--",
            size=15360,
            date=dt,
            owner="vagnarok",
            group="staff",
            is_link=False,
            link_target="",
        )

        self.assertEqual(item.name, "document.pdf")
        self.assertEqual(item.permissions, "-rw-r--r--")
        self.assertEqual(item.size, 15360)
        self.assertEqual(item.size_bytes, 15360)
        self.assertEqual(item.date, dt)
        self.assertEqual(item.formatted_date, "2026-08-25 14:30")
        self.assertEqual(item.date_modified, "2026-08-25 14:30")
        self.assertEqual(item.owner, "vagnarok")
        self.assertEqual(item.group, "staff")
        self.assertFalse(item.is_directory)
        self.assertFalse(item.is_link)
        self.assertFalse(item.is_directory_like)
        self.assertEqual(item.extension, ".pdf")
        self.assertFalse(item.is_root_owned)

    def test_file_item_formatted_size_units(self):
        # Bytes
        item_b = FileItem("f", "-rw-r--r--", 500, datetime.now(), "u", "g")
        self.assertEqual(item_b.formatted_size, "500 B")

        # Kilobytes
        item_kb = FileItem("f", "-rw-r--r--", 1536, datetime.now(), "u", "g")
        self.assertEqual(item_kb.formatted_size, "1.5 KB")

        # Megabytes
        item_mb = FileItem("f", "-rw-r--r--", int(2.5 * 1024 * 1024), datetime.now(), "u", "g")
        self.assertEqual(item_mb.formatted_size, "2.5 MB")

        # Gigabytes
        item_gb = FileItem("f", "-rw-r--r--", int(3.2 * 1024 * 1024 * 1024), datetime.now(), "u", "g")
        self.assertEqual(item_gb.formatted_size, "3.2 GB")

    def test_file_item_directories_and_symlinks(self):
        # Directory
        dir_item = FileItem("src", "drwxr-xr-x", 4096, datetime.now(), "u", "g")
        self.assertTrue(dir_item.is_directory)
        self.assertFalse(dir_item.is_link)
        self.assertTrue(dir_item.is_directory_like)
        self.assertEqual(dir_item.extension, "")
        self.assertEqual(dir_item.icon_name, "folder-symbolic")

        # File Symlink
        file_link = FileItem("link_file", "lrwxrwxrwx", 12, datetime.now(), "u", "g", is_link=True, link_target="target.txt")
        self.assertFalse(file_link.is_directory)
        self.assertTrue(file_link.is_link)
        self.assertFalse(file_link.is_directory_like)

        # Directory Symlink (with trailing slash)
        dir_link = FileItem("link_dir", "lrwxrwxrwx", 12, datetime.now(), "u", "g", is_link=True, link_target="subfolder/")
        self.assertFalse(dir_link.is_directory)
        self.assertTrue(dir_link.is_link)
        self.assertTrue(dir_link.is_directory_like)
        self.assertEqual(dir_link.icon_name, "folder-symbolic")

    def test_file_item_executability_and_scripts(self):
        # Executable by user
        exec_user = FileItem("bin1", "-rwxr-xr-x", 100, datetime.now(), "u", "g")
        self.assertTrue(exec_user.is_executable)
        self.assertTrue(exec_user.is_script_or_executable)

        # Executable by group
        exec_grp = FileItem("bin2", "-rw-r-xr--", 100, datetime.now(), "u", "g")
        self.assertTrue(exec_grp.is_executable)
        self.assertTrue(exec_grp.is_script_or_executable)

        # Executable by other
        exec_oth = FileItem("bin3", "-rw-r--r-x", 100, datetime.now(), "u", "g")
        self.assertTrue(exec_oth.is_executable)
        self.assertTrue(exec_oth.is_script_or_executable)

        # Setuid / Setgid / Sticky bit
        exec_suid = FileItem("suid", "-rwsr-xr-x", 100, datetime.now(), "u", "g")
        self.assertTrue(exec_suid.is_executable)

        # Non-executable plain file
        plain_file = FileItem("data.dat", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertFalse(plain_file.is_executable)
        self.assertFalse(plain_file.is_script_or_executable)

        # Non-executable script file (e.g. .py, .sh without chmod +x)
        script_non_exec = FileItem("script.py", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertFalse(script_non_exec.is_executable)
        self.assertTrue(script_non_exec.is_script_or_executable)

        for ext in [".sh", ".bash", ".zsh", ".pl", ".rb", ".js", ".ts", ".lua", ".php", ".bin", ".appimage"]:
            item = FileItem(f"test{ext}", "-rw-r--r--", 100, datetime.now(), "u", "g")
            self.assertTrue(item.is_script_or_executable, f"Failed for {ext}")

    def test_file_item_log_classification(self):
        log1 = FileItem("server.log", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertTrue(log1.is_log_file)

        log2 = FileItem("output.out", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertTrue(log2.is_log_file)

        log3 = FileItem("error.err", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertTrue(log3.is_log_file)

        log4 = FileItem("system.journal", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertTrue(log4.is_log_file)

        log5 = FileItem("auth.log.1", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertTrue(log5.is_log_file)

        # Directory with log name should not be considered a log file
        log_dir = FileItem("logs", "drwxr-xr-x", 4096, datetime.now(), "u", "g")
        self.assertFalse(log_dir.is_log_file)

    def test_file_item_type_badges(self):
        # Python
        item_py = FileItem("main.py", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertEqual(item_py.file_type_badge, ("PY", "badge-py"))

        # Shell
        for sh_name in ["run.sh", "init.bash", "setup.zsh"]:
            item_sh = FileItem(sh_name, "-rwxr-xr-x", 100, datetime.now(), "u", "g")
            self.assertEqual(item_sh.file_type_badge, ("SH", "badge-sh"))

        # Log
        item_log = FileItem("audit.log", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertEqual(item_log.file_type_badge, ("LOG", "badge-log"))

        # Docker
        for d_name in ["Dockerfile", "Dockerfile.prod", "docker-compose.yml", "docker-compose.yaml"]:
            item_d = FileItem(d_name, "-rw-r--r--", 100, datetime.now(), "u", "g")
            self.assertEqual(item_d.file_type_badge, ("DOCKER", "badge-docker"))

        # JSON
        item_json = FileItem("package.json", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertEqual(item_json.file_type_badge, ("JSON", "badge-json"))

        # YAML
        for y_name in ["config.yaml", "manifest.yml"]:
            item_yaml = FileItem(y_name, "-rw-r--r--", 100, datetime.now(), "u", "g")
            self.assertEqual(item_yaml.file_type_badge, ("YAML", "badge-yaml"))

        # None for directory or parent directory or other file types
        self.assertIsNone(FileItem("..", "drwxr-xr-x", 4096, datetime.now(), "u", "g").file_type_badge)
        self.assertIsNone(FileItem("unknown.xyz", "-rw-r--r--", 100, datetime.now(), "u", "g").file_type_badge)

    def test_file_item_root_owned(self):
        root_item = FileItem("passwd", "-rw-r--r--", 100, datetime.now(), "root", "root")
        self.assertTrue(root_item.is_root_owned)

        uid0_item = FileItem("shadow", "-rw-r-----", 100, datetime.now(), "0", "0")
        self.assertTrue(uid0_item.is_root_owned)

        user_item = FileItem("test.txt", "-rw-r--r--", 100, datetime.now(), "vagnarok", "vagnarok")
        self.assertFalse(user_item.is_root_owned)

    def test_from_ls_line_gnu_format(self):
        line = "-rw-r--r-- 1 user group 1024 2026-08-25 12:30 test.txt"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "test.txt")
        self.assertEqual(item.permissions, "-rw-r--r--")
        self.assertEqual(item.size, 1024)
        self.assertEqual(item.owner, "user")
        self.assertEqual(item.group, "group")
        self.assertEqual(item.date.year, 2026)
        self.assertEqual(item.date.month, 8)
        self.assertEqual(item.date.day, 25)
        self.assertEqual(item.date.hour, 12)
        self.assertEqual(item.date.minute, 30)
        self.assertFalse(item.is_link)

    def test_from_ls_line_gnu_symlink(self):
        line = "lrwxrwxrwx 1 user group 14 2026-08-25 12:30 my_link -> /var/log/syslog"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "my_link")
        self.assertTrue(item.is_link)
        self.assertEqual(item._link_target, "/var/log/syslog")

    def test_from_ls_line_eza_format(self):
        # eza format: perms links size owner group YYYY-MM-DD HH:MM name
        line = ".rw-r--r-- 1 2048 user group 2026-08-25 10:15 script.py"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "script.py")
        self.assertEqual(item.permissions, "-rw-r--r--")
        self.assertEqual(item.size, 2048)
        self.assertEqual(item.owner, "user")
        self.assertEqual(item.group, "group")

    def test_from_ls_line_with_ansi_escapes_and_classify(self):
        # Line with ANSI color escapes and classify indicator '*'
        ansi_line = "\x1b[32m-rwxr-xr-x 1 user group 512 2026-08-25 09:00 app*\x1b[0m"
        item = FileItem.from_ls_line(ansi_line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "app")
        self.assertTrue(item.is_executable)

    def test_from_ls_line_quoted_filename_with_spaces(self):
        line = "-rw-r--r-- 1 user group 4096 2026-08-25 15:45 'My Document 2026.pdf'"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "My Document 2026.pdf")
        self.assertEqual(item.size, 4096)

    def test_from_ls_line_with_timezone(self):
        line = "-rw-r--r-- 1 user group 100 2026-08-25 12:00 +0000 readme.md"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "readme.md")
        self.assertEqual(item.size, 100)

    def test_from_ls_line_regex_fallback(self):
        line = "drwxr-xr-x - user group 4096 2026-08-25 12:00:00.123456789 subfolder"
        item = FileItem.from_ls_line(line)
        self.assertIsNotNone(item)
        self.assertEqual(item.name, "subfolder")
        self.assertTrue(item.is_directory)

    def test_lazy_icon_resolution(self):
        item = FileItem("document.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertIsNone(item._cached_icon_name)
        icon = item.icon_name
        self.assertIsNotNone(icon)
        self.assertIsNotNone(item._cached_icon_name)


if __name__ == "__main__":
    unittest.main()
