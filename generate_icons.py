from PIL import Image, ImageDraw, ImageFont, ImageOps
import os

# Keep in lockstep with `ICON_CELL` in `crates/soul-runner/src/main.rs`.
ICON_DRAW_SIZE = (68, 68)
ICON_EXPORT_SIZE = (32, 32)


def draw_app_icon(app_name: str, output_path: str, size=ICON_DRAW_SIZE) -> None:
    """
    Draw a simple pictorial icon (not a text label) for each app.
    Black strokes on white; SoulOS inverts this PGM for the pressed state.
    """
    w, h = size
    img = Image.new("L", size, color=255)
    d = ImageDraw.Draw(img)
    m = 10
    box = (m, m, w - m, h - m)

    def line(xy, width=2, fill=0):
        d.line(xy, fill=fill, width=width)

    def rect(r, outline=0, width=2):
        d.rectangle(r, outline=outline, width=width)

    def ellipse(r, outline=0, width=2):
        d.ellipse(r, outline=outline, width=width)

    if app_name == "Notes":
        # Ruled note: vertical margin + horizontal lines
        line([(m + 8, m + 4), (m + 8, h - m - 4)], width=2, fill=64)
        for i in range(5):
            y = m + 10 + i * 9
            line([(m + 14, y), (w - m - 6, y)], width=1)

    elif app_name == "Address":
        # Contact silhouette: head + shoulders
        cx, cy = w // 2, h // 2 - 4
        ellipse((cx - 10, cy - 14, cx + 10, cy + 2), outline=0, width=2)
        d.arc((cx - 22, cy - 2, cx + 22, cy + 28), start=200, end=340, fill=0, width=2)

    elif app_name == "Date":
        # Calendar sheet with header bar and grid
        rect((m + 4, m + 6, w - m - 4, h - m - 6), outline=0, width=2)
        rect((m + 4, m + 6, w - m - 4, m + 18), outline=0, width=2)
        line([(m + 18, m + 22), (w - m - 18, m + 22)], width=1)
        line([(m + 18, m + 34), (w - m - 18, m + 34)], width=1)
        for x in (m + 22, w // 2, w - m - 22):
            line([(x, m + 38), (x, h - m - 12)], width=1)

    elif app_name == "ToDo":
        # Checkbox list: three rows with boxes
        for row in range(3):
            y = m + 8 + row * 16
            rect((m + 6, y, m + 18, y + 12), outline=0, width=2)
            line([(m + 22, y + 6), (w - m - 8, y + 6)], width=1)
        # Check middle box
        line([(m + 8, m + 22), (m + 12, m + 26), (m + 16, m + 18)], width=2)

    elif app_name == "Mail":
        # Envelope
        rect((m + 4, m + 14, w - m - 4, h - m - 8), outline=0, width=2)
        line([(m + 4, m + 14), (w // 2, m + 28), (w - m - 4, m + 14)], width=2)

    elif app_name == "Calc":
        # Display + keypad grid
        rect((m + 6, m + 6, w - m - 6, m + 18), outline=0, width=2)
        gw, gh = 3, 3
        cw = (box[2] - box[0] - 8) // gw
        ch = (box[3] - box[1] - 28) // gh
        ox, oy = m + 6, m + 24
        for gy in range(gh):
            for gx in range(gw):
                x0 = ox + gx * cw
                y0 = oy + gy * ch
                rect((x0 + 2, y0 + 2, x0 + cw - 2, y0 + ch - 2), outline=0, width=1)

    elif app_name == "Prefs":
        # Three sliders
        for i in range(3):
            y = m + 12 + i * 16
            line([(m + 8, y), (w - m - 8, y)], width=2)
            cx = m + 18 + (i * 19) % (w - 2 * m - 28)
            ellipse((cx - 5, y - 5, cx + 5, y + 5), outline=0, width=2)

    elif app_name == "Draw":
        # Canvas with a stroke
        rect((m + 4, m + 4, w - m - 4, h - m - 4), outline=0, width=2)
        d.arc((m + 12, m + 20, w - m - 12, h - m - 8), start=200, end=320, fill=0, width=3)

    elif app_name == "Sync":
        # Two arc arrows (refresh)
        cx, cy = w // 2, h // 2
        r = 18
        d.arc((cx - r, cy - r, cx + r, cy + r), start=30, end=200, fill=0, width=3)
        d.arc((cx - r, cy - r, cx + r, cy + r), start=210, end=20, fill=0, width=3)
        # arrow heads (small triangles as lines)
        ax, ay = cx + int(r * 0.7), cy - int(r * 0.5)
        line([(ax, ay), (ax - 6, ay - 4), (ax - 2, ay + 6), (ax, ay)], width=2)
        bx, by = cx - int(r * 0.7), cy + int(r * 0.5)
        line([(bx, by), (bx + 6, by + 4), (bx + 2, by - 6), (bx, by)], width=2)

    else:
        # Fallback: first letter only (should not happen if lists match)
        try:
            font = ImageFont.truetype("arial.ttf", 20)
        except OSError:
            font = ImageFont.load_default()
        ch = app_name[0]
        bbox = d.textbbox((0, 0), ch, font=font)
        tw = bbox[2] - bbox[0]
        th = bbox[3] - bbox[1]
        d.text(((w - tw) / 2, (h - th) / 2), ch, fill=0, font=font)

    try:
        resample = Image.Resampling.LANCZOS
    except AttributeError:
        resample = Image.LANCZOS
    img = img.resize(ICON_EXPORT_SIZE, resample)
    img.save(output_path, "PPM", PGM=True)


def combine_icons_into_spritesheet(icon_paths, output_path, icon_size=ICON_EXPORT_SIZE):
    if not icon_paths:
        return

    icon_width, icon_height = icon_size
    total_width = len(icon_paths) * icon_width

    spritesheet = Image.new("L", (total_width, icon_height), color=255)

    x_offset = 0
    for icon_path in icon_paths:
        icon = Image.open(icon_path)
        spritesheet.paste(icon, (x_offset, 0))
        x_offset += icon_width

    spritesheet.save(output_path, "PPM", PGM=True)
    print(f"Generated sprite sheet: {output_path}")


def invert_pgm(input_path, output_path):
    img = Image.open(input_path)
    inverted_img = ImageOps.invert(img)
    inverted_img.save(output_path, "PPM", PGM=True)
    print(f"Generated inverted PGM: {output_path}")


def generate_app_icons(output_dir):
    # Keep in lockstep with `APPS` in `crates/soul-runner/src/main.rs`.
    apps = [
        "Notes",
        "Address",
        "Date",
        "ToDo",
        "Mail",
        "Calc",
        "Prefs",
        "Draw",
        "Sync",
    ]
    if not os.path.exists(output_dir):
        os.makedirs(output_dir)

    icon_paths = []
    for app_name in apps:
        filename = os.path.join(output_dir, f"{app_name.lower()}_icon.pgm")
        draw_app_icon(app_name, filename)
        print(f"Generated {filename}")
        icon_paths.append(filename)

    spritesheet_path = os.path.join(output_dir, "app_icons_spritesheet.pgm")
    combine_icons_into_spritesheet(icon_paths, spritesheet_path)

    inverted_spritesheet_path = os.path.join(output_dir, "app_icons_spritesheet_inverted.pgm")
    invert_pgm(spritesheet_path, inverted_spritesheet_path)


if __name__ == "__main__":
    script_dir = os.path.dirname(__file__)
    assets_sprites_dir = os.path.join(script_dir, "assets", "sprites")
    generate_app_icons(assets_sprites_dir)
