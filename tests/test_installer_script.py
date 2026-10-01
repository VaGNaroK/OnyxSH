import os
import subprocess
import unittest
from pathlib import Path

class TestInstallerScript(unittest.TestCase):
    def setUp(self):
        self.repo_root = Path(__file__).resolve().parent.parent
        self.installer_path = self.repo_root / "install.sh"

    def test_installer_exists_and_is_executable(self):
        self.assertTrue(self.installer_path.exists(), "install.sh deve existir na raiz do repositório")
        self.assertTrue(os.access(self.installer_path, os.X_OK), "install.sh deve ter permissão de execução")

    def test_installer_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(self.installer_path)],
            capture_output=True,
            text=True
        )
        self.assertEqual(result.returncode, 0, f"Erro de sintaxe no install.sh: {result.stderr}")

    def test_installer_supports_modes(self):
        content = self.installer_path.read_text(encoding="utf-8")
        self.assertIn("build_deb.sh", content)
        self.assertIn("build_flatpak.sh", content)
        self.assertIn("install_deb", content)
        self.assertIn("install_flatpak", content)
        self.assertIn("uninstall", content)

if __name__ == "__main__":
    unittest.main()
