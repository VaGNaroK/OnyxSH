# tests/test_icon_theme.py
import unittest
import os
import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk

from onyxsh.utils.icons import (
    get_icon_path,
    has_bundled_icon,
    create_icon_image,
    create_icon_button,
    ensure_icon_theme_registered,
)


class TestIconTheme(unittest.TestCase):

    def setUp(self):
        ensure_icon_theme_registered()

    def test_bundled_icons_exist(self):
        expected_icons = [
            "view-reveal-symbolic",
            "starred-symbolic",
            "document-open-recent-symbolic",
            "view-refresh-symbolic",
            "folder-saved-search-symbolic",
            "go-up-symbolic",
        ]
        for icon_name in expected_icons:
            self.assertTrue(
                has_bundled_icon(icon_name),
                f"Bundled icon {icon_name} should exist in icons dir",
            )
            path = get_icon_path(icon_name)
            self.assertIsNotNone(path)
            self.assertTrue(os.path.isfile(path))

    def test_create_icon_image(self):
        img = create_icon_image("view-reveal-symbolic", size=16)
        self.assertIsInstance(img, Gtk.Image)
        self.assertEqual(img.get_pixel_size(), 16)

    def test_create_icon_button(self):
        btn = create_icon_button("starred-symbolic", size=16, tooltip="Star")
        self.assertIsInstance(btn, Gtk.Button)


if __name__ == "__main__":
    unittest.main()
