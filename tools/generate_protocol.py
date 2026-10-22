#!/usr/bin/env python3
"""Generate checked-in C++/Rust bindings using the pinned FlatBuffers compiler."""

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
VERSION = "25.2.10"
SHA256 = "b9c2df49707c57a48fc0923d52b8c73beb72d675f9d44b2211e4569be40a7421"
URL = f"https://codeload.github.com/google/flatbuffers/tar.gz/refs/tags/v{VERSION}"


def run(*args: object) -> None:
    subprocess.run([str(arg) for arg in args], check=True)


def compiler() -> Path:
    tools = ROOT / ".tools"
    source = tools / f"flatbuffers-{VERSION}"
    build = tools / f"flatbuffers-{VERSION}-build"
    executables = [build / "flatc", build / "flatc.exe", build / "Release/flatc.exe"]
    for executable in executables:
        if executable.is_file():
            return executable
    tools.mkdir(exist_ok=True)
    archive = tools / f"flatbuffers-{VERSION}.tar.gz"
    if not archive.is_file() or hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        request = urllib.request.Request(URL, headers={"User-Agent": "bv-sim-protocol-generator"})
        with urllib.request.urlopen(request, timeout=120) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != SHA256:
            raise SystemExit("FlatBuffers archive SHA-256 does not match the pinned source")
        archive.write_bytes(data)
    if not source.is_dir():
        with tarfile.open(archive) as stream:
            # Every member must stay within the checksum-verified source tree.
            for member in stream.getmembers():
                if not (tools / member.name).resolve().is_relative_to(source):
                    raise SystemExit(f"Unsafe archive member: {member.name}")
            if hasattr(tarfile, "data_filter"):
                stream.extractall(tools, filter="data")
            else:
                stream.extractall(tools)
    run("cmake", "-S", source, "-B", build, "-DCMAKE_BUILD_TYPE=Release",
        "-DFLATBUFFERS_BUILD_TESTS=OFF", "-DFLATBUFFERS_BUILD_FLATLIB=OFF",
        "-DFLATBUFFERS_INSTALL=OFF")
    run("cmake", "--build", build, "--config", "Release", "--target", "flatc", "--parallel", min(os.cpu_count() or 2, 4))
    for executable in executables:
        if executable.is_file():
            return executable
    raise SystemExit("CMake did not produce the FlatBuffers compiler")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--flatc", type=Path, help=f"existing flatc {VERSION} executable")
    parser.add_argument("--check", action="store_true", help="fail if checked-in bindings differ")
    args = parser.parse_args()
    flatc = args.flatc.resolve() if args.flatc else compiler()
    actual = subprocess.check_output([str(flatc), "--version"], text=True).strip()
    if actual != f"flatc version {VERSION}":
        raise SystemExit(f"Expected flatc version {VERSION}, got {actual!r}")
    with tempfile.TemporaryDirectory(prefix="bv-sim-protocol-") as directory:
        output = Path(directory) if args.check else ROOT / "protocol/generated"
        for language, flags, filename in [
            ("cpp", ["--cpp", "--scoped-enums"], "state_v2_generated.h"),
            ("rust", ["--rust"], "state_v2_generated.rs"),
        ]:
            destination = output / language
            destination.mkdir(parents=True, exist_ok=True)
            run(flatc, *flags, "-o", destination, ROOT / "protocol/state_v2.fbs")
            generated = destination / filename
            # flatc writes in text mode, which produces CRLF on Windows. Keep
            # checked-in bindings reproducible on every host.
            generated.write_bytes(generated.read_bytes().replace(b"\r\n", b"\n"))
            if args.check:
                checked_in = ROOT / "protocol/generated" / language / filename
                if not checked_in.is_file() or checked_in.read_bytes() != generated.read_bytes():
                    raise SystemExit(f"Outdated bindings: {checked_in.relative_to(ROOT)}; regenerate them")
    print("Protocol bindings match the schema" if args.check else "Generated C++ and Rust protocol bindings")


if __name__ == "__main__":
    main()
