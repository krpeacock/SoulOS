from PIL import Image, ImageDraw, ImageFont, ImageOps
import os

# Keep in lockstep with `ICON_CELL` in `crates/soul-runner/src/main.rs`.
ICON_EXPORT_SIZE = (32, 32)

EMOJI_FONT_PATH = "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"
# 109 is the only pixel size NotoColorEmoji exposes via FreeType on this system.
EMOJI_FONT_SIZE = 109

# (file_stem, emoji)  →  assets/sprites/{stem}_icon.pgm
APPS = [
    ("notes",     "📝"),
    ("address",   "👤"),
    ("date",      "📅"),
    ("todo",      "✅"),
    ("mail",      "✉️"),
    ("calc",      "🧮"),
    ("prefs",     "⚙️"),
    ("draw",      "✏️"),
    ("sync",      "🔄"),
    ("builder",   "🔨"),
    ("paint",     "🎨"),
    ("egui_demo", "🧪"),
    ("launcher2", "🏠"),
]


def render_emoji_icon(emoji: str, size=ICON_EXPORT_SIZE) -> Image.Image:
    font = ImageFont.truetype(EMOJI_FONT_PATH, EMOJI_FONT_SIZE)
    # NotoColorEmoji glyphs are 136×128 at size 109; use a square canvas with margin.
    canvas = 140
    img = Image.new("RGBA", (canvas, canvas), color=(255, 255, 255, 255))
    d = ImageDraw.Draw(img)
    bb = d.textbbox((0, 0), emoji, font=font, embedded_color=True)
    gw, gh = bb[2] - bb[0], bb[3] - bb[1]
    x = (canvas - gw) // 2 - bb[0]
    y = (canvas - gh) // 2 - bb[1]
    d.text((x, y), emoji, font=font, embedded_color=True)
    gray = img.convert("L")
    gray = ImageOps.autocontrast(gray)
    return gray.resize(size, Image.Resampling.LANCZOS)


def combine_icons_into_spritesheet(icon_paths, output_path, icon_size=ICON_EXPORT_SIZE):
    if not icon_paths:
        return
    icon_width, icon_height = icon_size
    spritesheet = Image.new("L", (len(icon_paths) * icon_width, icon_height), color=255)
    for i, icon_path in enumerate(icon_paths):
        icon = Image.open(icon_path)
        spritesheet.paste(icon, (i * icon_width, 0))
    spritesheet.save(output_path, "PPM", PGM=True)
    print(f"Generated sprite sheet: {output_path}")


def invert_pgm(input_path, output_path):
    img = ImageOps.invert(Image.open(input_path))
    img.save(output_path, "PPM", PGM=True)
    print(f"Generated inverted PGM: {output_path}")


def generate_app_icons(output_dir):
    os.makedirs(output_dir, exist_ok=True)
    icon_paths = []
    for stem, emoji in APPS:
        path = os.path.join(output_dir, f"{stem}_icon.pgm")
        render_emoji_icon(emoji).save(path, "PPM", PGM=True)
        print(f"Generated {path}")
        icon_paths.append(path)

    spritesheet_path = os.path.join(output_dir, "app_icons_spritesheet.pgm")
    combine_icons_into_spritesheet(icon_paths, spritesheet_path)

    inverted_path = os.path.join(output_dir, "app_icons_spritesheet_inverted.pgm")
    invert_pgm(spritesheet_path, inverted_path)


if __name__ == "__main__":
    script_dir = os.path.dirname(__file__)
    generate_app_icons(os.path.join(script_dir, "assets", "sprites"))
