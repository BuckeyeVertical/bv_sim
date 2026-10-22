#!/usr/bin/env python3
"""Package a native executable, assets, and installer for its operating system."""

import argparse
from pathlib import Path
import re
import shutil
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
WINDOWS = {"x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"}
TARGETS = WINDOWS | {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}


def bundle(binary: Path, target: str, output: Path) -> Path:
    version = (ROOT / "VERSION").read_text().strip()
    if not re.fullmatch(r"[0-9A-Za-z._-]+", version):
        raise ValueError("invalid release VERSION")
    if not binary.is_file():
        raise ValueError(f"binary does not exist: {binary}")
    windows = target in WINDOWS
    name = f"bv-sim-{version}-{target}"
    output.mkdir(parents=True, exist_ok=True)
    archive = output.resolve() / (name + (".zip" if windows else ".tar.gz"))
    with tempfile.TemporaryDirectory(prefix="bv-sim-bundle-") as temporary:
        stage = Path(temporary) / name
        (stage / "bin").mkdir(parents=True)
        executable = stage / "bin" / ("bv-sim.exe" if windows else "bv-sim")
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        for directory in ("assets", "scenarios", "docs"):
            shutil.copytree(ROOT / directory, stage / directory)
        (stage / "protocol").mkdir()
        for filename in ("README.md", "state_v2.fbs"):
            shutil.copy2(ROOT / "protocol" / filename, stage / "protocol" / filename)
        camera = stage / "tools/camera"
        camera.mkdir(parents=True)
        for source in (ROOT / "tools/camera").iterdir():
            if source.suffix == ".py" or source.name == "README.md":
                shutil.copy2(source, camera / source.name)
        for filename in ("VERSION", "README.md"):
            shutil.copy2(ROOT / filename, stage / filename)
        installer = "install.ps1" if windows else "install.sh"
        shutil.copy2(ROOT / "packaging" / installer, stage / installer)
        if windows:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as destination:
                for source in sorted(stage.rglob("*")):
                    if source.is_file():
                        destination.write(source, source.relative_to(stage.parent).as_posix())
        else:
            (stage / installer).chmod(0o755)
            with tarfile.open(archive, "w:gz") as destination:
                destination.add(stage, arcname=name)
    return archive


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("target", choices=sorted(TARGETS))
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    print(bundle(arguments.binary, arguments.target, arguments.output))


if __name__ == "__main__":
    main()
