#!/usr/bin/env python3
"""Export only the PX4 vehicle's physics/sensor SDF dependency closure."""

import argparse
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET


def export_models(source: Path, destination: Path, vehicle: str = "x500_gimbal") -> set[str]:
    pending = [vehicle]
    exported = set()
    if destination.exists() and any(destination.iterdir()):
        raise ValueError(f"output directory must be empty: {destination}")
    while pending:
        name = pending.pop(0)
        if name in exported:
            continue
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            raise ValueError(f"invalid model name: {name}")
        tree = ET.parse(source / name / "model.sdf", parser=ET.XMLParser(target=ET.TreeBuilder(insert_comments=True)))
        # Keep inertia, collision primitives, joints, plugins, and sensors
        # untouched. Only Bevy renders the aircraft and its environment.
        for parent in tree.iter():
            for child in list(parent):
                if child.tag == "visual":
                    parent.remove(child)
        if tree.findall(".//mesh"):
            raise ValueError(f"{name} requires a physical mesh; it cannot be dropped safely")
        for uri in tree.findall(".//uri"):
            if not uri.text or not uri.text.startswith("model://"):
                raise ValueError(f"unsupported external resource in {name}: {uri.text}")
            dependency = uri.text.removeprefix("model://")
            if not re.fullmatch(r"[A-Za-z0-9_-]+", dependency):
                raise ValueError(f"unexpected model resource in {name}: {uri.text}")
            pending.append(dependency)
        output = destination / name
        output.mkdir(parents=True)
        ET.indent(tree, space="  ")
        tree.write(output / "model.sdf", encoding="utf-8", xml_declaration=True)
        # Retain upstream authorship/metadata when provided by the model package.
        source_config = source / name / "model.config"
        if source_config.is_file():
            shutil.copy2(source_config, output / "model.config")
            exported.add(name)
            continue
        config = ET.Element("model")
        ET.SubElement(config, "name").text = name
        ET.SubElement(config, "version").text = "1.0"
        ET.SubElement(config, "sdf", version=tree.getroot().get("version", "1.9")).text = "model.sdf"
        ET.indent(config, space="  ")
        ET.ElementTree(config).write(output / "model.config", encoding="utf-8", xml_declaration=True)
        exported.add(name)
    for license_file in source.parent.glob("LICENSE*"):
        if license_file.is_file():
            directory = destination.parent / "licenses/PX4-gazebo-models"
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(license_file, directory / license_file.name)
    return exported


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="pinned PX4 Gazebo models directory")
    parser.add_argument("destination", type=Path, help="empty runtime models directory")
    args = parser.parse_args()
    models = export_models(args.source, args.destination)
    print("Exported physics and sensors only: " + ", ".join(sorted(models)))


if __name__ == "__main__":
    main()
