//! Snarf — the system clipboard.
//!
//! Snarf is a single-slot, system-wide holder for content of any kind
//! the [`ExchangePayload`] enum can carry: text, bitmap, or a named
//! resource. It is named after the original Newton-era app of the same
//! purpose: a tiny utility whose only job is to *transfer*.
//!
//! # Two roles, one app
//!
//! Snarf is registered as a normal app, but it serves two distinct
//! audiences:
//!
//! - **Background clipboard service.** Other apps copy and paste by
//!   issuing a [`SystemRequest::BackgroundSend`] with action
//!   `clipboard_copy` (payload = the content) or `clipboard_paste`
//!   (payload ignored). The kernel routes the call to Snarf without
//!   pushing it onto the navigation stack or repainting the screen.
//!   For paste, Snarf returns a [`SystemRequest::SendResult`] carrying
//!   the held payload; the kernel delivers that back to the caller as
//!   an `Exchange { action: "clipboard_paste", .. }` event in the same
//!   dispatch cycle.
//!
//! - **Foreground viewer.** Snarf appears in the launcher like any
//!   other app. Tapping its icon shows what's currently held — text,
//!   bitmap thumbnail, or resource summary — with a "Clear" button.
//!   This is the rough equivalent of the PalmOS clipboard panel: a
//!   place to peek at what cut/copy actually captured.
//!
//! # Why not multi-clipboard history?
//!
//! Initial implementation deliberately keeps a single slot. A history
//! ring would make paste ambiguous (which entry?) and complicate the
//! exchange protocol. Single-slot keeps `clipboard_paste` a pure
//! function of "the last thing snarfed."

use embedded_graphics::{
    draw_target::DrawTarget,
    image::{Image, ImageRaw},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{Ctx, Event, ExchangePayload, Kind, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_ui::{button, hit_test, title_bar, BLACK, TITLE_BAR_H, WHITE};

const FONT_W: i32 = 6;
const FONT_H: i32 = 10;
const LINE_H: i32 = FONT_H + 2;

/// Width of the foreground viewer in pixels.
const VIEWER_W: i32 = SCREEN_WIDTH as i32;
/// Bottom edge available to apps (above the system strip).
const APP_BOTTOM: i32 = APP_HEIGHT as i32;

/// Height of the "Clear" button area at the bottom of the viewer.
const FOOTER_H: i32 = 32;
/// Vertical extent of the content region between the title bar and the footer.
const CONTENT_TOP: i32 = TITLE_BAR_H as i32 + 4;
const CONTENT_BOTTOM: i32 = APP_BOTTOM - FOOTER_H;

/// The Snarf app: clipboard service + viewer.
///
/// State is intentionally minimal — a single optional payload is the
/// entire clipboard. There is no persistence: clipboard contents do
/// not survive a runner restart, matching the Newton/PalmOS behavior.
pub struct Snarf {
    clipboard: Option<ExchangePayload>,
}

impl Snarf {
    pub const APP_ID: &'static str = "com.soulos.snarf";
    pub const NAME: &'static str = "Snarf";

    pub fn new() -> Self {
        Self { clipboard: None }
    }

    /// Snapshot the currently-held payload. Returns an empty payload
    /// when nothing is on the clipboard. Used by the shell-side edit
    /// menu, which talks to Snarf directly rather than going through
    /// the exchange dispatcher.
    pub fn get_payload(&self) -> ExchangePayload {
        self.clipboard.clone().unwrap_or_default()
    }

    /// Replace the held payload. The viewer is invalidated externally
    /// when needed; the host is the only caller and it manages
    /// invalidation around its own dispatch cycle.
    pub fn set_payload(&mut self, payload: ExchangePayload) {
        self.clipboard = Some(payload);
    }

    /// Handle an event from the runtime.
    ///
    /// Returns `Some(SystemRequest::SendResult { ... })` only when
    /// servicing a `clipboard_paste` exchange — that is the path that
    /// hands the held payload back to the calling app. All other
    /// events (including `clipboard_copy`) return `None`.
    pub fn handle_event(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::AppStart => {
                ctx.invalidate_all();
                None
            }
            Event::Exchange { action, payload, .. } => match action.as_str() {
                "clipboard_copy" => {
                    self.clipboard = Some(payload);
                    // Foreground may be showing us — schedule a redraw so the
                    // viewer reflects the new contents on the next frame. If
                    // we are not active, this invalidate is a cheap no-op
                    // because the runtime won't draw an inactive app.
                    ctx.invalidate_all();
                    None
                }
                "clipboard_paste" => Some(SystemRequest::SendResult {
                    action: "clipboard_paste".to_string(),
                    payload: self.clipboard.clone().unwrap_or_default(),
                }),
                _ => None,
            },
            Event::PenUp { x, y } => {
                if hit_test(&clear_button_rect(), x, y) {
                    self.clipboard = None;
                    ctx.invalidate_all();
                }
                None
            }
            _ => None,
        }
    }

    pub fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D, _dirty: Rectangle) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Snarf");

        let header_y = CONTENT_TOP;
        let header = match self.clipboard.as_ref().and_then(|p| p.primary()) {
            None => "Clipboard is empty".to_string(),
            Some(rep) => match soul_core::classify_mime(&rep.mime) {
                Kind::Text => format!("Text  ·  {} bytes", rep.bytes.len()),
                Kind::Bitmap => match rep.as_bitmap() {
                    Some(bm) => format!("Bitmap  ·  {} × {}", bm.width, bm.height),
                    None => "Bitmap  ·  (invalid)".to_string(),
                },
                Kind::Other => format!("{}  ·  {} bytes", rep.mime, rep.bytes.len()),
            },
        };
        let style = MonoTextStyle::new(&FONT_6X10, BLACK);
        let header_x = ((VIEWER_W - header.chars().count() as i32 * FONT_W) / 2).max(2);
        let _ = Text::with_baseline(
            &header,
            Point::new(header_x, header_y),
            style,
            Baseline::Top,
        )
        .draw(canvas);

        let preview_top = header_y + LINE_H + 6;
        let preview_rect = Rectangle::new(
            Point::new(4, preview_top),
            Size::new(
                (VIEWER_W - 8) as u32,
                (CONTENT_BOTTOM - preview_top - 4).max(0) as u32,
            ),
        );

        match self.clipboard.as_ref().and_then(|p| p.primary()) {
            None => {
                draw_centered(canvas, preview_rect, "(nothing snarfed yet)");
            }
            Some(rep) => match soul_core::classify_mime(&rep.mime) {
                Kind::Text => match rep.as_text() {
                    Some(t) => draw_text_preview(canvas, preview_rect, t),
                    None => draw_centered(canvas, preview_rect, "(invalid text)"),
                },
                Kind::Bitmap => match rep.as_bitmap() {
                    Some(bm) => draw_bitmap_preview(canvas, preview_rect, bm.width, bm.height, &bm.pixels),
                    None => draw_centered(canvas, preview_rect, "(invalid bitmap)"),
                },
                Kind::Other => {
                    let msg = format!("(unknown: {})", rep.mime);
                    draw_centered(canvas, preview_rect, &msg);
                }
            },
        }

        let _ = button(canvas, clear_button_rect(), "Clear", false);
    }

    pub fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        vec![soul_core::a11y::A11yNode {
            bounds: clear_button_rect(),
            label: "Clear".into(),
            role: "button".into(),
        }]
    }

    pub fn persist(&mut self) {
        // Snarf is intentionally non-persistent: the clipboard is
        // cleared by quitting the runner, just like every other OS.
    }
}

impl Default for Snarf {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounds of the bottom-of-viewer "Clear" button.
fn clear_button_rect() -> Rectangle {
    let w: i32 = 80;
    let h: i32 = 22;
    let x = (VIEWER_W - w) / 2;
    let y = APP_BOTTOM - h - 6;
    Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32))
}

/// Center a single line of text inside `rect` using the default 6x10 font.
fn draw_centered<D: DrawTarget<Color = Gray8>>(canvas: &mut D, rect: Rectangle, text: &str) {
    let style = MonoTextStyle::new(&FONT_6X10, BLACK);
    let w = text.chars().count() as i32 * FONT_W;
    let x = rect.top_left.x + (rect.size.width as i32 - w) / 2;
    let y = rect.top_left.y + (rect.size.height as i32 - FONT_H) / 2;
    let _ = Text::with_baseline(text, Point::new(x.max(2), y.max(rect.top_left.y)), style, Baseline::Top)
        .draw(canvas);
}

/// Render the first chunk of `text` line-by-line inside `rect`.
///
/// Long lines are visually clipped at the right edge of the rectangle by
/// truncating to the column count that fits; tabs and control characters
/// are replaced with spaces. We deliberately do not word-wrap: the goal
/// is a faithful glance at what the clipboard holds, not a text editor.
fn draw_text_preview<D: DrawTarget<Color = Gray8>>(canvas: &mut D, rect: Rectangle, text: &str) {
    let style = MonoTextStyle::new(&FONT_6X10, BLACK);
    let cols = ((rect.size.width as i32) / FONT_W).max(1) as usize;
    let max_lines = ((rect.size.height as i32) / LINE_H).max(1) as usize;

    let mut y = rect.top_left.y;
    let mut drawn = 0usize;
    for raw in text.lines() {
        if drawn >= max_lines {
            break;
        }
        let cleaned: String = raw
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let truncated: String = cleaned.chars().take(cols).collect();
        let _ = Text::with_baseline(
            &truncated,
            Point::new(rect.top_left.x + 2, y),
            style,
            Baseline::Top,
        )
        .draw(canvas);
        y += LINE_H;
        drawn += 1;
    }
    if drawn == 0 {
        draw_centered(canvas, rect, "(empty text)");
    }
}

/// Render a grayscale bitmap centered inside `rect`.
///
/// The bitmap is drawn at 1:1 if it fits; otherwise the available
/// region is filled with a "too large to preview" message. We keep
/// preview cheap: scaling and downsampling belong in a dedicated
/// image viewer, not in the clipboard inspector.
fn draw_bitmap_preview<D: DrawTarget<Color = Gray8>>(
    canvas: &mut D,
    rect: Rectangle,
    width: u16,
    height: u16,
    pixels: &[u8],
) {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || pixels.len() != w * h {
        draw_centered(canvas, rect, "(invalid bitmap)");
        return;
    }
    if (width as i32) > rect.size.width as i32 || (height as i32) > rect.size.height as i32 {
        draw_centered(canvas, rect, "(too large to preview)");
        return;
    }
    let x = rect.top_left.x + (rect.size.width as i32 - width as i32) / 2;
    let y = rect.top_left.y + (rect.size.height as i32 - height as i32) / 2;

    let frame = Rectangle::new(Point::new(x - 1, y - 1), Size::new(width as u32 + 2, height as u32 + 2));
    let _ = frame.into_styled(PrimitiveStyle::with_stroke(BLACK, 1)).draw(canvas);
    let _ = Rectangle::new(Point::new(x, y), Size::new(width as u32, height as u32))
        .into_styled(PrimitiveStyle::with_fill(WHITE))
        .draw(canvas);

    let raw = ImageRaw::<Gray8>::new(pixels, width as u32);
    let _ = Image::new(&raw, Point::new(x, y)).draw(canvas);
}
