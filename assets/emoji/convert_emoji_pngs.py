"""
Convert all 24x24 PNG emoji in png/ to raw 8bpp indexed format for SoulOS.
- Input:  png/U+XXXX.png  (24x24, 8-color indexed, see soulos-grayscale.gpl)
- Output: raw/U+XXXX.raw  (24x24 bytes, each byte is palette index 0-7)
- Also regenerates meta/emoji_index.json (codepoint → filename mapping)

Usage (from repo root): make emoji-assets
"""
import os
import json
from pathlib import Path
from PIL import Image

PALETTE_SIZE = 8
SRC_W = 24
SRC_H = 24

SCRIPT_DIR = Path(__file__).parent
PNG_DIR = SCRIPT_DIR / "png"
RAW_DIR = SCRIPT_DIR / "raw"
META_DIR = SCRIPT_DIR / "meta"


def convert_png_to_raw(png_path: Path, raw_path: Path) -> None:
    img = Image.open(png_path)
    if img.size != (SRC_W, SRC_H):
        raise ValueError(f"{png_path}: must be {SRC_W}x{SRC_H} pixels, got {img.size}")
    if img.mode != "P":
        raise ValueError(f"{png_path}: must be indexed color (mode 'P'), got {img.mode}")
    data = img.tobytes()
    if any(b >= PALETTE_SIZE for b in data):
        raise ValueError(f"{png_path}: pixel index out of 0..{PALETTE_SIZE-1} range")
    raw_path.write_bytes(data)


def main() -> None:
    RAW_DIR.mkdir(exist_ok=True)
    META_DIR.mkdir(exist_ok=True)

    index = {}
    for png_path in sorted(PNG_DIR.glob("U+*.png")):
        stem = png_path.stem          # e.g. "U+1F642"
        codepoint = stem[2:]          # e.g. "1F642"
        raw_path = RAW_DIR / f"{stem}.raw"
        convert_png_to_raw(png_path, raw_path)
        index[codepoint] = raw_path.name
        print(f"  {png_path.name} → {raw_path.name}")

    (META_DIR / "emoji_index.json").write_text(
        json.dumps(index, indent=2) + "\n"
    )
    print(f"Done. {len(index)} emoji converted.")


if __name__ == "__main__":
    main()
