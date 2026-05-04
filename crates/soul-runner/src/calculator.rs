//! Native calculator app — f64 arithmetic, decimal input, right-aligned display.

use embedded_graphics::{
    mono_font::{ascii::{FONT_6X10, FONT_10X20}, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Baseline, Text},
};
use soul_core::{a11y::{A11yNode, A11yRole}, App, Ctx, Event, KeyCode, APP_HEIGHT, SCREEN_WIDTH};
use soul_ui::{hit_test, BLACK, WHITE};

// ── Layout constants ─────────────────────────────────────────────────────────

const W: i32 = SCREEN_WIDTH as i32;   // 240
const H: i32 = APP_HEIGHT as i32;     // 304

const TITLE_H: i32 = 15;              // title bar
const DISP_TOP: i32 = TITLE_H;        // display area starts here
const DISP_H: i32 = 56;               // display area height (context + number)
const BTN_TOP: i32 = DISP_TOP + DISP_H; // 71

// 5 rows × 4 columns, fitted into remaining height
const ROWS: i32 = 5;
const COLS: i32 = 4;
const GAP: i32 = 3;

// Derived: fill the space exactly
const BTN_W: i32 = (W - GAP * (COLS - 1)) / COLS;          // 57
const BTN_H: i32 = (H - BTN_TOP - GAP * (ROWS - 1)) / ROWS; // 45

// ── Button actions ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Op { Add, Sub, Mul, Div, Rem }

impl Op {
    fn symbol(self) -> char {
        match self { Op::Add => '+', Op::Sub => '-', Op::Mul => '*', Op::Div => '/', Op::Rem => '%' }
    }
}

#[derive(Clone, Copy)]
enum Btn {
    Digit(u8),
    Decimal,
    Op(Op),
    Equals,
    AllClear,
    ToggleSign,
    Backspace,
}

// Row-major order matching the 5×4 grid:
// Row 0: AC  +/-  BS   /
// Row 1:  7    8   9   *
// Row 2:  4    5   6   -
// Row 3:  1    2   3   +
// Row 4:  0    .   %   =
const BTNS: [(Btn, &str); 20] = [
    (Btn::AllClear,      "AC"),  (Btn::ToggleSign, "+/-"), (Btn::Backspace,   "BS"), (Btn::Op(Op::Div), "/"),
    (Btn::Digit(7),       "7"),  (Btn::Digit(8),    "8"),  (Btn::Digit(9),    "9"),  (Btn::Op(Op::Mul), "*"),
    (Btn::Digit(4),       "4"),  (Btn::Digit(5),    "5"),  (Btn::Digit(6),    "6"),  (Btn::Op(Op::Sub), "-"),
    (Btn::Digit(1),       "1"),  (Btn::Digit(2),    "2"),  (Btn::Digit(3),    "3"),  (Btn::Op(Op::Add), "+"),
    (Btn::Digit(0),       "0"),  (Btn::Decimal,     "."),  (Btn::Op(Op::Rem), "%"),  (Btn::Equals,      "="),
];

// ── Display formatting ────────────────────────────────────────────────────────

fn fmt(n: f64) -> String {
    if !n.is_finite() {
        return "Error".into();
    }
    // Format with 10 decimal places then strip trailing zeros.
    let s = format!("{:.10}", n);
    if s.contains('.') {
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    } else {
        s
    }
}

// ── Calculator state ──────────────────────────────────────────────────────────

pub struct Calculator {
    /// String shown in the large number display.
    display: String,
    /// Small line above the number: e.g. "3.14 *".
    context: String,
    /// Accumulated left-hand operand.
    acc: f64,
    /// Pending binary operator (None = no op waiting).
    pending: Option<Op>,
    /// If true, the next digit/decimal press starts a fresh entry.
    fresh: bool,
    /// Error state — div/mod by zero. Cleared by AllClear.
    error: bool,
    /// Index of the currently-pressed button (for visual inversion).
    pressed: Option<usize>,
    /// Pre-computed hit rectangles, in BTNS order.
    rects: [Rectangle; 20],
}

impl Calculator {
    pub const APP_ID: &'static str = "com.soulos.calculator";
    pub const NAME:   &'static str = "Calc";

    pub fn new() -> Self {
        let rects = Self::build_rects();
        Self {
            display: "0".into(),
            context: String::new(),
            acc: 0.0,
            pending: None,
            fresh: true,
            error: false,
            pressed: None,
            rects,
        }
    }

    pub fn persist(&mut self) {}

    // ── Layout ───────────────────────────────────────────────────────────────

    fn build_rects() -> [Rectangle; 20] {
        let mut rects = [Rectangle::zero(); 20];
        for row in 0..ROWS {
            for col in 0..COLS {
                let idx = (row * COLS + col) as usize;
                let x = col * (BTN_W + GAP);
                let y = BTN_TOP + row * (BTN_H + GAP);
                rects[idx] = Rectangle::new(Point::new(x, y), Size::new(BTN_W as u32, BTN_H as u32));
            }
        }
        rects
    }

    fn hit(&self, x: i16, y: i16) -> Option<usize> {
        self.rects.iter().position(|r| hit_test(r, x, y))
    }

    // ── Logic ────────────────────────────────────────────────────────────────

    fn do_op(&mut self) {
        if self.error { return; }
        let cur: f64 = self.display.parse().unwrap_or(0.0);
        match self.pending {
            None          => { self.acc = cur; }
            Some(Op::Add) => { self.acc += cur; }
            Some(Op::Sub) => { self.acc -= cur; }
            Some(Op::Mul) => { self.acc *= cur; }
            Some(Op::Div) => {
                if cur == 0.0 { self.set_error(); return; }
                self.acc /= cur;
            }
            Some(Op::Rem) => {
                if cur == 0.0 { self.set_error(); return; }
                self.acc %= cur;
            }
        }
        self.display = fmt(self.acc);
        self.fresh = true;
    }

    fn set_error(&mut self) {
        self.error = true;
        self.display = "Error".into();
        self.context = String::new();
        self.pending = None;
        self.fresh = true;
    }

    fn press(&mut self, btn: Btn) {
        match btn {
            Btn::AllClear => {
                self.display = "0".into();
                self.context = String::new();
                self.acc = 0.0;
                self.pending = None;
                self.fresh = true;
                self.error = false;
            }

            Btn::ToggleSign => {
                if self.error { return; }
                if self.display.starts_with('-') {
                    self.display = self.display[1..].to_string();
                } else if self.display != "0" {
                    self.display = format!("-{}", self.display);
                }
            }

            Btn::Backspace => {
                if self.error { self.press(Btn::AllClear); return; }
                if self.fresh {
                    self.display = "0".into();
                    self.fresh = false;
                    return;
                }
                if self.display.len() <= 1
                    || self.display == "-0"
                    || self.display == "-"
                {
                    self.display = "0".into();
                } else {
                    self.display.pop();
                    if self.display == "-" { self.display = "0".into(); }
                }
            }

            Btn::Digit(d) => {
                if self.error { return; }
                let s = d.to_string();
                if self.fresh {
                    self.display = s;
                    self.fresh = false;
                } else if self.display == "0" {
                    self.display = s;
                } else if self.display.trim_start_matches('-').len() < 12 {
                    self.display.push_str(&s);
                }
            }

            Btn::Decimal => {
                if self.error { return; }
                if self.fresh {
                    self.display = "0.".into();
                    self.fresh = false;
                } else if !self.display.contains('.') {
                    self.display.push('.');
                }
            }

            Btn::Op(op) => {
                if self.error { return; }
                self.do_op();
                if !self.error {
                    self.context = format!("{} {}", self.display, op.symbol());
                    self.pending = Some(op);
                }
            }

            Btn::Equals => {
                self.do_op();
                self.context = String::new();
                self.pending = None;
            }
        }
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    fn draw_display<D: DrawTarget<Color = Gray8>>(&self, canvas: &mut D) {
        // Context line (small, right-aligned) — e.g. "3.14 *"
        if !self.context.is_empty() {
            let small = MonoTextStyle::new(&FONT_6X10, BLACK);
            let cx = (W - 8 - self.context.chars().count() as i32 * 6).max(4);
            let _ = Text::with_baseline(&self.context, Point::new(cx, DISP_TOP + 4), small, Baseline::Top)
                .draw(canvas);
        }

        // Main number (large, right-aligned)
        let big = MonoTextStyle::new(&FONT_10X20, BLACK);
        let nw = self.display.chars().count() as i32 * 10;
        let nx = (W - 8 - nw).max(4);
        let _ = Text::with_baseline(&self.display, Point::new(nx, DISP_TOP + 16), big, Baseline::Top)
            .draw(canvas);

        // Separator line below display area
        let sep_y = BTN_TOP - 1;
        let _ = Line::new(Point::new(0, sep_y), Point::new(W, sep_y))
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas);
    }

    fn draw_button<D: DrawTarget<Color = Gray8>>(canvas: &mut D, rect: Rectangle, label: &str, pressed: bool) {
        let (fill, fg) = if pressed { (BLACK, WHITE) } else { (WHITE, BLACK) };
        let style = PrimitiveStyleBuilder::new()
            .fill_color(fill)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        let _ = RoundedRectangle::with_equal_corners(rect, Size::new(4, 4))
            .into_styled(style)
            .draw(canvas);
        let text_style = MonoTextStyle::new(&FONT_6X10, fg);
        let lw = label.chars().count() as i32 * 6;
        let lh = 10i32;
        let pos = rect.top_left
            + Point::new(
                (rect.size.width as i32 - lw) / 2,
                (rect.size.height as i32 - lh) / 2,
            );
        let _ = Text::with_baseline(label, pos, text_style, Baseline::Top).draw(canvas);
    }
}

// ── App trait ─────────────────────────────────────────────────────────────────

impl App for Calculator {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::AppStart => {
                ctx.invalidate_all();
            }

            Event::PenDown { x, y } => {
                if let Some(i) = self.hit(x, y) {
                    self.pressed = Some(i);
                    let (btn, _) = BTNS[i];
                    self.press(btn);
                    ctx.invalidate_all();
                }
            }

            Event::PenUp { .. } => {
                if self.pressed.take().is_some() {
                    ctx.invalidate_all();
                }
            }

            Event::Key(k) => {
                let btn = match k {
                    KeyCode::Char('0') => Some(Btn::Digit(0)),
                    KeyCode::Char('1') => Some(Btn::Digit(1)),
                    KeyCode::Char('2') => Some(Btn::Digit(2)),
                    KeyCode::Char('3') => Some(Btn::Digit(3)),
                    KeyCode::Char('4') => Some(Btn::Digit(4)),
                    KeyCode::Char('5') => Some(Btn::Digit(5)),
                    KeyCode::Char('6') => Some(Btn::Digit(6)),
                    KeyCode::Char('7') => Some(Btn::Digit(7)),
                    KeyCode::Char('8') => Some(Btn::Digit(8)),
                    KeyCode::Char('9') => Some(Btn::Digit(9)),
                    KeyCode::Char('.') | KeyCode::Char(',') => Some(Btn::Decimal),
                    KeyCode::Char('+') => Some(Btn::Op(Op::Add)),
                    KeyCode::Char('-') => Some(Btn::Op(Op::Sub)),
                    KeyCode::Char('*') => Some(Btn::Op(Op::Mul)),
                    KeyCode::Char('/') => Some(Btn::Op(Op::Div)),
                    KeyCode::Char('%') => Some(Btn::Op(Op::Rem)),
                    KeyCode::Enter     => Some(Btn::Equals),
                    KeyCode::Backspace => Some(Btn::Backspace),
                    _ => None,
                };
                if let Some(b) = btn {
                    self.press(b);
                    ctx.invalidate_all();
                }
            }

            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Clear background
        let bg = PrimitiveStyleBuilder::new().fill_color(WHITE).build();
        let _ = Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
            .into_styled(bg)
            .draw(canvas);

        // Title bar
        let bar_style = PrimitiveStyleBuilder::new().fill_color(BLACK).build();
        let _ = Rectangle::new(Point::zero(), Size::new(W as u32, TITLE_H as u32))
            .into_styled(bar_style)
            .draw(canvas);
        let title_style = MonoTextStyle::new(&FONT_6X10, WHITE);
        let _ = Text::with_baseline(Self::NAME, Point::new(4, 2), title_style, Baseline::Top)
            .draw(canvas);

        // Display area
        self.draw_display(canvas);

        // Buttons
        for (i, (_, label)) in BTNS.iter().enumerate() {
            let pressed = self.pressed == Some(i);
            Self::draw_button(canvas, self.rects[i], label, pressed);
        }
    }

    fn a11y_nodes(&self) -> Vec<A11yNode> {
        let mut nodes = Vec::with_capacity(BTNS.len() + 1);
        let display_rect = Rectangle::new(
            Point::new(0, DISP_TOP),
            Size::new(W as u32, DISP_H as u32),
        );
        nodes.push(
            A11yNode::new(display_rect, "Display", A11yRole::Label)
                .with_value(self.display.clone()),
        );
        for (i, (_, label)) in BTNS.iter().enumerate() {
            nodes.push(A11yNode::new(self.rects[i], *label, A11yRole::Button));
        }
        nodes
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> Calculator { Calculator::new() }

    fn digits(c: &mut Calculator, s: &str) {
        for ch in s.chars() {
            match ch {
                '0'..='9' => c.press(Btn::Digit(ch as u8 - b'0')),
                '.' => c.press(Btn::Decimal),
                _ => {}
            }
        }
    }

    #[test]
    fn initial_state() {
        let c = calc();
        assert_eq!(c.display, "0");
        assert!(!c.error);
    }

    #[test]
    fn integer_add() {
        let mut c = calc();
        digits(&mut c, "3");
        c.press(Btn::Op(Op::Add));
        digits(&mut c, "5");
        c.press(Btn::Equals);
        assert_eq!(c.display, "8");
    }

    #[test]
    fn decimal_add() {
        let mut c = calc();
        digits(&mut c, "0.1");
        c.press(Btn::Op(Op::Add));
        digits(&mut c, "0.2");
        c.press(Btn::Equals);
        // 0.1 + 0.2 with 10-decimal rounding should display as "0.3"
        assert_eq!(c.display, "0.3");
    }

    #[test]
    fn multiply_and_divide() {
        let mut c = calc();
        digits(&mut c, "6");
        c.press(Btn::Op(Op::Mul));
        digits(&mut c, "7");
        c.press(Btn::Equals);
        assert_eq!(c.display, "42");

        c.press(Btn::Op(Op::Div));
        digits(&mut c, "2");
        c.press(Btn::Equals);
        assert_eq!(c.display, "21");
    }

    #[test]
    fn chained_ops() {
        // 3 + 5 * 2 = (left-to-right): (3+5)*2 = 16
        let mut c = calc();
        digits(&mut c, "3");
        c.press(Btn::Op(Op::Add));
        digits(&mut c, "5");
        c.press(Btn::Op(Op::Mul));
        digits(&mut c, "2");
        c.press(Btn::Equals);
        assert_eq!(c.display, "16");
    }

    #[test]
    fn divide_by_zero_sets_error() {
        let mut c = calc();
        digits(&mut c, "5");
        c.press(Btn::Op(Op::Div));
        digits(&mut c, "0");
        c.press(Btn::Equals);
        assert!(c.error);
        assert_eq!(c.display, "Error");
    }

    #[test]
    fn all_clear_resets_error() {
        let mut c = calc();
        digits(&mut c, "1");
        c.press(Btn::Op(Op::Div));
        digits(&mut c, "0");
        c.press(Btn::Equals);
        assert!(c.error);
        c.press(Btn::AllClear);
        assert!(!c.error);
        assert_eq!(c.display, "0");
    }

    #[test]
    fn backspace_removes_last_digit() {
        let mut c = calc();
        digits(&mut c, "123");
        c.press(Btn::Backspace);
        assert_eq!(c.display, "12");
        c.press(Btn::Backspace);
        assert_eq!(c.display, "1");
        c.press(Btn::Backspace);
        assert_eq!(c.display, "0");
    }

    #[test]
    fn toggle_sign() {
        let mut c = calc();
        digits(&mut c, "5");
        c.press(Btn::ToggleSign);
        assert_eq!(c.display, "-5");
        c.press(Btn::ToggleSign);
        assert_eq!(c.display, "5");
    }

    #[test]
    fn decimal_only_once() {
        let mut c = calc();
        digits(&mut c, "3");
        c.press(Btn::Decimal);
        c.press(Btn::Decimal); // second press ignored
        digits(&mut c, "14");
        assert_eq!(c.display, "3.14");
    }

    #[test]
    fn fmt_strips_trailing_zeros() {
        assert_eq!(fmt(3.0),       "3");
        assert_eq!(fmt(3.14),      "3.14");
        assert_eq!(fmt(0.3),       "0.3");
        assert_eq!(fmt(-7.5),      "-7.5");
        assert_eq!(fmt(f64::INFINITY), "Error");
        assert_eq!(fmt(f64::NAN),      "Error");
    }

    #[test]
    fn a11y_nodes_include_display_with_live_value() {
        let mut c = calc();
        digits(&mut c, "42");
        let nodes = c.a11y_nodes();
        let display = nodes
            .iter()
            .find(|n| n.role == A11yRole::Label && n.label == "Display")
            .expect("Calculator should expose its display as an a11y node");
        assert_eq!(display.value.as_deref(), Some("42"));
    }

    #[test]
    fn a11y_nodes_include_every_button_as_button_role() {
        let c = calc();
        let nodes = c.a11y_nodes();
        let button_count = nodes
            .iter()
            .filter(|n| n.role == A11yRole::Button)
            .count();
        assert_eq!(button_count, BTNS.len(), "every keypad button must be exposed");
    }
}
