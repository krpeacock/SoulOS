//! Native Settings app: edit accessibility preferences (rate,
//! verbosity, punctuation, screen curtain).
//!
//! Architecturally the app mutates `ctx.a11y` for instant preview;
//! the runner Host watches for changes between frames and persists
//! the new values to `system_settings` (per-app override when this
//! app is open inside another scope, global otherwise — matching the
//! Phase 4 contract).

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Baseline, Text},
};
use soul_core::{
    a11y::{A11yNode, A11yRole, A11yState, Verbosity},
    App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH,
};
use soul_hal::{Punctuation, SpeechRequest};
use soul_ui::{hit_test, BLACK, WHITE};

const W: i32 = SCREEN_WIDTH as i32;
const H: i32 = APP_HEIGHT as i32;

const TITLE_H: i32 = 15;
const ROW_PAD: i32 = 6;
const STEP_W: i32 = 24;

/// Step granularity for the rate stepper. Coarse on purpose — the
/// 80–400 wpm range doesn't need 1-wpm precision and steppers are
/// faster to tap than a draggable slider on a stylus screen.
const RATE_STEP: u16 = 20;
const RATE_MIN: u16 = 80;
const RATE_MAX: u16 = 400;

/// Pure UI — no DB, no I/O. The runner Host owns persistence; this
/// app only mutates `ctx.a11y` and lets the Host detect & write.
pub struct Settings {
    /// Currently pressed control, for visual feedback. Cleared on PenUp.
    pressed: Option<Control>,
    /// Mirror of `A11yManager` values, refreshed at the tail of every
    /// `handle` call. `App::draw` and `App::a11y_nodes` get no `Ctx`,
    /// so the mirror is how those methods see live state.
    rate_wpm: u16,
    verbosity: Verbosity,
    punctuation: Punctuation,
    curtain: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    RateMinus,
    RatePlus,
    Verbosity(Verbosity),
    Punctuation(Punctuation),
    CurtainToggle,
    Reset,
}

impl Settings {
    pub const APP_ID: &'static str = "com.soulos.settings";
    pub const NAME: &'static str = "Settings";

    pub fn new() -> Self {
        Self {
            pressed: None,
            rate_wpm: SpeechRequest::DEFAULT_RATE_WPM,
            verbosity: Verbosity::Medium,
            punctuation: Punctuation::Some,
            curtain: false,
        }
    }

    fn refresh_mirror(&mut self, ctx: &Ctx<'_>) {
        self.rate_wpm = ctx.a11y.rate_wpm;
        self.verbosity = ctx.a11y.verbosity;
        self.punctuation = ctx.a11y.punctuation;
        self.curtain = ctx.a11y.screen_curtain;
    }

    // ── Layout ───────────────────────────────────────────────────────────────

    fn rate_label_pos() -> Point {
        Point::new(8, TITLE_H + 8)
    }
    fn rate_value_rect() -> Rectangle {
        Rectangle::new(
            Point::new(8 + 60, TITLE_H + 4),
            Size::new(60, 14),
        )
    }
    fn rate_minus_rect() -> Rectangle {
        Rectangle::new(
            Point::new(W - 8 - STEP_W * 2 - 4, TITLE_H + 4),
            Size::new(STEP_W as u32, 14),
        )
    }
    fn rate_plus_rect() -> Rectangle {
        Rectangle::new(
            Point::new(W - 8 - STEP_W, TITLE_H + 4),
            Size::new(STEP_W as u32, 14),
        )
    }

    fn verbosity_label_pos() -> Point {
        Point::new(8, TITLE_H + 24 + ROW_PAD)
    }
    fn verbosity_seg_rect(i: usize) -> Rectangle {
        let total_w = (W - 16) - 60;
        let seg_w = total_w / 3;
        Rectangle::new(
            Point::new(8 + 60 + i as i32 * seg_w, TITLE_H + 24 + 2),
            Size::new(seg_w as u32 - 2, 14),
        )
    }

    fn punct_label_pos() -> Point {
        Point::new(8, TITLE_H + 24 + 24 + ROW_PAD)
    }
    fn punct_seg_rect(i: usize) -> Rectangle {
        let total_w = (W - 16) - 60;
        let seg_w = total_w / 3;
        Rectangle::new(
            Point::new(8 + 60 + i as i32 * seg_w, TITLE_H + 48 + 2),
            Size::new(seg_w as u32 - 2, 14),
        )
    }

    fn curtain_label_pos() -> Point {
        Point::new(8, TITLE_H + 72 + ROW_PAD)
    }
    fn curtain_box_rect() -> Rectangle {
        Rectangle::new(
            Point::new(8 + 80, TITLE_H + 72 + 2),
            Size::new(14, 14),
        )
    }

    fn reset_rect() -> Rectangle {
        Rectangle::new(
            Point::new(W / 2 - 50, H - 24),
            Size::new(100, 18),
        )
    }

    fn hit(x: i16, y: i16) -> Option<Control> {
        if hit_test(&Self::rate_minus_rect(), x, y) {
            return Some(Control::RateMinus);
        }
        if hit_test(&Self::rate_plus_rect(), x, y) {
            return Some(Control::RatePlus);
        }
        for (i, v) in [Verbosity::Low, Verbosity::Medium, Verbosity::High]
            .into_iter()
            .enumerate()
        {
            if hit_test(&Self::verbosity_seg_rect(i), x, y) {
                return Some(Control::Verbosity(v));
            }
        }
        for (i, p) in [Punctuation::None, Punctuation::Some, Punctuation::All]
            .into_iter()
            .enumerate()
        {
            if hit_test(&Self::punct_seg_rect(i), x, y) {
                return Some(Control::Punctuation(p));
            }
        }
        if hit_test(&Self::curtain_box_rect(), x, y) {
            return Some(Control::CurtainToggle);
        }
        if hit_test(&Self::reset_rect(), x, y) {
            return Some(Control::Reset);
        }
        None
    }

    // ── Mutations ────────────────────────────────────────────────────────────

    fn apply(c: Control, ctx: &mut Ctx<'_>) {
        match c {
            Control::RateMinus => {
                let new = ctx.a11y.rate_wpm.saturating_sub(RATE_STEP);
                ctx.a11y.rate_wpm = new.max(RATE_MIN);
            }
            Control::RatePlus => {
                ctx.a11y.rate_wpm = (ctx.a11y.rate_wpm + RATE_STEP).min(RATE_MAX);
            }
            Control::Verbosity(v) => ctx.a11y.verbosity = v,
            Control::Punctuation(p) => ctx.a11y.punctuation = p,
            Control::CurtainToggle => {
                ctx.a11y.screen_curtain = !ctx.a11y.screen_curtain;
            }
            Control::Reset => {
                ctx.a11y.rate_wpm = SpeechRequest::DEFAULT_RATE_WPM;
                ctx.a11y.verbosity = Verbosity::Medium;
                ctx.a11y.punctuation = Punctuation::Some;
                ctx.a11y.screen_curtain = false;
            }
        }
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    fn draw_seg<D: DrawTarget<Color = Gray8>>(
        canvas: &mut D,
        rect: Rectangle,
        label: &str,
        active: bool,
        pressed: bool,
    ) {
        let (fill, fg) = if active || pressed {
            (BLACK, WHITE)
        } else {
            (WHITE, BLACK)
        };
        let style = PrimitiveStyleBuilder::new()
            .fill_color(fill)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        let _ = RoundedRectangle::with_equal_corners(rect, Size::new(3, 3))
            .into_styled(style)
            .draw(canvas);
        let text_style = MonoTextStyle::new(&FONT_6X10, fg);
        let lw = label.chars().count() as i32 * 6;
        let pos = rect.top_left
            + Point::new(
                (rect.size.width as i32 - lw) / 2,
                (rect.size.height as i32 - 10) / 2,
            );
        let _ = Text::with_baseline(label, pos, text_style, Baseline::Top).draw(canvas);
    }

    fn draw_label<D: DrawTarget<Color = Gray8>>(canvas: &mut D, at: Point, text: &str) {
        let style = MonoTextStyle::new(&FONT_6X10, BLACK);
        let _ = Text::with_baseline(text, at, style, Baseline::Top).draw(canvas);
    }
}

impl App for Settings {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::AppStart => {
                ctx.invalidate_all();
            }
            Event::PenDown { x, y } => {
                if let Some(c) = Self::hit(x, y) {
                    self.pressed = Some(c);
                    ctx.invalidate_all();
                }
            }
            Event::PenUp { x, y } => {
                let was = self.pressed.take();
                if let Some(c) = was {
                    if let Some(hit) = Self::hit(x, y) {
                        if hit == c {
                            Self::apply(c, ctx);
                        }
                    }
                    ctx.invalidate_all();
                }
            }
            _ => {}
        }
        // Always refresh the local mirror so subsequent draw/
        // a11y_nodes calls see the live A11yManager values — even
        // ones mutated outside this app (long-press Power for the
        // curtain, future remote sync, etc).
        self.refresh_mirror(ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Title bar.
        let _ = Rectangle::new(Point::zero(), Size::new(W as u32, TITLE_H as u32))
            .into_styled(PrimitiveStyle::with_fill(BLACK))
            .draw(canvas);
        let title_style = MonoTextStyle::new(&FONT_6X10, WHITE);
        let _ = Text::with_baseline(
            Self::NAME,
            Point::new(4, 2),
            title_style,
            Baseline::Top,
        )
        .draw(canvas);

        // Rate row.
        Self::draw_label(canvas, Self::rate_label_pos(), "Rate:");
        let value_text = format!("{} wpm", self.rate_wpm);
        let _ = Text::with_baseline(
            &value_text,
            Self::rate_value_rect().top_left + Point::new(0, 2),
            MonoTextStyle::new(&FONT_6X10, BLACK),
            Baseline::Top,
        )
        .draw(canvas);
        Self::draw_seg(
            canvas,
            Self::rate_minus_rect(),
            "-",
            false,
            self.pressed == Some(Control::RateMinus),
        );
        Self::draw_seg(
            canvas,
            Self::rate_plus_rect(),
            "+",
            false,
            self.pressed == Some(Control::RatePlus),
        );

        // Verbosity row.
        Self::draw_label(canvas, Self::verbosity_label_pos(), "Verbose:");
        for (i, (v, lbl)) in [
            (Verbosity::Low, "Low"),
            (Verbosity::Medium, "Med"),
            (Verbosity::High, "High"),
        ]
        .iter()
        .enumerate()
        {
            Self::draw_seg(
                canvas,
                Self::verbosity_seg_rect(i),
                lbl,
                *v == self.verbosity,
                self.pressed == Some(Control::Verbosity(*v)),
            );
        }

        // Punctuation row.
        Self::draw_label(canvas, Self::punct_label_pos(), "Punct:");
        for (i, (p, lbl)) in [
            (Punctuation::None, "None"),
            (Punctuation::Some, "Some"),
            (Punctuation::All, "All"),
        ]
        .iter()
        .enumerate()
        {
            Self::draw_seg(
                canvas,
                Self::punct_seg_rect(i),
                lbl,
                *p == self.punctuation,
                self.pressed == Some(Control::Punctuation(*p)),
            );
        }

        // Curtain row.
        Self::draw_label(canvas, Self::curtain_label_pos(), "Curtain:");
        let cb = Self::curtain_box_rect();
        let _ = cb
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas);
        if self.curtain {
            // Filled square inside the box.
            let inner = Rectangle::new(
                cb.top_left + Point::new(3, 3),
                Size::new(cb.size.width - 6, cb.size.height - 6),
            );
            let _ = inner
                .into_styled(PrimitiveStyle::with_fill(BLACK))
                .draw(canvas);
        }
        let state_text = if self.curtain { "On" } else { "Off" };
        let _ = Text::with_baseline(
            state_text,
            cb.top_left + Point::new(20, 2),
            MonoTextStyle::new(&FONT_6X10, BLACK),
            Baseline::Top,
        )
        .draw(canvas);

        // Reset button.
        Self::draw_seg(
            canvas,
            Self::reset_rect(),
            "Reset",
            false,
            self.pressed == Some(Control::Reset),
        );
    }

    fn a11y_nodes(&self) -> Vec<A11yNode> {
        let mut nodes = Vec::with_capacity(11);
        nodes.push(
            A11yNode::new(Self::rate_value_rect(), "Rate", A11yRole::Slider)
                .with_value(format!("{} wpm", self.rate_wpm))
                .with_hint("Adjust speech rate; tap minus or plus to change"),
        );
        nodes.push(
            A11yNode::new(Self::rate_minus_rect(), "Rate down", A11yRole::Button)
                .with_hint("Decrease speech rate by 20 wpm"),
        );
        nodes.push(
            A11yNode::new(Self::rate_plus_rect(), "Rate up", A11yRole::Button)
                .with_hint("Increase speech rate by 20 wpm"),
        );
        for (i, (v, lbl)) in [
            (Verbosity::Low, "Verbose Low"),
            (Verbosity::Medium, "Verbose Medium"),
            (Verbosity::High, "Verbose High"),
        ]
        .iter()
        .enumerate()
        {
            let mut node =
                A11yNode::new(Self::verbosity_seg_rect(i), *lbl, A11yRole::Button);
            node.state.selected = *v == self.verbosity;
            nodes.push(node);
        }
        for (i, (p, lbl)) in [
            (Punctuation::None, "Punctuation None"),
            (Punctuation::Some, "Punctuation Some"),
            (Punctuation::All, "Punctuation All"),
        ]
        .iter()
        .enumerate()
        {
            let mut node =
                A11yNode::new(Self::punct_seg_rect(i), *lbl, A11yRole::Button);
            node.state.selected = *p == self.punctuation;
            nodes.push(node);
        }
        nodes.push(
            A11yNode::new(Self::curtain_box_rect(), "Screen curtain", A11yRole::Checkbox)
                .with_state(A11yState::checked(self.curtain)),
        );
        nodes.push(
            A11yNode::new(Self::reset_rect(), "Reset to defaults", A11yRole::Button)
                .with_hint("Restore rate, verbosity, punctuation, and curtain"),
        );
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soul_core::{a11y::A11yManager, Dirty};

    fn with_ctx(a11y: &mut A11yManager, f: impl FnOnce(&mut Ctx<'_>)) {
        let mut dirty = Dirty::default();
        let mut ctx = Ctx {
            now_ms: 0,
            dirty: &mut dirty,
            a11y,
        };
        f(&mut ctx);
    }

    #[test]
    fn rate_plus_minus_steps_by_twenty_within_bounds() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 200;
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::RatePlus, ctx));
        assert_eq!(a11y.rate_wpm, 220);
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::RateMinus, ctx));
        assert_eq!(a11y.rate_wpm, 200);
    }

    #[test]
    fn rate_clamps_to_min_max() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = RATE_MIN;
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::RateMinus, ctx));
        assert_eq!(a11y.rate_wpm, RATE_MIN);
        a11y.rate_wpm = RATE_MAX;
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::RatePlus, ctx));
        assert_eq!(a11y.rate_wpm, RATE_MAX);
    }

    #[test]
    fn verbosity_segments_set_value() {
        let mut a11y = A11yManager::new();
        with_ctx(&mut a11y, |ctx| {
            Settings::apply(Control::Verbosity(Verbosity::High), ctx)
        });
        assert_eq!(a11y.verbosity, Verbosity::High);
    }

    #[test]
    fn punctuation_segments_set_value() {
        let mut a11y = A11yManager::new();
        with_ctx(&mut a11y, |ctx| {
            Settings::apply(Control::Punctuation(Punctuation::All), ctx)
        });
        assert_eq!(a11y.punctuation, Punctuation::All);
    }

    #[test]
    fn curtain_toggle_flips() {
        let mut a11y = A11yManager::new();
        assert!(!a11y.screen_curtain);
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::CurtainToggle, ctx));
        assert!(a11y.screen_curtain);
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::CurtainToggle, ctx));
        assert!(!a11y.screen_curtain);
    }

    #[test]
    fn reset_restores_defaults() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 320;
        a11y.verbosity = Verbosity::High;
        a11y.punctuation = Punctuation::All;
        a11y.screen_curtain = true;
        with_ctx(&mut a11y, |ctx| Settings::apply(Control::Reset, ctx));
        assert_eq!(a11y.rate_wpm, SpeechRequest::DEFAULT_RATE_WPM);
        assert_eq!(a11y.verbosity, Verbosity::Medium);
        assert_eq!(a11y.punctuation, Punctuation::Some);
        assert!(!a11y.screen_curtain);
    }

    #[test]
    fn pen_tap_on_minus_then_release_decreases_rate() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 240;
        let mut s = Settings::new();
        let r = Settings::rate_minus_rect();
        let cx = (r.top_left.x + (r.size.width as i32) / 2) as i16;
        let cy = (r.top_left.y + (r.size.height as i32) / 2) as i16;
        with_ctx(&mut a11y, |ctx| {
            s.handle(Event::PenDown { x: cx, y: cy }, ctx);
            s.handle(Event::PenUp { x: cx, y: cy }, ctx);
        });
        assert_eq!(a11y.rate_wpm, 220);
    }

    #[test]
    fn pen_release_outside_press_target_does_not_apply() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 240;
        let mut s = Settings::new();
        let r = Settings::rate_minus_rect();
        let cx = (r.top_left.x + (r.size.width as i32) / 2) as i16;
        let cy = (r.top_left.y + (r.size.height as i32) / 2) as i16;
        with_ctx(&mut a11y, |ctx| {
            s.handle(Event::PenDown { x: cx, y: cy }, ctx);
            // Drift offscreen before lifting — the gesture is cancelled.
            s.handle(Event::PenUp { x: -1, y: -1 }, ctx);
        });
        assert_eq!(a11y.rate_wpm, 240);
    }

    #[test]
    fn handle_refreshes_mirror_from_ctx() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 280;
        a11y.verbosity = Verbosity::High;
        a11y.punctuation = Punctuation::All;
        a11y.screen_curtain = true;
        let mut s = Settings::new();
        with_ctx(&mut a11y, |ctx| s.handle(Event::AppStart, ctx));
        assert_eq!(s.rate_wpm, 280);
        assert_eq!(s.verbosity, Verbosity::High);
        assert_eq!(s.punctuation, Punctuation::All);
        assert!(s.curtain);
    }

    #[test]
    fn a11y_tree_shape_matches_controls() {
        let mut a11y = A11yManager::new();
        a11y.rate_wpm = 220;
        a11y.verbosity = Verbosity::High;
        a11y.punctuation = Punctuation::Some;
        a11y.screen_curtain = true;
        let mut s = Settings::new();
        with_ctx(&mut a11y, |ctx| s.handle(Event::AppStart, ctx));

        let nodes = s.a11y_nodes();
        // 1 slider + 2 stepper buttons + 3 verbosity + 3 punctuation
        // + 1 checkbox + 1 reset = 11.
        assert_eq!(nodes.len(), 11);
        assert_eq!(nodes[0].role, A11yRole::Slider);
        assert_eq!(nodes[0].value.as_deref(), Some("220 wpm"));
        // Verbosity::High is selected; the others are not.
        let selected_high = nodes
            .iter()
            .find(|n| n.label == "Verbose High")
            .unwrap()
            .state
            .selected;
        assert!(selected_high);
        // Curtain checkbox state mirrors the snapshot.
        let curtain = nodes
            .iter()
            .find(|n| n.role == A11yRole::Checkbox)
            .unwrap();
        assert_eq!(curtain.state.checked, Some(true));
    }
}
