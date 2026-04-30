// Anti-aliased text rendering via fontdue at logical pixel scale.
//
// Each glyph is rasterized at the requested `size_px` in logical pixels and
// coverage values (0–255) are blended against a white background before being
// emitted as individual `Gray8` pixels.  Those pixels pass through the
// DrawTarget's `draw_iter` normally — on the hosted platform each logical pixel
// expands to a PIXEL_SCALE×PIXEL_SCALE physical block, so the gray edge
// coverage appears at 4×4 granularity rather than sub-pixel, but is still
// visibly smoother than the 1-bit bitmap fonts they replace.
//
// `draw_text_aa_phys` on `MiniFbDisplay` (soul-hal-hosted) bypasses `draw_iter`
// to write single physical pixels and achieves true sub-pixel precision.

extern crate alloc;

use alloc::boxed::Box;

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Gray8, prelude::*};
use fontdue::{Font, FontSettings};
use once_cell::race::OnceBox;

/// The three system typefaces available for text rendering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontFace {
    /// Liberation Sans — the default proportional sans-serif face.
    Sans,
    /// Liberation Serif — a proportional serif face for reading.
    Serif,
    /// Liberation Mono — a fixed-width face for code or alignment.
    Mono,
}

static SANS: OnceBox<Font> = OnceBox::new();
static SERIF: OnceBox<Font> = OnceBox::new();
static MONO: OnceBox<Font> = OnceBox::new();

const SANS_DATA: &[u8] =
    include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
const SERIF_DATA: &[u8] =
    include_bytes!("../assets/fonts/LiberationSerif-Regular.ttf");
const MONO_DATA: &[u8] =
    include_bytes!("../assets/fonts/LiberationMono-Regular.ttf");

/// Borrow the lazily-initialised `Font` for a given face.
pub fn get_font_for(face: FontFace) -> &'static Font {
    match face {
        FontFace::Sans => SANS.get_or_init(|| {
            Box::new(
                Font::from_bytes(SANS_DATA, FontSettings::default())
                    .expect("bundled Liberation Sans is valid"),
            )
        }),
        FontFace::Serif => SERIF.get_or_init(|| {
            Box::new(
                Font::from_bytes(SERIF_DATA, FontSettings::default())
                    .expect("bundled Liberation Serif is valid"),
            )
        }),
        FontFace::Mono => MONO.get_or_init(|| {
            Box::new(
                Font::from_bytes(MONO_DATA, FontSettings::default())
                    .expect("bundled Liberation Mono is valid"),
            )
        }),
    }
}

/// Draw `text` anti-aliased with an explicit face, **top of cap-height** at `(x, y)`.
///
/// `size_px` is the font size in logical pixels. `luma = 0` → black; `luma = 255` → white.
pub fn draw_text_face<D>(
    canvas: &mut D,
    text: &str,
    x: i32,
    y: i32,
    size_px: f32,
    luma: u8,
    face: FontFace,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let font = get_font_for(face);
    let cap_h = {
        let (m, _) = font.rasterize('H', size_px);
        m.height as i32
    };
    let baseline_y = y + cap_h;

    let mut cursor_x = x as f32;
    for c in text.chars() {
        let (metrics, bitmap) = font.rasterize(c, size_px);
        let glyph_top = baseline_y - (metrics.height as i32 + metrics.ymin);
        let glyph_left = cursor_x as i32 + metrics.xmin;

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + col];
                if coverage == 0 {
                    continue;
                }
                let a = coverage as u32;
                let fg = luma as u32;
                let blended = ((fg * a + 255 * (255 - a)) / 255) as u8;
                Pixel(
                    Point::new(glyph_left + col as i32, glyph_top + row as i32),
                    Gray8::new(blended),
                )
                .draw(canvas)?;
            }
        }
        cursor_x += metrics.advance_width;
    }
    Ok(())
}

/// Draw `text` anti-aliased in Sans (existing callers unchanged).
pub fn draw_text<D>(
    canvas: &mut D,
    text: &str,
    x: i32,
    y: i32,
    size_px: f32,
    luma: u8,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    draw_text_face(canvas, text, x, y, size_px, luma, FontFace::Sans)
}

/// Pixel-advance width of a single character with the given face.
pub fn char_advance(c: char, size_px: f32, face: FontFace) -> f32 {
    get_font_for(face).rasterize(c, size_px).0.advance_width
}

/// Return the pixel width of `text` rendered at `size_px` with the given face.
pub fn text_width_face(text: &str, size_px: f32, face: FontFace) -> i32 {
    text.chars()
        .map(|c| char_advance(c, size_px, face) as i32)
        .sum()
}

/// Return the pixel width of `text` rendered at `size_px` in Sans.
pub fn text_width(text: &str, size_px: f32) -> i32 {
    text_width_face(text, size_px, FontFace::Sans)
}

/// The approximate cap-height of the given face at `size_px` logical pixels.
pub fn cap_height_face(size_px: f32, face: FontFace) -> i32 {
    get_font_for(face).rasterize('H', size_px).0.height as i32
}

/// The approximate cap-height of Sans at `size_px` logical pixels.
pub fn cap_height(size_px: f32) -> i32 {
    cap_height_face(size_px, FontFace::Sans)
}
