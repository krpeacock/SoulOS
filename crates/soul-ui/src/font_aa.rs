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

static FONT: OnceBox<Font> = OnceBox::new();

const FONT_DATA: &[u8] =
    include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");

fn get_font() -> &'static Font {
    FONT.get_or_init(|| {
        Box::new(
            Font::from_bytes(FONT_DATA, FontSettings::default())
                .expect("bundled Liberation Sans is valid"),
        )
    })
}

/// Draw `text` anti-aliased, with the **top of the cap-height** at `(x, y)`.
///
/// `size_px` is the font size in logical pixels (roughly the cap-height).
/// `luma = 0` draws black text; `luma = 255` draws white text.
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
    let font = get_font();
    // Determine cap-height from 'H' so we can top-align consistently.
    let cap_h = {
        let (m, _) = font.rasterize('H', size_px);
        m.height as i32
    };
    let baseline_y = y + cap_h;

    let mut cursor_x = x as f32;
    for c in text.chars() {
        let (metrics, bitmap) = font.rasterize(c, size_px);
        // glyph top in screen-y (down = positive):
        //   baseline_y − (height + ymin)  because ymin is from baseline to glyph bottom
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
                // Blend against white (255): full coverage → luma, none → 255.
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

/// Return the pixel width of `text` rendered at `size_px` logical pixels.
pub fn text_width(text: &str, size_px: f32) -> i32 {
    let font = get_font();
    text.chars()
        .map(|c| font.rasterize(c, size_px).0.advance_width as i32)
        .sum()
}

/// The approximate cap-height of the font at `size_px` logical pixels.
pub fn cap_height(size_px: f32) -> i32 {
    let font = get_font();
    font.rasterize('H', size_px).0.height as i32
}
