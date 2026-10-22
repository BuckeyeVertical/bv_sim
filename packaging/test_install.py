"""Exercise installer relocation, shell quoting, and idempotent PATH setup."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class InstallTest(unittest.TestCase):
    def test_installed_command_survives_bundle_removal(self):
        source = Path(__file__).resolve().parent
        with tempfile.TemporaryDirectory(prefix="bv-sim-installer-") as temporary:
            root = Path(temporary)
            home = root / "a user's home"
            home.mkdir()
            bundle = root / "Downloads" / "release"
            (bundle / "bin").mkdir(parents=True)
            (bundle / "assets").mkdir()
            (bundle / "scenarios").mkdir()
            (bundle / "assets" / "asset.txt").write_text("asset")
            (bundle / "scenarios" / "suas_2026.toml").write_text("scenario")
            (bundle / "VERSION").write_text("0.1.0\n")
            binary = bundle / "bin" / "bv-sim"
            binary.write_text('#!/bin/sh\nprintf "ok:%s\\n" "$1"\n')
            binary.chmod(0o755)
            shutil.copy2(source / "install.sh", bundle / "install.sh")
            environment = dict(os.environ, HOME=str(home), SHELL="/bin/zsh")
            for _ in range(2):
                subprocess.run(["bash", str(bundle / "install.sh")], env=environment, check=True, capture_output=True)
            config = (home / ".zshrc").read_text()
            self.assertEqual(config.count('export PATH="$HOME/.local/bin:$PATH"'), 1)
            shutil.rmtree(bundle)
            output = subprocess.check_output([str(home / ".local/bin/bv-sim"), "two words"], env=environment, text=True)
            self.assertEqual(output.strip(), "ok:two words")
            self.assertTrue((home / ".local/share/bv-sim/current/assets/asset.txt").is_file())


if __name__ == "__main__":
    unittest.main()
