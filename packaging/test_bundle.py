"""Verify release archives derive their version from Cargo.toml."""

import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import bundle


class BundleTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="bv-sim-bundle-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for directory in ("assets", "scenarios", "docs", "protocol", "tools/camera", "packaging"):
            (self.root / directory).mkdir(parents=True)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "9.8.7"\n')
        for filename in ("README.md", "assets/asset.txt", "scenarios/suas_2026.toml", "protocol/README.md", "protocol/state_v2.fbs"):
            (self.root / filename).write_text("bundle fixture\n")
        for filename in ("install.sh", "install.ps1"):
            shutil.copy2(Path(__file__).with_name(filename), self.root / "packaging" / filename)
        self.binary = self.root / "bv-sim"
        self.binary.write_text('#!/bin/sh\necho "bv-sim 9.8.7"\n')
        self.binary.chmod(0o755)

    def test_all_platform_archives_use_workspace_version_without_version_file(self):
        for target in ("aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"):
            with self.subTest(target=target), patch.object(bundle, "ROOT", self.root):
                archive = bundle.bundle(self.binary, target, self.root / "dist")
                name = f"bv-sim-9.8.7-{target}"
                if archive.suffix == ".zip":
                    with zipfile.ZipFile(archive) as stream:
                        files = stream.namelist()
                    installer, executable = "install.ps1", "bv-sim.exe"
                else:
                    with tarfile.open(archive) as stream:
                        files = stream.getnames()
                    installer, executable = "install.sh", "bv-sim"
                self.assertTrue(archive.name.startswith(name))
                for filename in (installer, f"bin/{executable}", "assets/asset.txt", "scenarios/suas_2026.toml"):
                    self.assertIn(f"{name}/{filename}", files)
                self.assertFalse(any(Path(filename).name == "VERSION" for filename in files))

    @unittest.skipIf(os.name == "nt", "The shell installer runs on macOS/Linux")
    def test_packaged_executable_installs_without_source_manifest_or_version_file(self):
        with patch.object(bundle, "ROOT", self.root):
            archive = bundle.bundle(self.binary, "x86_64-unknown-linux-gnu", self.root / "dist")
        extracted = self.root / "extracted"
        with tarfile.open(archive) as stream:
            stream.extractall(extracted, filter="data")
        home = self.root / "home"
        home.mkdir()
        release = extracted / "bv-sim-9.8.7-x86_64-unknown-linux-gnu"
        environment = dict(os.environ, HOME=str(home), SHELL="/bin/bash")
        subprocess.run(["bash", str(release / "install.sh"), "--no-path"], env=environment, check=True, capture_output=True)
        shutil.rmtree(extracted)
        installed = home / ".local/bin/bv-sim"
        self.assertEqual(subprocess.check_output([str(installed), "--version"], text=True).strip(), "bv-sim 9.8.7")
        self.assertFalse((home / ".local/share/bv-sim/current/VERSION").exists())


if __name__ == "__main__":
    unittest.main()
