//! A small `App` that demonstrates the full pipeline end-to-end:
//! `soul-core` event loop, `soul-ui` widgets, dirty-rect redraw,
//! and pointer input — all running in wasm against an HTML canvas.
//!
//! It is intentionally **not** the full Host (which loads scripts and
//! icon assets from the filesystem). When wasm asset loading lands, the
//! Host can replace this directly — `soul_core::run` doesn't care which
//! `App` it's driving.

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH, SYSTEM_STRIP_H};
use soul_ui::{button, hit_test, label, title_bar, BLACK, WHITE};

const FONT_W: i32 = 6;
const FONT_H: i32 = 10;

const STRIP_TOP: i32 = APP_HEIGHT as i32;
const STRIP_H: i32 = SYSTEM_STRIP_H as i32;
const STRIP_SEGMENT_W: i32 = SCREEN_WIDTH as i32 / 3;

fn button_rect() -> Rectangle {
    Rectangle::new(Point::new(70, 200), Size::new(100, 28))
}

pub struct WelcomeApp {
    /// Number of times the demo button has been tapped — proves the
    /// event loop is round-tripping pointer input correctly.
    taps: u32,
    /// Held while the pointer is down on the button so the press
    /// state can render inverted (PalmOS convention).
    pressed: bool,
}

impl WelcomeApp {
    pub fn new() -> Self {
        Self {
            taps: 0,
            pressed: false,
        }
    }
}

impl App for WelcomeApp {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::AppStart => ctx.invalidate_all(),
            Event::PenDown { x, y } => {
                if hit_test(&button_rect(), x, y) {
                    self.pressed = true;
                    ctx.invalidate(button_rect());
                }
            }
            Event::PenUp { x, y } => {
                let was_pressed = self.pressed;
                self.pressed = false;
                if was_pressed && hit_test(&button_rect(), x, y) {
                    self.taps = self.taps.saturating_add(1);
                    ctx.invalidate_all();
                } else if was_pressed {
                    ctx.invalidate(button_rect());
                }
            }
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "SoulOS");

        // Centered welcome text.
        let welcome = "Welcome";
        let welcome_x = (SCREEN_WIDTH as i32 - welcome.len() as i32 * FONT_W) / 2;
        let _ = label(canvas, Point::new(welcome_x, 80), welcome);

        let subtitle = "Web preview";
        let sub_x = (SCREEN_WIDTH as i32 - subtitle.len() as i32 * FONT_W) / 2;
        let _ = label(canvas, Point::new(sub_x, 100), subtitle);

        // Tap counter — proves the input pipeline is live.
        let mut buf = [0u8; 32];
        let count_text = format_taps(self.taps, &mut buf);
        let count_x = (SCREEN_WIDTH as i32 - count_text.len() as i32 * FONT_W) / 2;
        let _ = label(canvas, Point::new(count_x, 150), count_text);

        let _ = button(canvas, button_rect(), "Tap me", self.pressed);

        draw_system_strip(canvas);
    }
}

fn draw_system_strip<D>(canvas: &mut D)
where
    D: DrawTarget<Color = Gray8>,
{
    let strip = Rectangle::new(
        Point::new(0, STRIP_TOP),
        Size::new(SCREEN_WIDTH as u32, STRIP_H as u32),
    );
    let _ = strip
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(canvas);

    let style = MonoTextStyle::new(&FONT_6X10, WHITE);
    let y = STRIP_TOP + (STRIP_H - FONT_H) / 2;
    for (i, label) in ["Home", "SoulOS", "Menu"].iter().enumerate() {
        let x = i as i32 * STRIP_SEGMENT_W
            + (STRIP_SEGMENT_W - label.len() as i32 * FONT_W) / 2;
        let _ = Text::with_baseline(label, Point::new(x, y), style, Baseline::Top).draw(canvas);
    }
}

/// Format "Taps: N" into `buf` and return a `&str` view.
fn format_taps(n: u32, buf: &mut [u8; 32]) -> &str {
    use core::fmt::Write;
    struct Cursor<'a> {
        buf: &'a mut [u8],
        len: usize,
    }
    impl<'a> Write for Cursor<'a> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            let space = self.buf.len() - self.len;
            let take = bytes.len().min(space);
            self.buf[self.len..self.len + take].copy_from_slice(&bytes[..take]);
            self.len += take;
            Ok(())
        }
    }
    let mut cur = Cursor { buf, len: 0 };
    let _ = write!(cur, "Taps: {}", n);
    let len = cur.len;
    core::str::from_utf8(&buf[..len]).unwrap_or("Taps: ?")
}
