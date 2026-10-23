#!/usr/bin/env python3
"""Bump or verify the release version using Python 3.11+; no Cargo/network needed."""

import argparse
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
VERSION_PATTERN = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def parse_version(version: str) -> tuple[int, int, int]:
    match = VERSION_PATTERN.fullmatch(version)
    if not match:
        raise ValueError(f"Invalid version {version!r}; expected X.Y.Z, for example 0.1.2")
    return tuple(int(part) for part in match.groups())


def next_version(current: str, requested: str) -> str:
    major, minor, patch = parse_version(current)
    increments = {
        "major": (major + 1, 0, 0),
        "minor": (major, minor + 1, 0),
        "patch": (major, minor, patch + 1),
    }
    if requested in increments:
        return ".".join(map(str, increments[requested]))
    if parse_version(requested) < (major, minor, patch):
        raise ValueError(f"Cannot lower the release version from {current} to {requested}")
    return requested


def replace_version_line(text: str, version: str) -> str:
    updated, count = re.subn(
        r'(?m)^([ \t]*version[ \t]*=[ \t]*)"[^"\n]+"',
        lambda match: f'{match[1]}"{version}"', text,
    )
    if count != 1:
        raise ValueError("Expected exactly one version field in the Cargo table")
    return updated


def release_files(root: Path, version: str) -> dict[Path, str]:
    """Prepare all edits before writing; preserve external dependency locks."""
    manifest_path = root / "Cargo.toml"
    manifest_text = manifest_path.read_text()
    manifest = tomllib.loads(manifest_text)
    names = set()
    for member in manifest["workspace"]["members"]:
        package = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
        if package.get("version") != {"workspace": True}:
            raise ValueError(f"{member}/Cargo.toml must use version.workspace = true")
        names.add(package["name"])

    tables = re.split(r"(?m)(?=^\[)", manifest_text)
    workspace_tables = 0
    for index, table in enumerate(tables):
        if table.splitlines() and table.splitlines()[0].strip() == "[workspace.package]":
            tables[index] = replace_version_line(table, version)
            workspace_tables += 1
    if workspace_tables != 1:
        raise ValueError("Cargo.toml must have one [workspace.package] table")

    lock_path = root / "Cargo.lock"
    blocks = re.split(r"(?m)(?=^\[\[package\]\])", lock_path.read_text())
    found = set()
    for index, block in enumerate(blocks[1:], start=1):
        package = tomllib.loads(block)["package"][0]
        name = package["name"]
        if name in names and "source" not in package:
            if name in found:
                raise ValueError(f"Duplicate workspace package {name} in Cargo.lock")
            blocks[index] = replace_version_line(block, version)
            found.add(name)
    if found != names:
        raise ValueError(f"Cargo.lock is missing workspace packages: {', '.join(sorted(names - found))}")

    return {manifest_path: "".join(tables), lock_path: "".join(blocks)}


def check(root: Path, tag: str | None = None) -> str:
    current = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    parse_version(current)
    outdated = [
        str(path.relative_to(root))
        for path, expected in release_files(root, current).items()
        if path.read_text() != expected
    ]
    if outdated:
        raise ValueError(
            f"Release version {current} is out of sync in {', '.join(outdated)}. "
            f"Run python tools/bump_version.py {current} to synchronize the files."
        )
    if tag and tag != f"v{current}":
        raise ValueError(
            f"Tag {tag} does not match release version {current}; expected v{current}. "
            "Commit the version bump before creating and pushing its tag."
        )
    return current


def bump(root: Path, requested: str) -> tuple[str, list[Path]]:
    current = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    version = next_version(current, requested)
    updates = release_files(root, version)
    changed = []
    for path, text in updates.items():
        if path.read_text() != text:
            path.write_text(text)
            changed.append(path.relative_to(root))
    check(root)
    return version, changed


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="patch, minor, major, or an explicit X.Y.Z")
    parser.add_argument("--check", action="store_true", help="verify versions; print the release version")
    parser.add_argument("--tag", help="with --check, also verify a release tag (empty means no tag)")
    args = parser.parse_args()
    if args.check and args.version:
        parser.error("--check does not accept a version to bump")
    if not args.check and (not args.version or args.tag is not None):
        parser.error("provide patch, minor, major, or X.Y.Z; --tag requires --check")
    try:
        if args.check:
            print(check(ROOT, args.tag))
        else:
            version, changed = bump(ROOT, args.version)
            print(f"Release version: {version}")
            for path in changed:
                print(f"  Updated {path}")
            print(f"Commit the changes, then run: git tag v{version}")
            print(f"Push the commit and tag: git push origin HEAD v{version}")
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
