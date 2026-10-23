"""Exercise the download installer with local archives and simulated GitHub replies."""

import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest


@unittest.skipIf(os.name == "nt", "The shell installer runs on macOS/Linux")
class DownloadInstallTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="bv-sim-download-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = Path(__file__).resolve().parents[1]
        self.shims = self.root / "shims"
        self.shims.mkdir()
        self.home = self.root / "a user's home"
        self.home.mkdir()
        self.log = self.root / "requests.log"
        self.environment = dict(
            os.environ, HOME=str(self.home), TMPDIR=str(self.root), SHELL="/bin/bash",
            PATH=str(self.shims) + os.pathsep + os.environ["PATH"],
            BV_TEST_SYSTEM="Linux", BV_TEST_ARCH="x86_64", BV_TEST_VERSION="0.1.1",
            BV_TEST_REQUESTS=str(self.log),
        )
        (self.shims / "uname").write_text(
            '#!/bin/sh\ncase "$1" in -s) echo "$BV_TEST_SYSTEM";; -m) echo "$BV_TEST_ARCH";; esac\n'
        )
        (self.shims / "curl").write_text('''#!/usr/bin/env python3
import os
from pathlib import Path
import shutil
import sys
args = sys.argv[1:]
urls = [arg for arg in args if arg.startswith("https://")]
assert len(urls) == 1
url = urls[0]
with Path(os.environ["BV_TEST_REQUESTS"]).open("a") as log:
    log.write(url + "\\n")
base = "https://github.com/BuckeyeVertical/bv_sim"
version = os.environ["BV_TEST_VERSION"]
if "--write-out" in args:
    assert url == base + "/releases/latest"
    print(base + "/releases/tag/v" + version, end="")
else:
    target = os.environ["BV_TEST_TARGET"]
    assert url == f"{base}/releases/download/v{version}/bv-sim-{version}-{target}.tar.gz"
    if os.environ.get("BV_TEST_FAIL_DOWNLOAD"):
        sys.exit(22)
    shutil.copy2(os.environ["BV_TEST_ARCHIVE"], args[args.index("--output") + 1])
''')
        for shim in self.shims.iterdir():
            shim.chmod(0o755)

    def archive(self, target, version="0.1.1"):
        bundle = self.root / f"bv-sim-{version}-{target}"
        (bundle / "bin").mkdir(parents=True)
        (bundle / "assets").mkdir()
        (bundle / "scenarios").mkdir()
        (bundle / "assets/asset.txt").write_text("asset")
        (bundle / "scenarios/suas_2026.toml").write_text("scenario")
        (bundle / "bin/bv-sim").write_text(f"#!/bin/sh\necho bv-sim {version}\n")
        (bundle / "bin/bv-sim").chmod(0o755)
        shutil.copy2(self.source / "packaging/install.sh", bundle / "install.sh")
        archive = self.root / f"release-{version}.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            stream.add(bundle, arcname=bundle.name)
        shutil.rmtree(bundle)
        self.environment.update(BV_TEST_ARCHIVE=str(archive), BV_TEST_TARGET=target, BV_TEST_VERSION=version)

    def run_installer(self):
        return subprocess.run(
            ["bash", str(self.source / "install.sh")], env=self.environment,
            cwd=self.root, capture_output=True, text=True,
        )

    def test_detects_platform_and_installs_complete_bundle(self):
        for system, architecture, target in (
            ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
            ("Darwin", "arm64", "aarch64-apple-darwin"),
        ):
            with self.subTest(system=system):
                self.environment.update(BV_TEST_SYSTEM=system, BV_TEST_ARCH=architecture)
                self.archive(target)
                result = self.run_installer()
                self.assertEqual(result.returncode, 0, result.stderr)
                installed = self.home / ".local/share/bv-sim/current"
                self.assertFalse((installed / "VERSION").exists())
                self.assertEqual((installed / "assets/asset.txt").read_text(), "asset")
                binary = self.home / ".local/bin/bv-sim"
                self.assertEqual(subprocess.check_output([str(binary)], text=True).strip(), "bv-sim 0.1.1")
                self.assertEqual(list(self.root.glob("bv-sim-download.*")), [])

    def test_repeating_command_upgrades_to_latest_release(self):
        for version in ("0.1.1", "0.1.2"):
            self.archive("x86_64-unknown-linux-gnu", version)
            result = self.run_installer()
            self.assertEqual(result.returncode, 0, result.stderr)
        binary = self.home / ".local/bin/bv-sim"
        self.assertEqual(subprocess.check_output([str(binary), "--version"], text=True).strip(), "bv-sim 0.1.2")
        self.assertEqual((self.home / ".bashrc").read_text().count('export PATH="$HOME/.local/bin:$PATH"'), 1)

    def test_failed_download_cleans_up_without_installing(self):
        self.archive("x86_64-unknown-linux-gnu")
        self.environment["BV_TEST_FAIL_DOWNLOAD"] = "1"
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.home / ".local").exists())
        self.assertEqual(list(self.root.glob("bv-sim-download.*")), [])

    def test_unsupported_architecture_stops_before_downloading(self):
        self.environment.update(BV_TEST_SYSTEM="Darwin", BV_TEST_ARCH="x86_64")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Apple Silicon", result.stderr)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
