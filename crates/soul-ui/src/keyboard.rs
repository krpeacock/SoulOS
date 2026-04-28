//! On-screen keyboard widget.
//!
//! [`Keyboard`] is a stateful widget that renders a QWERTY-style
//! soft keyboard and consumes stylus/pointer events from the SoulOS
//! runtime. It manages its own layer state ([`Layer::Lower`],
//! [`Layer::Upper`], [`Layer::Symbols`]) and produces [`TypedKey`]
//! events for its owning app to interpret.
//!
//! # Lifecycle
//!
//! The app owns the [`Keyboard`] in its state and forwards pen
//! events to the widget. The widget returns [`Rectangle`]s the app
//! should feed to `Ctx::invalidate` so the dirty-rect runtime
//! repaints exactly what changed:
//!
//! ```ignore
//! use soul_ui::{Keyboard, TypedKey};
//!
//! struct MyApp { keyboard: Keyboard, buffer: String }
//!
//! impl soul_core::App for MyApp {
//!     fn handle(&mut self, event: soul_core::Event, ctx: &mut soul_core::Ctx<'_>) {
//!         match event {
//!             soul_core::Event::PenDown { x, y }
//!             | soul_core::Event::PenMove { x, y } => {
//!                 if let Some(r) = self.keyboard.pen_moved(x, y) {
//!                     ctx.invalidate(r);
//!                 }
//!             }
//!             soul_core::Event::PenUp { x, y } => {
//!                 let out = self.keyboard.pen_released(x, y);
//!                 if let Some(r) = out.dirty { ctx.invalidate(r); }
//!                 match out.typed {
//!                     Some(TypedKey::Char(c)) => self.buffer.push(c),
//!                     Some(TypedKey::Backspace) => { self.buffer.pop(); }
//!                     Some(TypedKey::Enter) => self.buffer.push('\n'),
//!                     None => {}
//!                 }
//!             }
//!             _ => {}
//!         }
//!     }
//!     /* fn draw(...) { self.keyboard.draw(canvas); } */
//! #    fn draw<D: embedded_graphics::draw_target::DrawTarget<Color = embedded_graphics::pixelcolor::Gray8>>(&mut self, _c: &mut D) {}
//! }
//! ```
//!
//! # Layers
//!
//! The keyboard starts in [`Layer::Lower`]. Tapping `sh` toggles to
//! [`Layer::Upper`]; tapping `123` switches to [`Layer::Symbols`],
//! where an `ABC` key returns to lowercase. Apps can query the
//! current layer with [`Keyboard::layer`] but should not need to.

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
};

use crate::emoji;
use crate::palette::{BLACK, GRAY, WHITE};

/// Total keyboard height in pixels. Apps sizing a text area above the
/// keyboard should subtract this from the screen height.
pub const KEYBOARD_HEIGHT: u32 = 96;

/// Total keyboard width in pixels. Always equal to the virtual screen
/// width.
pub const KEYBOARD_WIDTH: u32 = 240;

const KEY_ROW_H: u32 = 24;
const KEY_CELL_W: u32 = 24;

const FONT_W: i32 = 6;
const FONT_H: i32 = 10;

/// Internal description of a keycap: logical key, visible label,
/// width in cells (1 cell = [`KEY_CELL_W`] px).
type Kd = (Key, &'static str, u8);

/// Internal key identity used by the layout tables and the press
/// handler. Apps receive [`TypedKey`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Char(char),
    Backspace,
    Return,
    Space,
    /// Toggles between lowercase and uppercase layers.
    Shift,
    /// Switches to the symbols/numbers layer.
    Numbers,
    /// (Symbols/Emoji layer only.) Switches back to lowercase letters.
    Letters,
    /// Switches to the emoji layer. Available from every layer's
    /// bottom row so emoji are one tap from any input mode.
    Emoji,
}

/// Which set of keycaps the keyboard is currently displaying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Lowercase QWERTY plus `, space .` and backspace/return.
    Lower,
    /// Uppercase QWERTY. Shift is rendered visually "down".
    Upper,
    /// Digits `0–9`, common punctuation, and symbols.
    Symbols,
    /// Paper-style emoji glyphs from [`crate::emoji`].
    Emoji,
}

/// A user-facing key press produced by the keyboard when the stylus
/// is released over the same key that was initially pressed. Modifier
/// keys (Shift, `123`, `ABC`) change state internally and do *not*
/// produce a `TypedKey`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedKey {
    /// A printable character. The value reflects the current layer
    /// (e.g., `'A'` while the keyboard is in [`Layer::Upper`]).
    Char(char),
    /// The delete-left key.
    Backspace,
    /// The return / newline key.
    Enter,
}

/// The result of forwarding a pen-released event to the keyboard.
///
/// `typed` is `Some` when the user released over a printable or
/// editing key (not a layer-switch). `dirty` is a bounding rectangle
/// covering everything that visually changed (key highlight clears,
/// layer re-renders, etc.) and should be passed to
/// `Ctx::invalidate`.
#[derive(Default, Debug, Clone, Copy)]
#[must_use = "dirty rectangles must be passed to Ctx::invalidate and typed keys should be applied"]
pub struct KeyboardOutput {
    /// The character or editing intent produced by this release, or
    /// `None` if no key was typed (layer switch or miss).
    pub typed: Option<TypedKey>,
    /// Bounding rectangle of pixels that need to be redrawn.
    pub dirty: Option<Rectangle>,
}

// -- layout tables -----------------------------------------------------------

const L_ROW0: &[Kd] = &[
    (Key::Char('q'), "q", 1),
    (Key::Char('w'), "w", 1),
    (Key::Char('e'), "e", 1),
    (Key::Char('r'), "r", 1),
    (Key::Char('t'), "t", 1),
    (Key::Char('y'), "y", 1),
    (Key::Char('u'), "u", 1),
    (Key::Char('i'), "i", 1),
    (Key::Char('o'), "o", 1),
    (Key::Char('p'), "p", 1),
];
const L_ROW1: &[Kd] = &[
    (Key::Char('a'), "a", 1),
    (Key::Char('s'), "s", 1),
    (Key::Char('d'), "d", 1),
    (Key::Char('f'), "f", 1),
    (Key::Char('g'), "g", 1),
    (Key::Char('h'), "h", 1),
    (Key::Char('j'), "j", 1),
    (Key::Char('k'), "k", 1),
    (Key::Char('l'), "l", 1),
];
const L_ROW2: &[Kd] = &[
    (Key::Shift, "sh", 1),
    (Key::Char('z'), "z", 1),
    (Key::Char('x'), "x", 1),
    (Key::Char('c'), "c", 1),
    (Key::Char('v'), "v", 1),
    (Key::Char('b'), "b", 1),
    (Key::Char('n'), "n", 1),
    (Key::Char('m'), "m", 1),
    (Key::Backspace, "del", 2),
];
const L_ROW3: &[Kd] = &[
    (Key::Numbers, "123", 2),
    (Key::Emoji, "\u{263A}", 1),
    (Key::Char(','), ",", 1),
    (Key::Space, "space", 3),
    (Key::Char('.'), ".", 1),
    (Key::Return, "ret", 2),
];

const U_ROW0: &[Kd] = &[
    (Key::Char('Q'), "Q", 1),
    (Key::Char('W'), "W", 1),
    (Key::Char('E'), "E", 1),
    (Key::Char('R'), "R", 1),
    (Key::Char('T'), "T", 1),
    (Key::Char('Y'), "Y", 1),
    (Key::Char('U'), "U", 1),
    (Key::Char('I'), "I", 1),
    (Key::Char('O'), "O", 1),
    (Key::Char('P'), "P", 1),
];
const U_ROW1: &[Kd] = &[
    (Key::Char('A'), "A", 1),
    (Key::Char('S'), "S", 1),
    (Key::Char('D'), "D", 1),
    (Key::Char('F'), "F", 1),
    (Key::Char('G'), "G", 1),
    (Key::Char('H'), "H", 1),
    (Key::Char('J'), "J", 1),
    (Key::Char('K'), "K", 1),
    (Key::Char('L'), "L", 1),
];
const U_ROW2: &[Kd] = &[
    (Key::Shift, "sh", 1),
    (Key::Char('Z'), "Z", 1),
    (Key::Char('X'), "X", 1),
    (Key::Char('C'), "C", 1),
    (Key::Char('V'), "V", 1),
    (Key::Char('B'), "B", 1),
    (Key::Char('N'), "N", 1),
    (Key::Char('M'), "M", 1),
    (Key::Backspace, "del", 2),
];
const U_ROW3: &[Kd] = &[
    (Key::Numbers, "123", 2),
    (Key::Emoji, "\u{263A}", 1),
    (Key::Char(','), ",", 1),
    (Key::Space, "space", 3),
    (Key::Char('.'), ".", 1),
    (Key::Return, "ret", 2),
];

const S_ROW0: &[Kd] = &[
    (Key::Char('1'), "1", 1),
    (Key::Char('2'), "2", 1),
    (Key::Char('3'), "3", 1),
    (Key::Char('4'), "4", 1),
    (Key::Char('5'), "5", 1),
    (Key::Char('6'), "6", 1),
    (Key::Char('7'), "7", 1),
    (Key::Char('8'), "8", 1),
    (Key::Char('9'), "9", 1),
    (Key::Char('0'), "0", 1),
];
const S_ROW1: &[Kd] = &[
    (Key::Char('-'), "-", 1),
    (Key::Char('/'), "/", 1),
    (Key::Char(':'), ":", 1),
    (Key::Char(';'), ";", 1),
    (Key::Char('('), "(", 1),
    (Key::Char(')'), ")", 1),
    (Key::Char('$'), "$", 1),
    (Key::Char('&'), "&", 1),
    (Key::Char('@'), "@", 1),
];
const S_ROW2: &[Kd] = &[
    (Key::Char('!'), "!", 1),
    (Key::Char('?'), "?", 1),
    (Key::Char('+'), "+", 1),
    (Key::Char('='), "=", 1),
    (Key::Char('#'), "#", 1),
    (Key::Char('"'), "\"", 1),
    (Key::Char('\''), "'", 1),
    (Key::Char('*'), "*", 1),
    (Key::Backspace, "del", 2),
];
const S_ROW3: &[Kd] = &[
    (Key::Letters, "ABC", 2),
    (Key::Char(','), ",", 1),
    (Key::Space, "space", 4),
    (Key::Char('.'), ".", 1),
    (Key::Return, "ret", 2),
];

const E_ROW0: &[Kd] = &[
    (Key::Char('\u{2665}'), "\u{2665}", 1),
    (Key::Char('\u{2605}'), "\u{2605}", 1),
    (Key::Char('\u{263A}'), "\u{263A}", 1),
    (Key::Char('\u{2639}'), "\u{2639}", 1),
    (Key::Char('\u{2713}'), "\u{2713}", 1),
    (Key::Char('\u{2717}'), "\u{2717}", 1),
    (Key::Char('\u{2610}'), "\u{2610}", 1),
    (Key::Char('\u{2611}'), "\u{2611}", 1),
    (Key::Char('\u{2600}'), "\u{2600}", 1),
    (Key::Char('\u{266A}'), "\u{266A}", 1),
];
const E_ROW1: &[Kd] = &[
    (Key::Char('\u{26A0}'), "\u{26A0}", 1),
    (Key::Char('\u{2709}'), "\u{2709}", 1),
    (Key::Char('\u{2602}'), "\u{2602}", 1),
    (Key::Char('\u{2601}'), "\u{2601}", 1),
    (Key::Char('\u{2302}'), "\u{2302}", 1),
    (Key::Char('\u{260E}'), "\u{260E}", 1),
    (Key::Char('\u{2699}'), "\u{2699}", 1),
    (Key::Char('\u{26A1}'), "\u{26A1}", 1),
    (Key::Backspace, "del", 2),
];
const E_ROW2: &[Kd] = &[];
const E_ROW3: &[Kd] = &[
    (Key::Letters, "ABC", 2),
    (Key::Char(','), ",", 1),
    (Key::Space, "space", 4),
    (Key::Char('.'), ".", 1),
    (Key::Return, "ret", 2),
];

const ROW_INDENTS: [i32; 4] = [0, 12, 0, 0];

/// A stateful on-screen keyboard.
///
/// Owns its current layer, the in-flight highlighted key under the
/// user's finger, and nothing else. Construct once at the top of
/// your `handle`-able state and reuse across frames.
#[derive(Clone)]
pub struct Keyboard {
    top_y: i32,
    layer: Layer,
    highlighted: Option<(Key, Rectangle)>,
}

impl Keyboard {
    /// Create a keyboard whose top edge sits at `top_y` on the
    /// virtual screen. The keyboard extends [`KEYBOARD_HEIGHT`]
    /// pixels downward and [`KEYBOARD_WIDTH`] pixels across.
    pub fn new(top_y: i32) -> Self {
        Self {
            top_y,
            layer: Layer::Lower,
            highlighted: None,
        }
    }

    /// The layer currently displayed. Provided for diagnostics; you
    /// normally don't need this — `pen_released` handles layer
    /// transitions internally.
    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// The bounding rectangle of the keyboard in screen
    /// coordinates. Useful if your app wants to hit-test whether a
    /// tap fell on the keyboard vs. on surrounding chrome.
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(
            Point::new(0, self.top_y),
            Size::new(KEYBOARD_WIDTH, KEYBOARD_HEIGHT),
        )
    }

    /// Feed a pen-down or pen-move coordinate to the keyboard.
    ///
    /// Updates the highlighted key and returns a bounding rectangle
    /// to repaint (or `None` if no visual state changed).
    pub fn pen_moved(&mut self, x: i16, y: i16) -> Option<Rectangle> {
        let new_hit = self.hit_at(x, y);
        let new_key = new_hit.map(|(k, _)| k);
        let old_key = self.highlighted.map(|(k, _)| k);
        if new_key == old_key {
            return None;
        }
        let mut dirty: Option<Rectangle> = None;
        if let Some((_, r)) = self.highlighted {
            dirty = Some(merge(dirty, r));
        }
        if let Some((_, r)) = new_hit {
            dirty = Some(merge(dirty, r));
        }
        self.highlighted = new_hit;
        dirty
    }

    /// Feed a pen-up coordinate to the keyboard.
    ///
    /// Returns a [`KeyboardOutput`] describing what was typed (if
    /// anything) and which rectangle needs to be repainted. If
    /// pen-up happens on the same key that was highlighted, the
    /// keyboard "commits" the press.
    pub fn pen_released(&mut self, x: i16, y: i16) -> KeyboardOutput {
        let released = self.hit_at(x, y).map(|(k, _)| k);
        let held = self.highlighted;
        let mut dirty: Option<Rectangle> = None;
        if let Some((_, r)) = held {
            dirty = Some(merge(dirty, r));
        }
        self.highlighted = None;
        let mut typed = None;
        if let (Some((k, _)), Some(r)) = (held, released) {
            if k == r {
                match k {
                    Key::Char(c) => typed = Some(TypedKey::Char(c)),
                    Key::Space => typed = Some(TypedKey::Char(' ')),
                    Key::Backspace => typed = Some(TypedKey::Backspace),
                    Key::Return => typed = Some(TypedKey::Enter),
                    Key::Shift => {
                        self.layer = match self.layer {
                            Layer::Lower => Layer::Upper,
                            Layer::Upper => Layer::Lower,
                            Layer::Symbols => Layer::Symbols,
                            Layer::Emoji => Layer::Emoji,
                        };
                        dirty = Some(merge(dirty, self.bounds()));
                    }
                    Key::Numbers => {
                        self.layer = Layer::Symbols;
                        dirty = Some(merge(dirty, self.bounds()));
                    }
                    Key::Letters => {
                        self.layer = Layer::Lower;
                        dirty = Some(merge(dirty, self.bounds()));
                    }
                    Key::Emoji => {
                        self.layer = Layer::Emoji;
                        dirty = Some(merge(dirty, self.bounds()));
                    }
                }
            }
        }
        KeyboardOutput { typed, dirty }
    }

    /// Drop any in-flight highlight without producing a press. Call
    /// when the user's pen strays off the keyboard onto other UI.
    /// Returns the rect of the previously-highlighted key, if any.
    pub fn pen_cancelled(&mut self) -> Option<Rectangle> {
        let r = self.highlighted.map(|(_, r)| r);
        self.highlighted = None;
        r
    }

    /// Render the keyboard into `canvas`. Call unconditionally from
    /// your app's `draw()`; the runtime clips it to the dirty rect.
    pub fn draw<D>(&self, canvas: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let bg = PrimitiveStyleBuilder::new().fill_color(GRAY).build();
        Rectangle::new(
            Point::new(0, self.top_y),
            Size::new(KEYBOARD_WIDTH, KEYBOARD_HEIGHT),
        )
        .into_styled(bg)
        .draw(canvas)?;

        let rows = self.rows();
        let highlight = self.highlighted.map(|(k, _)| k);
        let shift_active = self.layer == Layer::Upper;

        for (row_i, row) in rows.iter().enumerate() {
            let y = self.top_y + (row_i as i32) * KEY_ROW_H as i32;
            let mut x = ROW_INDENTS[row_i];
            for kd in row.iter() {
                let w = kd.2 as u32 * KEY_CELL_W;
                let rect =
                    Rectangle::new(Point::new(x + 1, y + 1), Size::new(w - 2, KEY_ROW_H - 2));
                let pressed =
                    Some(kd.0) == highlight || (matches!(kd.0, Key::Shift) && shift_active);
                key_cap(canvas, rect, kd.1, pressed)?;
                x += w as i32;
            }
        }
        Ok(())
    }

    fn rows(&self) -> [&'static [Kd]; 4] {
        match self.layer {
            Layer::Lower => [L_ROW0, L_ROW1, L_ROW2, L_ROW3],
            Layer::Upper => [U_ROW0, U_ROW1, U_ROW2, U_ROW3],
            Layer::Symbols => [S_ROW0, S_ROW1, S_ROW2, S_ROW3],
            Layer::Emoji => [E_ROW0, E_ROW1, E_ROW2, E_ROW3],
        }
    }

    fn hit_at(&self, x: i16, y: i16) -> Option<(Key, Rectangle)> {
        let y_rel = y as i32 - self.top_y;
        if y_rel < 0 || y_rel >= KEYBOARD_HEIGHT as i32 {
            return None;
        }
        let row_i = (y_rel / KEY_ROW_H as i32) as usize;
        if row_i >= 4 {
            return None;
        }
        let rows = self.rows();
        let mut x_cur = ROW_INDENTS[row_i];
        for kd in rows[row_i].iter() {
            let w = kd.2 as i32 * KEY_CELL_W as i32;
            if (x as i32) >= x_cur && (x as i32) < x_cur + w {
                let rect = Rectangle::new(
                    Point::new(
                        x_cur + 1,
                        self.top_y + (row_i as i32) * KEY_ROW_H as i32 + 1,
                    ),
                    Size::new(w as u32 - 2, KEY_ROW_H - 2),
                );
                return Some((kd.0, rect));
            }
            x_cur += w;
        }
        None
    }
}

fn key_cap<D>(canvas: &mut D, rect: Rectangle, label: &str, pressed: bool) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let (fill, text_color) = if pressed {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };
    let style = PrimitiveStyleBuilder::new()
        .fill_color(fill)
        .stroke_color(BLACK)
        .stroke_width(1)
        .build();
    RoundedRectangle::with_equal_corners(rect, Size::new(2, 2))
        .into_styled(style)
        .draw(canvas)?;
    let label_w = label.chars().count() as i32 * FONT_W;
    let pos = rect.top_left
        + Point::new(
            (rect.size.width as i32 - label_w) / 2,
            (rect.size.height as i32 - FONT_H) / 2,
        );
    let text_style = MonoTextStyle::new(&FONT_6X10, text_color);
    emoji::draw_text(canvas, label, pos, text_style)?;
    Ok(())
}

fn merge(existing: Option<Rectangle>, new: Rectangle) -> Rectangle {
    match existing {
        Some(e) => union_rect(e, new),
        None => new,
    }
}

fn union_rect(a: Rectangle, b: Rectangle) -> Rectangle {
    let ax1 = a.top_left.x + a.size.width as i32;
    let ay1 = a.top_left.y + a.size.height as i32;
    let bx1 = b.top_left.x + b.size.width as i32;
    let by1 = b.top_left.y + b.size.height as i32;
    let x0 = a.top_left.x.min(b.top_left.x);
    let y0 = a.top_left.y.min(b.top_left.y);
    let x1 = ax1.max(bx1);
    let y1 = ay1.max(by1);
    Rectangle::new(
        Point::new(x0, y0),
        Size::new((x1 - x0) as u32, (y1 - y0) as u32),
    )
}
