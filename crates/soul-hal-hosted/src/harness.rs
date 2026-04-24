//! Headless platform for deterministic testing.
//!
//! Stage 1 of the test harness (see `docs/Harness.md` §4.1, §4.2).
//! Provides a `Platform` impl that runs with no window and reads time
//! from a clock the test advances explicitly. Higher-level driver API
//! (`Harness::tap`, `find_text`, `snapshot`, …) lands in later stages.

use std::collections::VecDeque;

use soul_hal::{InputEvent, Platform};

use crate::MiniFbDisplay;

/// A monotonic clock that only advances when the test asks it to.
///
/// Replaces `Instant::now()` for frame-exact reproducibility — animations
/// driven by `Event::Tick(ms)` land on the same frame every run.
#[derive(Debug, Default, Clone)]
pub struct VirtualClock {
    elapsed_ms: u64,
}

impl VirtualClock {
    pub fn new() -> Self {
        Self { elapsed_ms: 0 }
    }

    pub fn now_ms(&self) -> u64 {
        self.elapsed_ms
    }

    pub fn advance(&mut self, ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(ms);
    }
}

/// A `Platform` with no window. Same framebuffer type as `HostedPlatform`,
/// but: no minifb, virtual clock, no-op flush, speech captured to a `Vec`.
pub struct HeadlessPlatform {
    pub display: MiniFbDisplay,
    pub pending: VecDeque<InputEvent>,
    pub clock: VirtualClock,
    pub speech_log: Vec<String>,
}

impl HeadlessPlatform {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            display: MiniFbDisplay::new(width, height),
            pending: VecDeque::new(),
            clock: VirtualClock::new(),
            speech_log: Vec::new(),
        }
    }
}

impl Platform for HeadlessPlatform {
    type Display = MiniFbDisplay;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.pending.pop_front()
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn flush(&mut self) {}

    fn sleep_ms(&mut self, ms: u32) {
        self.clock.advance(ms as u64);
    }

    fn speak(&mut self, text: &str) {
        self.speech_log.push(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_clock_starts_at_zero_and_advances() {
        let mut clock = VirtualClock::new();
        assert_eq!(clock.now_ms(), 0);
        clock.advance(16);
        assert_eq!(clock.now_ms(), 16);
        clock.advance(100);
        assert_eq!(clock.now_ms(), 116);
    }

    #[test]
    fn headless_platform_clock_is_deterministic() {
        let mut p = HeadlessPlatform::new(240, 320);
        assert_eq!(p.now_ms(), 0);
        p.sleep_ms(16);
        assert_eq!(p.now_ms(), 16);
        p.sleep_ms(16);
        assert_eq!(p.now_ms(), 32);
    }

    #[test]
    fn headless_platform_drains_input_fifo() {
        let mut p = HeadlessPlatform::new(240, 320);
        p.pending.push_back(InputEvent::StylusDown { x: 10, y: 20 });
        p.pending.push_back(InputEvent::StylusUp { x: 10, y: 20 });
        assert_eq!(
            p.poll_event(),
            Some(InputEvent::StylusDown { x: 10, y: 20 })
        );
        assert_eq!(p.poll_event(), Some(InputEvent::StylusUp { x: 10, y: 20 }));
        assert_eq!(p.poll_event(), None);
    }

    #[test]
    fn headless_platform_records_speech() {
        let mut p = HeadlessPlatform::new(240, 320);
        p.speak("hello");
        p.speak("world");
        assert_eq!(p.speech_log, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn headless_platform_flush_is_a_noop() {
        let mut p = HeadlessPlatform::new(240, 320);
        p.flush();
        p.flush();
    }
}
