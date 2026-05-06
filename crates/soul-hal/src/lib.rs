//! # soul-hal — platform abstraction layer for SoulOS
//!
//! Describes the minimum a platform implementation must provide so
//! the same app code runs in the hosted desktop simulator, on an
//! e-ink reader, or on an old ARM phone.
//!
//! This crate is the boundary between the portable SoulOS runtime
//! and any concrete hardware (or host OS pretending to be hardware).
//! It's `no_std` by design — **never** leak `std` types across this
//! line, or bare-metal targets will stop compiling.
//!
//! # Platform contract
//!
//! To run SoulOS on a new target, implement [`Platform`]. Rendering
//! is delegated to [`embedded_graphics::draw_target::DrawTarget`],
//! so any panel driver in the `embedded-graphics` ecosystem
//! (EPD27, SSD1306, ILI9341, …) plugs in with minimal glue.
//!
//! Input arrives via [`Platform::poll_event`] as a stream of
//! [`InputEvent`]s. The runtime drains these each frame and
//! dispatches them to the active app.
//!
//! ```ignore
//! use soul_hal::{Platform, InputEvent};
//!
//! struct MyBoard { /* framebuffer, input pins, rtc */ }
//!
//! impl Platform for MyBoard {
//!     type Display = MyFramebuffer;
//!     fn display(&mut self) -> &mut Self::Display { /* ... */ }
//!     fn poll_event(&mut self) -> Option<InputEvent> { /* ... */ }
//!     fn now_ms(&self) -> u64 { /* ... */ }
//!     fn flush(&mut self) { /* ... */ }
//!     fn sleep_ms(&mut self, ms: u32) { /* ... */ }
//! }
//! ```

#![no_std]

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Gray8};

/// Canonical SoulOS pixel color. All HALs expose a display whose
/// pixel type is [`Gray8`]; concrete panels may dither or upsample
/// internally.
pub type Color = Gray8;

/// The contract a target must satisfy to host SoulOS.
///
/// One foreground app at a time, cooperative scheduling, and a
/// single display surface. Methods are called in the order:
/// `poll_event*` → `now_ms` → `display` → `flush` → `sleep_ms` per
/// frame.
pub trait Platform {
    /// The display type. Must implement [`DrawTarget`] over
    /// [`Color`] so any `embedded-graphics` primitive can be drawn.
    type Display: DrawTarget<Color = Color>;

    /// Mutably borrow the framebuffer. The runtime calls this each
    /// frame to render into it, then hands it back before calling
    /// [`Self::flush`].
    fn display(&mut self) -> &mut Self::Display;

    /// Return the next queued input event, or `None` if the queue
    /// is empty. The runtime drains this to exhaustion each frame.
    fn poll_event(&mut self) -> Option<InputEvent>;

    /// Milliseconds since platform start. Must be monotonic; does
    /// not need to represent wall-clock time.
    fn now_ms(&self) -> u64;

    /// Present the current framebuffer to the user and pump the
    /// platform's input queue. Called once per frame after `draw`.
    fn flush(&mut self);

    /// Sleep for at least `ms` milliseconds. On embedded targets,
    /// this is typically a WFI / low-power wait.
    fn sleep_ms(&mut self, ms: u32);

    /// Speak a structured TTS request.
    ///
    /// Implementations are expected to honor `interrupt`: when `true`
    /// the platform must stop any utterance currently in flight before
    /// starting this one, so a screen reader user advancing focus
    /// rapidly never falls behind. Implementations that cannot
    /// natively interrupt should still drop the request rather than
    /// queue it.
    fn speak(&mut self, req: SpeechRequest<'_>);

    /// Show or hide the screen curtain.
    ///
    /// On e-ink targets the curtain suppresses panel flushes — the
    /// big win is not privacy but eliminating the 300–900 ms refresh
    /// flash on every focus step. The default is a no-op so the trait
    /// stays cheap to implement on platforms where curtain has no
    /// meaningful behavior. Phase 3b wires hosted/web implementations.
    fn set_screen_curtain(&mut self, _on: bool) {}
}

/// How aggressively a TTS engine should pronounce punctuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Punctuation {
    /// Skip almost all punctuation. Quiet, fast.
    None,
    /// Read a useful subset (commas, periods, colons). Default.
    #[default]
    Some,
    /// Read every punctuation symbol literally — useful when editing
    /// code, math, or addresses.
    All,
}

/// A structured TTS request handed to [`Platform::speak`].
///
/// Lives in `soul-hal` so the no_std core can construct one without
/// pulling in `String`. `text` is borrowed for the duration of the
/// call; the implementation must not retain it past return.
#[derive(Debug, Clone, Copy)]
pub struct SpeechRequest<'a> {
    /// Utterance to speak. Caller-owned; the impl must not retain.
    pub text: &'a str,
    /// Words per minute. Implementations clamp to whatever range the
    /// underlying engine supports; values outside ~80..=400 are
    /// generally clamped to the engine's bounds.
    pub rate_wpm: u16,
    /// When true, abort any currently-speaking utterance before
    /// starting this one. Screen readers want this on for every
    /// new utterance triggered by user navigation.
    pub interrupt: bool,
    /// Punctuation verbosity.
    pub punctuation: Punctuation,
}

impl<'a> SpeechRequest<'a> {
    /// Default rate. Roughly the macOS `say` default (~175 wpm) and a
    /// reasonable middle ground between TalkBack/VoiceOver presets.
    pub const DEFAULT_RATE_WPM: u16 = 175;

    /// Construct a request with sensible defaults: default rate,
    /// `interrupt = true`, `Punctuation::Some`.
    pub const fn new(text: &'a str) -> Self {
        Self {
            text,
            rate_wpm: Self::DEFAULT_RATE_WPM,
            interrupt: true,
            punctuation: Punctuation::Some,
        }
    }

    /// Builder: set the rate.
    pub const fn with_rate_wpm(mut self, rate_wpm: u16) -> Self {
        self.rate_wpm = rate_wpm;
        self
    }

    /// Builder: set the interrupt flag.
    pub const fn with_interrupt(mut self, interrupt: bool) -> Self {
        self.interrupt = interrupt;
        self
    }

    /// Builder: set the punctuation policy.
    pub const fn with_punctuation(mut self, punctuation: Punctuation) -> Self {
        self.punctuation = punctuation;
        self
    }
}

/// An input event produced by the platform.
///
/// SoulOS expects stylus/touch as the primary input; keyboards and
/// hard buttons are secondary. Coordinates are in virtual-screen
/// space (origin top-left, Y grows downward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// Stylus/finger contacted the panel at `(x, y)`.
    StylusDown { x: i16, y: i16 },
    /// Stylus/finger dragged to `(x, y)`.
    StylusMove { x: i16, y: i16 },
    /// Stylus/finger left the panel at `(x, y)`.
    StylusUp { x: i16, y: i16 },
    /// Scroll-wheel or two-finger swipe. Deltas are in pixel-equivalent
    /// units. Positive `dy` means the user wants to scroll down (reveal
    /// content below); positive `dx` means scroll right.
    Wheel { dx: i16, dy: i16 },
    /// A hardware button was pressed.
    ButtonDown(HardButton),
    /// A hardware button was released.
    ButtonUp(HardButton),
    /// A typing-keyboard key was pressed (repeats on hold).
    Key(KeyCode),
    /// The platform requests shutdown (e.g., window closed).
    Quit,
}

/// A typing-keyboard key, either a printable character or a named
/// editing key.
///
/// Produced by physical keyboards or host-provided soft keyboards.
/// The platform handles modifier resolution (shift, caps lock)
/// before emitting; apps receive the already-composed character.
/// Apps that need raw scan codes should handle that at the HAL
/// level, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A printable Unicode scalar value (shift/caps already applied).
    Char(char),
    /// The delete-left key.
    Backspace,
    /// Return / line feed.
    Enter,
    /// Horizontal tab.
    Tab,
    /// Arrow-left cursor key.
    ArrowLeft,
    /// Arrow-right cursor key.
    ArrowRight,
    /// Arrow-up cursor key.
    ArrowUp,
    /// Arrow-down cursor key.
    ArrowDown,
}

/// The SoulOS hard-button set, modeled on the classic PalmOS device
/// hardware.
///
/// On a hosted simulator these are mapped from function keys. On a
/// real device they're physical buttons. Missing a button on your
/// target is fine — the HAL can simply never emit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardButton {
    /// Power. Closes the active app / suspends the device.
    Power,
    /// Home. Returns the user to the Launcher.
    Home,
    /// Menu. Opens the app's menu bar (analogous to Palm's Menu
    /// silk button).
    Menu,
    /// App quick-launch A. Palm convention: Datebook / Calendar.
    AppA,
    /// App quick-launch B. Palm convention: Address Book.
    AppB,
    /// App quick-launch C. Palm convention: ToDo List.
    AppC,
    /// App quick-launch D. Palm convention: Memo Pad.
    AppD,
    /// Page-up hard button (list scrolling).
    PageUp,
    /// Page-down hard button (list scrolling).
    PageDown,
    /// Volume up.
    VolumeUp,
    /// Volume down.
    VolumeDown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_request_defaults() {
        let r = SpeechRequest::new("hello");
        assert_eq!(r.text, "hello");
        assert_eq!(r.rate_wpm, SpeechRequest::DEFAULT_RATE_WPM);
        assert!(r.interrupt);
        assert_eq!(r.punctuation, Punctuation::Some);
    }

    #[test]
    fn speech_request_builders_chain() {
        let r = SpeechRequest::new("x")
            .with_rate_wpm(240)
            .with_interrupt(false)
            .with_punctuation(Punctuation::All);
        assert_eq!(r.rate_wpm, 240);
        assert!(!r.interrupt);
        assert_eq!(r.punctuation, Punctuation::All);
    }
}
