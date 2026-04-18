//! # soul-core — runtime SDK for SoulOS applications
//!
//! The [`App`] trait, the event loop ([`run`]), and everything an
//! app sees of the world while it's running: events
//! ([`Event`]), the per-frame context ([`Ctx`]), and virtual-screen
//! constants ([`SCREEN_WIDTH`], [`SCREEN_HEIGHT`]).
//!
//! This crate is `no_std + alloc` and depends only on
//! [`soul-hal`](../soul_hal/index.html) for the platform trait. It
//! compiles for both the hosted desktop simulator and bare-metal
//! targets with no code changes.
//!
//! # Writing a SoulOS app
//!
//! An app is any type that implements [`App`]. The runtime owns
//! your app, drives its event loop, and renders it into the
//! platform's display. You:
//!
//! - implement [`App::handle`] to react to events (stylus,
//!   keyboard, hard buttons, ticks). When app state that affects
//!   rendering changes, call [`Ctx::invalidate`] with the screen
//!   rectangle that needs to be repainted.
//! - implement [`App::draw`] to paint the current state into a
//!   generic [`DrawTarget`]. The runtime clips to the dirty region
//!   before calling you, so drawing the whole scene on every frame
//!   is cheap.
//!
//! ```ignore
//! use embedded_graphics::{
//!     prelude::*, pixelcolor::Gray8, draw_target::DrawTarget,
//!     primitives::Rectangle,
//! };
//! use soul_core::{App, Ctx, Event, run, SCREEN_WIDTH};
//! use soul_ui::title_bar;
//!
//! struct Hello;
//!
//! impl App for Hello {
//!     fn handle(&mut self, _event: Event, _ctx: &mut Ctx<'_>) {}
//!     fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
//!         let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Hello");
//!     }
//! }
//!
//! fn start<P: soul_hal::Platform>(platform: &mut P) {
//!     run(platform, Hello);
//! }
//! ```
//!
//! # Dirty-rect tracking (required)
//!
//! SoulOS targets e-ink, where a full-screen refresh takes hundreds
//! of milliseconds and visibly flashes. The runtime clips rendering
//! to the union of rectangles passed to [`Ctx::invalidate`] during
//! `handle`, so **only repaint what changed**: call `invalidate`
//! with the exact rectangle whose pixels differ from the previous
//! frame. When nothing changes, no repaint happens and no flush is
//! sent to the panel. See the project `CLAUDE.md` for the
//! philosophy.
//!
//! [`DrawTarget`]: embedded_graphics::draw_target::DrawTarget

#![no_std]
extern crate alloc;

use embedded_graphics::{
    draw_target::{DrawTarget, DrawTargetExt},
    pixelcolor::{Gray8, GrayColor},
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

pub use soul_hal::{HardButton, KeyCode};
use soul_hal::{InputEvent, Platform};

/// Width of the SoulOS virtual screen in pixels.
///
/// Every SoulOS app must render correctly inside this width. HAL
/// implementations may upscale or center this virtual surface on a
/// larger physical panel.
pub const SCREEN_WIDTH: u16 = 240;

/// Height of the SoulOS virtual screen in pixels. See
/// [`SCREEN_WIDTH`] for rationale.
pub const SCREEN_HEIGHT: u16 = 320;

/// Height of the system strip reserved along the bottom of every
/// screen for system affordances (Home, Menu, status).
///
/// The shell draws the strip; apps never render into it. See
/// [`APP_HEIGHT`] for the usable region.
pub const SYSTEM_STRIP_H: u16 = 16;

/// Height of the screen region available to apps. Apps must lay
/// out their content inside `(0, 0) – (SCREEN_WIDTH, APP_HEIGHT)`;
/// the shell reserves `SCREEN_HEIGHT – APP_HEIGHT` pixels at the
/// bottom for the system strip.
pub const APP_HEIGHT: u16 = SCREEN_HEIGHT - SYSTEM_STRIP_H;

/// An event delivered to [`App::handle`].
///
/// Events are strictly in-order and delivered one at a time; the
/// runtime never concurrently invokes `handle`. Handlers should
/// update internal state and, if rendering changed, call
/// [`Ctx::invalidate`] with the affected screen rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Fired once, before the first `draw`, when the runtime begins
    /// running this app.
    AppStart,
    /// Fired once when the runtime is shutting this app down (e.g.,
    /// the platform requested quit, or the runtime is switching to
    /// another app). Persist state here.
    AppStop,
    /// Stylus/finger pressed the screen at `(x, y)` in virtual-screen
    /// coordinates.
    PenDown { x: i16, y: i16 },
    /// Stylus/finger is being dragged; last known coordinate.
    PenMove { x: i16, y: i16 },
    /// Stylus/finger lifted; `(x, y)` is the final coordinate.
    PenUp { x: i16, y: i16 },
    /// A hard button (e.g., [`HardButton::Home`]) was pressed.
    ButtonDown(HardButton),
    /// A hard button was released.
    ButtonUp(HardButton),
    /// A keyboard key was pressed (physical keyboard or an SDL/host
    /// equivalent). Repeats on hold.
    Key(KeyCode),
    /// Periodic tick carrying the platform clock in milliseconds.
    /// Sent once per frame after all input events have been
    /// dispatched. Useful for animations and timers.
    Tick(u64),
    /// Application menu (e.g. Palm-style silk **Menu** or the
    /// hard [`HardButton::Menu`] key). Delivered once per activation,
    /// not paired with a release.
    Menu,
}

/// Dirty-region accumulator used by the runtime.
///
/// Apps never construct this directly; they invalidate through
/// [`Ctx::invalidate`]. Held publicly so custom runtimes (tests,
/// alternative platforms) can drive the event loop themselves.
#[derive(Debug, Default)]
pub struct Dirty {
    region: Option<Rectangle>,
}

impl Dirty {
    /// Start with a full-screen dirty region. The runtime uses this
    /// on the first frame and after an app switch to force a
    /// complete repaint.
    pub fn full() -> Self {
        Self {
            region: Some(full_screen()),
        }
    }

    /// Union `rect` into the current dirty region. Zero-area
    /// rectangles are ignored.
    pub fn add(&mut self, rect: Rectangle) {
        if rect.size.width == 0 || rect.size.height == 0 {
            return;
        }
        self.region = Some(match self.region {
            Some(existing) => union(existing, rect),
            None => rect,
        });
    }

    /// Mark the entire screen dirty. Expensive on e-ink — use
    /// sparingly, typically only on app activation or layout
    /// changes.
    pub fn add_all(&mut self) {
        self.region = Some(full_screen());
    }

    /// Consume and return the current dirty region.
    pub fn take(&mut self) -> Option<Rectangle> {
        self.region.take()
    }
}

fn full_screen() -> Rectangle {
    Rectangle::new(
        Point::zero(),
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
    )
}

fn union(a: Rectangle, b: Rectangle) -> Rectangle {
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

pub mod a11y;

use a11y::A11yManager;

/// The per-event context passed to [`App::handle`].
///
/// Holds the platform time and a handle for the dirty-region
/// accumulator. Apps call [`Ctx::invalidate`] to request a repaint
/// of a specific rectangle.
pub struct Ctx<'a> {
    /// Milliseconds since the platform started (monotonic).
    pub now_ms: u64,
    pub dirty: &'a mut Dirty,
    pub a11y: &'a mut A11yManager,
}

impl<'a> Ctx<'a> {
    /// Mark `rect` as needing repaint this frame.
    ///
    /// `rect` is in virtual-screen coordinates. The runtime unions
    /// all invalidated rectangles and clips the next `draw` call to
    /// the resulting bounding box.
    pub fn invalidate(&mut self, rect: Rectangle) {
        self.dirty.add(rect);
    }

    /// Mark the entire screen as dirty. Use when a layout change
    /// makes per-rectangle tracking impractical (app switch, theme
    /// change). Avoid in steady-state rendering.
    pub fn invalidate_all(&mut self) {
        self.dirty.add_all();
    }
}

/// Trait implemented by SoulOS apps.
///
/// An app is *state plus a handler plus a renderer*. The runtime
/// owns the app by value for its lifetime; there's no heap
/// indirection, no dynamic dispatch, and no hidden threading.
/// Cooperative scheduling is enforced structurally: when your
/// `handle` returns, the runtime is free to do other work.
pub trait App {
    /// React to a single event. Update internal state and call
    /// `ctx.invalidate` for any rectangle whose pixels changed.
    ///
    /// Must return quickly. For long-running work, split it across
    /// `Tick` events or pump a state machine; never block.
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>);

    /// Paint the current app state into `canvas`.
    ///
    /// The runtime only calls this when there's a non-empty dirty
    /// region, and `canvas` is a clipped view of the real display:
    /// draws outside the dirty region are discarded. You can and
    /// should draw the entire scene unconditionally — the clipper
    /// makes this cheap.
    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>;

    /// Return a list of accessible nodes for the current state.
    fn a11y_nodes(&self) -> alloc::vec::Vec<a11y::A11yNode> {
        alloc::vec::Vec::new()
    }
}

fn translate(input: InputEvent) -> Option<Event> {
    match input {
        InputEvent::StylusDown { x, y } => Some(Event::PenDown { x, y }),
        InputEvent::StylusMove { x, y } => Some(Event::PenMove { x, y }),
        InputEvent::StylusUp { x, y } => Some(Event::PenUp { x, y }),
        InputEvent::ButtonDown(HardButton::Menu) => Some(Event::Menu),
        InputEvent::ButtonUp(HardButton::Menu) => None,
        InputEvent::ButtonDown(b) => Some(Event::ButtonDown(b)),
        InputEvent::ButtonUp(b) => Some(Event::ButtonUp(b)),
        InputEvent::Key(k) => Some(Event::Key(k)),
        InputEvent::Quit => None,
    }
}

/// Run `app` on `platform` until the platform emits a quit event.
///
/// This is the canonical SoulOS event loop:
///
/// 1. Deliver [`Event::AppStart`], then enter the main loop.
/// 2. Drain all pending input events through [`App::handle`].
/// 3. Deliver a [`Event::Tick`] with the current monotonic time.
/// 4. If any handler called [`Ctx::invalidate`], clip the display
///    to the dirty region, fill it with [`Gray8::WHITE`], and call
///    [`App::draw`].
/// 5. Flush the platform (present frame, pump events) and sleep
///    for ~16 ms.
/// 6. On quit, deliver [`Event::AppStop`] and return.
///
/// Apps never call `run` themselves — the `soul-runner` binary
/// does. This function is public so alternative platforms (tests,
/// bare-metal bootloaders) can embed it.
pub fn run<P: Platform, A: App>(platform: &mut P, mut app: A) {
    let mut dirty = Dirty::full();
    let mut a11y = A11yManager::new();
    {
        let now = platform.now_ms();
        let mut ctx = Ctx {
            now_ms: now,
            dirty: &mut dirty,
            a11y: &mut a11y,
        };
        app.handle(Event::AppStart, &mut ctx);
    }
    loop {
        while let Some(ev) = platform.poll_event() {
            if matches!(ev, InputEvent::Quit) {
                let now = platform.now_ms();
                let mut ctx = Ctx {
                    now_ms: now,
                    dirty: &mut dirty,
                    a11y: &mut a11y,
                };
                app.handle(Event::AppStop, &mut ctx);
                return;
            }
            if let Some(e) = translate(ev) {
                let now = platform.now_ms();
                let mut ctx = Ctx {
                    now_ms: now,
                    dirty: &mut dirty,
                    a11y: &mut a11y,
                };
                app.handle(e, &mut ctx);
            }
        }
        {
            let now = platform.now_ms();
            let mut ctx = Ctx {
                now_ms: now,
                dirty: &mut dirty,
                a11y: &mut a11y,
            };
            app.handle(Event::Tick(now), &mut ctx);
        }
        if let Some(rect) = dirty.take() {
            let mut clip = platform.display().clipped(&rect);
            let _ = rect
                .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                .draw(&mut clip);
            app.draw(&mut clip);
        }

        // Drain accessibility speech
        for text in a11y.pending_speech.drain(..) {
            platform.speak(&text);
        }

        platform.flush();
        platform.sleep_ms(16);
    }
}
