//! Monochrome emoji glyphs for monospace text rendering.
//!
//! SoulOS targets paper-like monochrome and grayscale displays. The
//! modern colour-emoji pipeline (subpixel COLR/CBDT, ZWJ sequences,
//! skin-tone modifiers) does not fit that aesthetic and would dwarf
//! the entire rest of the UI in code size. Instead, this module ships
//! a small fixed table of 5×8 BMP-symbol bitmaps — the same sort of
//! marks a user would scrawl in the margin of a paper notebook.
//!
//! Each glyph is rendered into one monospace cell, the same width as
//! the surrounding ASCII text. That keeps every existing layout,
//! cursor, hit-test, and word-wrap calculation byte-for-byte
//! unchanged. An unrecognised character falls through to the normal
//! `embedded-graphics` font path.
//!
//! Use [`draw_text`] in place of `Text::with_baseline(...).draw(...)`
//! when you want emoji fallback. ASCII strings render identically.

use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Gray8,
    prelude::*,
    text::{Baseline, Text},
};

const GLYPH_W: usize = 5;
const GLYPH_H: usize = 8;

/// One 5×8 monochrome glyph. Each byte is a row; the low five bits
/// are columns, with bit 4 = leftmost column.
type Glyph = [u8; GLYPH_H];

/// Lookup table of (Unicode scalar, bitmap) pairs. Kept small and
/// sorted by code point. Linear scan is fine — at this size it's
/// faster than any map and costs no allocation.
const TABLE: &[(char, Glyph)] = &[
    // ⌂ U+2302 house
    ('\u{2302}', [0x04, 0x0E, 0x1F, 0x11, 0x15, 0x15, 0x1F, 0x00]),
    // ☀ U+2600 sun
    ('\u{2600}', [0x04, 0x15, 0x0E, 0x1F, 0x0E, 0x15, 0x04, 0x00]),
    // ☁ U+2601 cloud
    ('\u{2601}', [0x00, 0x06, 0x0F, 0x1F, 0x1F, 0x1F, 0x00, 0x00]),
    // ☂ U+2602 umbrella
    ('\u{2602}', [0x04, 0x0E, 0x1F, 0x1F, 0x04, 0x04, 0x0C, 0x00]),
    // ★ U+2605 star
    ('\u{2605}', [0x04, 0x0E, 0x1F, 0x0E, 0x1B, 0x00, 0x00, 0x00]),
    // ☎ U+260E telephone
    ('\u{260E}', [0x0C, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x0C, 0x00]),
    // ☐ U+2610 ballot box
    ('\u{2610}', [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F, 0x00]),
    // ☑ U+2611 ballot box with check
    ('\u{2611}', [0x1F, 0x11, 0x13, 0x17, 0x1D, 0x19, 0x1F, 0x00]),
    // ☹ U+2639 frowning face
    ('\u{2639}', [0x0E, 0x11, 0x15, 0x11, 0x11, 0x0E, 0x1B, 0x00]),
    // ☺ U+263A smiling face
    ('\u{263A}', [0x0E, 0x11, 0x15, 0x11, 0x11, 0x1B, 0x0E, 0x00]),
    // ♥ U+2665 heart
    ('\u{2665}', [0x0A, 0x1F, 0x1F, 0x0E, 0x04, 0x00, 0x00, 0x00]),
    // ♪ U+266A note
    ('\u{266A}', [0x03, 0x03, 0x02, 0x02, 0x02, 0x0E, 0x1E, 0x0C]),
    // ⚙ U+2699 gear
    ('\u{2699}', [0x04, 0x1F, 0x0E, 0x1B, 0x1B, 0x0E, 0x1F, 0x04]),
    // ⚠ U+26A0 warning
    ('\u{26A0}', [0x04, 0x0A, 0x0A, 0x1B, 0x1B, 0x11, 0x1F, 0x00]),
    // ⚡ U+26A1 lightning
    ('\u{26A1}', [0x06, 0x0C, 0x1E, 0x06, 0x0C, 0x18, 0x00, 0x00]),
    // ✉ U+2709 envelope
    ('\u{2709}', [0x1F, 0x1B, 0x15, 0x11, 0x11, 0x1F, 0x00, 0x00]),
    // ✓ U+2713 check
    ('\u{2713}', [0x00, 0x01, 0x02, 0x12, 0x0A, 0x04, 0x00, 0x00]),
    // ✗ U+2717 ballot x
    ('\u{2717}', [0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x00, 0x00]),
];

/// Look up the bitmap for `c`. Returns `None` for non-emoji code
/// points (including all of ASCII and Latin-1 — ordinary characters
/// continue to render through the regular font).
pub fn lookup(c: char) -> Option<&'static Glyph> {
    TABLE.iter().find(|(k, _)| *k == c).map(|(_, g)| g)
}

/// Returns true when [`draw_text`] would render `c` from the emoji
/// table rather than from the fallback font.
pub fn is_emoji(c: char) -> bool {
    lookup(c).is_some()
}

/// Draw `text` as a monospace run, substituting bitmaps from the
/// emoji table for any code points that have one and deferring to
/// `style.font` for everything else.
///
/// Each character — emoji or not — advances the pen by exactly one
/// font cell, so layout code that assumes fixed-width text remains
/// correct verbatim.
pub fn draw_text<D>(
    canvas: &mut D,
    text: &str,
    pos: Point,
    style: MonoTextStyle<'_, Gray8>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let cell_w = style.font.character_size.width as i32;
    let cell_h = style.font.character_size.height as i32;
    let color = style.text_color.unwrap_or(super::palette::BLACK);
    let mut x = pos.x;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        if let Some(glyph) = lookup(c) {
            draw_glyph(canvas, glyph, Point::new(x, pos.y), cell_w, cell_h, color)?;
        } else {
            let s = c.encode_utf8(&mut buf);
            Text::with_baseline(s, Point::new(x, pos.y), style, Baseline::Top).draw(canvas)?;
        }
        x += cell_w;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_falls_through() {
        assert!(lookup('a').is_none());
        assert!(lookup(' ').is_none());
        assert!(lookup('!').is_none());
        assert!(!is_emoji('z'));
    }

    #[test]
    fn known_emoji_resolve() {
        assert!(is_emoji('\u{2665}')); // heart
        assert!(is_emoji('\u{2605}')); // star
        assert!(is_emoji('\u{2713}')); // check
        assert!(is_emoji('\u{2611}')); // checked box
    }

    #[test]
    fn table_is_sorted_by_code_point() {
        for w in TABLE.windows(2) {
            assert!(
                w[0].0 < w[1].0,
                "emoji table out of order: U+{:04X} should precede U+{:04X}",
                w[1].0 as u32,
                w[0].0 as u32,
            );
        }
    }
}

fn draw_glyph<D>(
    canvas: &mut D,
    glyph: &Glyph,
    pos: Point,
    cell_w: i32,
    cell_h: i32,
    color: Gray8,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let off_x = (cell_w - GLYPH_W as i32) / 2;
    let off_y = (cell_h - GLYPH_H as i32) / 2;
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..GLYPH_W {
            if bits & (1 << (GLYPH_W - 1 - col)) != 0 {
                Pixel(
                    Point::new(pos.x + off_x + col as i32, pos.y + off_y + row as i32),
                    color,
                )
                .draw(canvas)?;
            }
        }
    }
    Ok(())
}
