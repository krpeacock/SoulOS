# SoulOS Emoji Assets

Emoji are rendered as 24×24 grayscale bitmaps. Two sources are supported:

- **Monochrome glyphs** — hand-drawn bit patterns in `soul-ui/src/emoji.rs`, used for classic Unicode symbols (⌂ ☀ ♥ ✓ etc.).
- **Raw bitmap assets** — 24×24 PNG files authored in GIMP and compiled to `.raw` at build time. These appear in `assets/emoji/png/` and live in the emoji keyboard picker alongside the monochrome glyphs.

## Adding a new emoji

1. Draw a 24×24 PNG using the SoulOS grayscale palette (see *Palette setup* below).
2. Save it as `assets/emoji/png/U+XXXX.png`, where `XXXX` is the Unicode codepoint in uppercase hex (e.g. `U+1F600.png` for 😀).
3. Run `make emoji-assets` — this converts the PNG to `assets/emoji/raw/U+XXXX.raw` and updates `meta/emoji_index.json`.
4. Run `cargo build` — `build.rs` picks up the new `.raw` file, embeds it with `include_bytes!`, and adds the emoji to the keyboard picker automatically. No edits to Rust source required.

## Palette setup (GIMP)

- Copy `soulos-grayscale.gpl` to your GIMP palettes folder.
- Open GIMP → Image → Mode → Indexed → select **SoulOS Grayscale** palette (8 colors).
- Draw the glyph. Background pixels should use the lightest palette entry (index 7 = white).
- Export as PNG, keeping indexed mode.

## File layout

```
assets/emoji/
├── png/              # source PNGs (checked in, one per emoji)
│   └── U+1F642.png
├── raw/              # compiled .raw files (generated, gitignored)
│   └── U+1F642.raw
├── meta/
│   ├── emoji_list.json   # full Unicode emoji list (name lookup for picker)
│   └── emoji_index.json  # generated index of compiled assets
├── convert_emoji_pngs.py # build script invoked by `make emoji-assets`
└── soulos-grayscale.gpl  # GIMP palette
```

## Raw format

Each `.raw` file is exactly 576 bytes: 24 rows × 24 columns, one byte per pixel. Each byte is a palette index in the range 0–7, where 0 = black and 7 = white. The runtime downsamples to the target cell size using integer box-filter averaging.

## Build integration

`crates/soul-ui/build.rs` runs automatically on every `cargo build`:

- Scans `assets/emoji/raw/` for `U+XXXX.raw` files.
- Embeds each as a `&[u8; 576]` static via `include_bytes!`.
- Looks up the emoji name in `meta/emoji_list.json`.
- Generates two files in `$OUT_DIR`:
  - `raw_emoji.rs` — the embedded bitmap data (`RAW_EMOJI` static).
  - `emoji_list.rs` — the picker list (`EMOJI_LIST` static), combining monochrome glyphs and raw assets.

Both files are included directly in `soul-ui/src/emoji.rs` with `include!`.

## License

All emoji assets must be original artwork or CC0/public domain.
