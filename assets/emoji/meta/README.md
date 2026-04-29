# Emoji Asset Folders

- `png/` — Source PNGs (editor-friendly, 8-color indexed)
- `raw/` — Build output: .raw files (24x24, palette indices)
- `meta/` — Metadata, lists, and index files

**Pipeline:**

- Author new emoji in `png/`.
- Run the asset pipeline to generate `raw/` and update `emoji_index.json`.
- Only emoji with a .raw file and index entry will be available in the UI.
