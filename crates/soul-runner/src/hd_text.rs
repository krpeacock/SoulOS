//! Physical-resolution text rendering hook for the desktop (minifb) platform.
//!
//! The hosted display renders at PIXEL_SCALE×PIXEL_SCALE per logical pixel.
//! `draw_text_aa_phys` on `MiniFbDisplay` rasterizes glyphs at full physical
//! resolution and writes 1:1 physical pixels, producing crisp text instead of
//! the 4×4-block-expanded gray edges that the default draw_iter path yields.
//!
//! Call `register_hosted_display` once (before `soul_core::run`) to install
//! `hosted_phys_text` as the soul-ui text hook.

use soul_hal_hosted::MiniFbDisplay;

// Pointer to the live MiniFbDisplay — valid for the duration of `main()`.
// Set once by `register_hosted_display`; never mutated after that.
static mut HOSTED_DISPLAY: Option<*mut MiniFbDisplay> = None;

/// Store a raw pointer to the `MiniFbDisplay` for use by `hosted_phys_text`.
///
/// # Safety
/// `display` must remain valid until the process exits (the typical caller
/// passes `&mut platform.display` where `platform` lives in `main`).
pub unsafe fn register_hosted_display(display: *mut MiniFbDisplay) {
    HOSTED_DISPLAY = Some(display);
}

/// Physical-resolution text renderer — signature matches `soul_ui::font_aa::set_phys_text_fn`.
///
/// Delegates to `MiniFbDisplay::draw_text_aa_phys`, which rasterizes glyphs at
/// `size_px * PIXEL_SCALE` and writes individual physical-buffer entries.
pub fn hosted_phys_text(x: i32, y: i32, text: &str, size_px: f32, luma: u8) {
    unsafe {
        if let Some(d) = HOSTED_DISPLAY {
            (*d).draw_text_aa_phys(x, y, text, size_px, luma);
        }
    }
}
