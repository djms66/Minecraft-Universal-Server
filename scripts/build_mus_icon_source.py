#!/usr/bin/env python3
"""Build a platform-safe square icon from the supplied M.U.S. SVG master.

Construit une icône carrée multiplateforme à partir du SVG maître M.U.S. fourni.
The source artwork remains untouched; this wrapper adds the approved neutral tile
and replaces font-dependent white letter fills with the SVG's own inner contours.
Le dessin source reste intact ; ce cadre ajoute le fond neutre approuvé et remplace
les lettres dépendantes d'une police par leurs contours vectoriels internes.
"""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
NS = "http://www.w3.org/2000/svg"
ET.register_namespace("", NS)


def materialize_letter_fill(path: ET.Element) -> ET.Element:
    """Return the SVG path for the inner contour of a compound outlined glyph."""
    data = path.get("d", "")
    first = re.search(r"M\s*(-?[\d.]+)[, ]+(-?[\d.]+)", data)
    inner = re.search(r"z\s*([mM])\s*(-?[\d.]+)[, ]+(-?[\d.]+)", data, re.IGNORECASE)
    if not first or not inner:
        raise ValueError("Expected an outer and an inner glyph contour")

    origin_x, origin_y = map(float, first.groups())
    command, inner_x, inner_y = inner.groups()
    inner_x, inner_y = float(inner_x), float(inner_y)
    if command == "m":
        inner_x += origin_x
        inner_y += origin_y
    inner_start = inner.start(1)
    remainder = data[inner_start:].strip()
    remainder = re.sub(r"^[mM]\s*-?[\d.]+[, ]+-?[\d.]+", "", remainder, count=1)
    fill = ET.Element(f"{{{NS}}}path", {
        "d": f"M{inner_x:g},{inner_y:g}{remainder}",
        "style": "fill:#fff;fill-rule:nonzero;",
    })
    return fill


def main() -> None:
    master = ROOT / "src-tauri" / "icon-source" / "logo_MUS.svg"
    target = ROOT / "src-tauri" / "icon-source" / "logo_MUS_icon.svg"
    artwork = ET.parse(master).getroot()
    icon = ET.Element(f"{{{NS}}}svg", {
        "width": "512", "height": "512", "viewBox": "0 0 512 512", "version": "1.1",
    })
    ET.SubElement(icon, f"{{{NS}}}rect", {
        "x": "0", "y": "0", "width": "512", "height": "512", "rx": "112", "ry": "112",
        "style": "fill:#D3D6DA;",
    })
    placed = ET.SubElement(icon, f"{{{NS}}}svg", {
        "x": "27", "y": "24", "width": "458", "height": "464",
        "viewBox": "0 0 1146 1159", "preserveAspectRatio": "xMidYMid meet",
    })
    for child in artwork:
        placed.append(deepcopy(child))

    # The supplied SVG draws an outlined letter contour, then overlays live text.
    # Add a white vector fill beneath that outline and remove live text so icon
    # generators render the mark consistently without installing D-DIN Exp.
    # Le SVG fourni trace le contour des lettres puis superpose du texte. Ajoute
    # un remplissage blanc vectoriel et enlève le texte pour éviter toute police.
    for group in placed.iter(f"{{{NS}}}g"):
        if not group.get("transform"):
            continue
        outline = group.find(f"{{{NS}}}path")
        label = group.find(f"{{{NS}}}text")
        if outline is None or label is None:
            continue
        group.insert(0, materialize_letter_fill(outline))
        group.remove(label)

    ET.ElementTree(icon).write(target, encoding="UTF-8", xml_declaration=True)
    print(f"Built {target} from {master}")


if __name__ == "__main__":
    main()
