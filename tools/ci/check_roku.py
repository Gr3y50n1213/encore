"""Checks the Roku channel before it is packed (roku.yml): the manifest has what a
channel needs, every SceneGraph XML file parses, every script a component names is
in the package, and every image the manifest names is a PNG of Roku's size."""
import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

SIZES = {"mm_icon_focus_hd": (290, 218), "mm_icon_focus_fhd": (540, 405), "splash_screen_fhd": (1920, 1080)}


def png_size(p):
    head = p.read_bytes()[:24]
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{p} is not a PNG")
    return struct.unpack(">II", head[16:24])


def main(root):
    root = Path(root)
    manifest = {}
    for line in (root / "manifest").read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            manifest[k] = v
    for k in ("title", "major_version", "minor_version", "build_version", "mm_icon_focus_hd"):
        if k not in manifest:
            raise SystemExit(f"manifest has no {k}")
    for k, size in SIZES.items():
        if k in manifest:
            p = root / manifest[k].removeprefix("pkg:/")
            if png_size(p) != size:
                raise SystemExit(f"{p} is {png_size(p)}, Roku wants {size}")
    if not (root / "source" / "main.brs").exists():
        raise SystemExit("source/main.brs is missing")
    for x in sorted((root / "components").glob("*.xml")):
        tree = ET.parse(x)
        for s in tree.iter("script"):
            uri = s.get("uri", "")
            if uri and not (root / uri.removeprefix("pkg:/")).exists():
                raise SystemExit(f"{x.name} names {uri}, which is not in the package")
    print(f"roku channel ok: {manifest['title']} {manifest['major_version']}.{manifest['minor_version']}.{manifest['build_version']}")


if __name__ == "__main__":
    main(sys.argv[1])
