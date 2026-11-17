#!/usr/bin/env python3
"""Color the logo's negative-space M/U/S glyphs white in generated icon assets.

The supplied vector logo draws its letters as transparent cutouts in the colored
faces.  The generated rounded-square background shows through those cutouts.
This pass recolors only the cutouts white, keeping the slightly darker light-gray tile and the
rest of the artwork intact.

Requires Pillow: python3 -m pip install Pillow
"""

from __future__ import annotations

import io
import struct
from pathlib import Path

from PIL import Image, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "src-tauri" / "icons"
MASTER = ICON_DIR / "icon.png"

# Bounding boxes mapped from the supplied 512 x 512 app icon.  Each box is over
# a colored face, so near-gray pixels inside it belong to the corresponding
# negative-space letter rather than to the rounded tile.
# Boîtes de délimitation reportées depuis l’icône fournie en 512 × 512. Chaque
# boîte couvre une face colorée : les pixels presque gris sont donc les découpes
# des lettres, et non le fond carré arrondi.
GLYPH_BOXES_512 = (
    (90, 100, 220, 235),  # M
    (300, 100, 415, 235),  # U
    (195, 275, 315, 415),  # S
)


def colorize_letters(image: Image.Image) -> Image.Image:
    image = image.convert("RGBA")
    width, height = image.size
    if width != height or width < 16:
        return

    scale_x = width / 512
    scale_y = height / 512
    pixels = image.load()
    for x0, y0, x1, y1 in GLYPH_BOXES_512:
        box = (
            max(0, round(x0 * scale_x)),
            max(0, round(y0 * scale_y)),
            min(width, round(x1 * scale_x)),
            min(height, round(y1 * scale_y)),
        )
        crop = image.crop(box)
        white_core = Image.new("L", crop.size, 0)
        core_pixels = crop.load()
        mask_pixels = white_core.load()
        for y in range(crop.height):
            for x in range(crop.width):
                red, green, blue, alpha = core_pixels[x, y]
                if alpha > 240 and min(red, green, blue) >= 240:
                    mask_pixels[x, y] = 255

        # Extend one raster pixel beyond the solid tile-colored core to cover
        # the antialiased edge without leaving a gray halo around the glyph.
        # Étendre d’un pixel raster au-delà du cœur uni du carré pour couvrir
        # l’anticrénelage sans laisser de halo gris autour de la lettre.
        mask = white_core.filter(ImageFilter.MaxFilter(3)) if width >= 64 else white_core
        mask_data = mask.load()
        for y in range(crop.height):
            for x in range(crop.width):
                if mask_data[x, y]:
                    red, green, blue, alpha = core_pixels[x, y]
                    pixels[box[0] + x, box[1] + y] = (255, 255, 255, alpha)

    return image


def colorize_icns(path: Path) -> None:
    data = path.read_bytes()
    if data[:4] != b"icns":
        raise ValueError(f"Not an ICNS file: {path}")
    chunks: list[bytes] = []
    offset = 8
    while offset < len(data):
        if offset + 8 > len(data):
            raise ValueError(f"Truncated ICNS chunk header: {path}")
        chunk_type = data[offset : offset + 4]
        chunk_length = struct.unpack(">I", data[offset + 4 : offset + 8])[0]
        if chunk_length < 8 or offset + chunk_length > len(data):
            raise ValueError(f"Invalid ICNS chunk length: {path}")
        payload = data[offset + 8 : offset + chunk_length]
        if payload.startswith(b"\x89PNG\r\n\x1a\n"):
            with Image.open(io.BytesIO(payload)) as source:
                output = io.BytesIO()
                colorize_letters(source.copy()).save(output, format="PNG", optimize=True)
                payload = output.getvalue()
        chunks.append(chunk_type + struct.pack(">I", len(payload) + 8) + payload)
        offset += chunk_length
    body = b"".join(chunks)
    path.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def rebuild_ico() -> None:
    with Image.open(MASTER) as source:
        master = source.convert("RGBA")

    master.save(
        ICON_DIR / "icon.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

def main() -> None:
    pngs = sorted(ICON_DIR.rglob("*.png"))
    for path in pngs:
        with Image.open(path) as source:
            output = colorize_letters(source.copy())
        output.save(path, format="PNG", optimize=True)
    colorize_icns(ICON_DIR / "icon.icns")
    rebuild_ico()
    background_xml = ICON_DIR / "android" / "values" / "ic_launcher_background.xml"
    if background_xml.exists():
        text = background_xml.read_text()
        background_xml.write_text(text.replace("#FFFFFF", "#E0E2E5").replace("#ffffff", "#E0E2E5"))
    print(f"Colorized M/U/S in {len(pngs)} PNG assets and embedded macOS icon sizes.")


if __name__ == "__main__":
    main()
