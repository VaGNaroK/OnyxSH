"""Unit tests for Flatpak manifest, runtime version alignment, and packaging script validation."""

import os
import re
import unittest
from pathlib import Path


class TestFlatpakManifest(unittest.TestCase):
    """Test suite validating Flatpak manifest configuration and script alignment."""

    def setUp(self):
        self.repo_root = Path(__file__).resolve().parents[1]
        self.manifest_path = self.repo_root / "manifests" / "io.github.vagnarok.OnyxSH.yaml"
        self.build_script_path = self.repo_root / "scripts" / "build_flatpak.sh"

    def test_manifest_file_exists(self):
        self.assertTrue(
            self.manifest_path.is_file(),
            f"Flatpak manifest not found at {self.manifest_path}",
        )

    def test_manifest_basic_metadata(self):
        content = self.manifest_path.read_text(encoding="utf-8")
        
        # Check app-id
        app_id_match = re.search(r"^app-id:\s*([^\s]+)", content, re.MULTILINE)
        self.assertIsNotNone(app_id_match, "app-id not found in manifest")
        self.assertEqual(app_id_match.group(1), "io.github.vagnarok.OnyxSH")

        # Check command
        cmd_match = re.search(r"^command:\s*([^\s]+)", content, re.MULTILINE)
        self.assertIsNotNone(cmd_match, "command not found in manifest")
        self.assertEqual(cmd_match.group(1), "onyxsh")

        # Check runtime and sdk
        self.assertIn("runtime: org.gnome.Platform", content)
        self.assertIn("sdk: org.gnome.Sdk", content)

    def test_runtime_version_not_eol(self):
        content = self.manifest_path.read_text(encoding="utf-8")
        match = re.search(
            r"^runtime-version:\s*['\"]?([^'\"\s]+)['\"]?", content, re.MULTILINE
        )
        self.assertIsNotNone(match, "runtime-version not found in manifest")
        runtime_version = match.group(1)

        # GNOME 46, 47, 48 are EOL
        eol_versions = {"46", "47", "48"}
        self.assertNotIn(
            runtime_version,
            eol_versions,
            f"Runtime version {runtime_version} is End of Life (EOL)!",
        )
        # Should be at least 50 or higher
        version_num = int(runtime_version)
        self.assertGreaterEqual(
            version_num, 50, f"Runtime version {runtime_version} is below GNOME 50"
        )
        self.assertEqual(runtime_version, "51", "Expected current stable GNOME 51 runtime")

    def test_vte_module_config(self):
        content = self.manifest_path.read_text(encoding="utf-8")
        self.assertIn("- name: vte", content)
        self.assertIn("--libdir=/app/lib", content)
        self.assertIn("-Dgtk4=true", content)
        self.assertIn("-Dgtk3=false", content)
        self.assertIn("-Dgir=true", content)

        # Check VTE download source matches vte 0.84+
        vte_url_match = re.search(
            r"url:\s*https://download\.gnome\.org/sources/vte/([0-9.]+)/vte-([0-9.]+)\.tar\.xz",
            content,
        )
        self.assertIsNotNone(vte_url_match, "VTE download URL not found or invalid format")
        vte_major_minor = float(vte_url_match.group(1))
        self.assertGreaterEqual(
            vte_major_minor, 0.80, "VTE version should be 0.80 or higher for GNOME 50+"
        )

        # Ensure sha256 is present and 64 hex characters
        sha_match = re.search(r"sha256:\s*([0-9a-fA-F]{64})", content)
        self.assertIsNotNone(sha_match, "Valid sha256 checksum not found for VTE")

    def test_onyxsh_launcher_typelib_paths(self):
        launcher_path = self.repo_root / "usr" / "bin" / "onyxsh"
        self.assertTrue(launcher_path.is_file())
        launcher_content = launcher_path.read_text(encoding="utf-8")
        self.assertIn("GI_TYPELIB_PATH", launcher_content)
        self.assertIn("LD_LIBRARY_PATH", launcher_content)
        self.assertIn("/app/lib/girepository-1.0", launcher_content)
        self.assertIn("/app/lib64/girepository-1.0", launcher_content)

    def test_finish_args_permissions(self):
        content = self.manifest_path.read_text(encoding="utf-8")
        required_args = [
            "--share=ipc",
            "--share=network",
            "--socket=x11",
            "--socket=wayland",
            "--socket=fallback-x11",
            "--device=dri",
            "--filesystem=host",
            "--filesystem=home",
            "--talk-name=org.freedesktop.Secret",
            "--talk-name=org.freedesktop.Notifications",
            "--talk-name=org.freedesktop.Flatpak",
        ]
        for arg in required_args:
            self.assertIn(arg, content, f"Missing required finish-arg permission: {arg}")

    def test_build_script_runtime_version_dynamic_extraction(self):
        self.assertTrue(self.build_script_path.is_file())
        script_content = self.build_script_path.read_text(encoding="utf-8")

        # Ensure build script doesn't hardcode EOL runtime 46
        self.assertNotIn("org.gnome.Sdk//46", script_content)
        self.assertNotIn("org.gnome.Platform//46", script_content)
        self.assertNotIn("org.gnome.Sdk.*46", script_content)

        # Ensure it reads dynamically from MANIFEST_FILE
        self.assertIn("RUNTIME_VERSION", script_content)
        self.assertIn('org.gnome.Sdk//${RUNTIME_VERSION}', script_content)
        self.assertIn('org.gnome.Platform//${RUNTIME_VERSION}', script_content)


if __name__ == "__main__":
    unittest.main()

