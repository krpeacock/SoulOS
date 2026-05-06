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
    pixelcolor::Gray8,
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

/// Data carried by an [`Event::Exchange`] delivery.
///
/// The kernel routes exchange payloads between apps without either
/// side knowing the other's internal structure. New payload kinds
/// can be added here without changing the exchange protocol.
#[derive(Debug, Clone)]
pub enum ExchangePayload {
    /// Raw grayscale bitmap. `pixels.len() == width as usize * height as usize`.
    Bitmap {
        width: u16,
        height: u16,
        pixels: alloc::vec::Vec<u8>,
    },
    /// Plain text — a script, a note, a template.
    Text(alloc::string::String),
    /// A named resource belonging to a specific app — used for kernel-mediated
    /// resource get/set without launching the owning app's UI.
    ///
    /// `app_id` identifies the owning app. `kind` names the resource type
    /// ("icon", "script", "form", …). For get requests `pixels` and `text`
    /// are empty; for set/return they carry the resource data.
    Resource {
        app_id: alloc::string::String,
        kind: alloc::string::String,
        width: u16,
        height: u16,
        pixels: alloc::vec::Vec<u8>,
        text: alloc::string::String,
    },
}

/// An event delivered to [`App::handle`].
///
/// Events are strictly in-order and delivered one at a time; the
/// runtime never concurrently invokes `handle`. Handlers should
/// update internal state and, if rendering changed, call
/// [`Ctx::invalidate`] with the affected screen rectangle.
///
/// `Event` is `Clone` but not `Copy` because [`Event::Exchange`]
/// carries heap-allocated payload data.
#[derive(Debug, Clone)]
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
    /// Scroll-wheel or two-finger swipe. Pixel-equivalent deltas;
    /// positive `dy` scrolls down (reveals content below).
    Wheel { dx: i16, dy: i16 },
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
    /// The kernel is delivering an exchange payload to this app.
    ///
    /// Fired when another app (or the system) called `system_send` or
    /// when this app's `system_request` was fulfilled. `action` names
    /// what kind of data is arriving; `sender` is the originating
    /// app's ID. The app should inspect `action` and handle `payload`
    /// accordingly, then call `system_return` when done.
    Exchange {
        action: alloc::string::String,
        payload: ExchangePayload,
        sender: alloc::string::String,
    },
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
    /// Only called when there is a non-empty dirty region.  `canvas`
    /// is already clipped to `dirty` — writes outside it are
    /// discarded by the hardware/simulator.
    ///
    /// **Do not draw unconditionally.**  Use `dirty` to restrict your
    /// iteration to only the changed region.  On SoulOS every wasted
    /// pixel write costs real cycles on slow hardware.
    fn draw<D>(&mut self, canvas: &mut D, dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>;

    /// Return a list of accessible nodes for the current state.
    fn a11y_nodes(&self) -> alloc::vec::Vec<a11y::A11yNode> {
        alloc::vec::Vec::new()
    }

    /// Restrict focus traversal to a sub-rectangle of the screen.
    ///
    /// Apps drawing a modal (date picker, confirm-delete, item
    /// chooser) return that modal's bounds so the [`a11y::FocusRing`]
    /// can filter background nodes out of focus traversal. Apps that
    /// have no modal — the default — return `None`.
    fn a11y_focus_scope(&self) -> Option<Rectangle> {
        None
    }
}

fn translate(input: InputEvent) -> Option<Event> {
    match input {
        InputEvent::StylusDown { x, y } => Some(Event::PenDown { x, y }),
        InputEvent::StylusMove { x, y } => Some(Event::PenMove { x, y }),
        InputEvent::StylusUp { x, y } => Some(Event::PenUp { x, y }),
        InputEvent::Wheel { dx, dy } => Some(Event::Wheel { dx, dy }),
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
///    to the dirty region and call [`App::draw`] with that rect.
///    The app owns its own background — no blanket white fill.
/// 5. Flush the platform (present frame, pump new events).
/// 6. Sleep for the remainder of a 16 ms budget (adaptive).
/// 7. On quit, deliver [`Event::AppStop`] and return.
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
        let frame_start = platform.now_ms();

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

        // Rebuild the focus ring once per frame from the app's current
        // a11y tree and focus scope. The ring's signature gate makes
        // this cheap when the tree hasn't changed shape.
        if a11y.enabled {
            let scope = match app.a11y_focus_scope() {
                Some(rect) => a11y::FocusScope::Modal { rect },
                None => a11y::FocusScope::Whole,
            };
            a11y.focus.rebuild(app.a11y_nodes(), scope);
        }

        if let Some(rect) = dirty.take() {
            let mut clip = platform.display().clipped(&rect);
            // Clear only the dirty region to white before drawing.
            // This is bounded to the invalidated rect, not the full screen.
            let _ = rect
                .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                .draw(&mut clip);
            app.draw(&mut clip, rect);
        }

        // Drain accessibility speech
        for text in a11y.pending_speech.drain(..) {
            platform.speak(&text);
        }

        // Flush (present frame + pump new events into the HAL queue).
        platform.flush();

        // Adaptive sleep: hold ~16 ms per frame, accounting for work done.
        let elapsed = platform.now_ms().saturating_sub(frame_start);
        let budget: u64 = 16;
        if elapsed < budget {
            platform.sleep_ms((budget - elapsed) as u32);
        }
    }
}
