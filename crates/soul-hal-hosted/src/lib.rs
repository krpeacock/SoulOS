//! Hosted desktop HAL backed by embedded-graphics-simulator (SDL2).
//! Lets the same app code that runs on bare metal also run in a window.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use embedded_graphics::{pixelcolor::Gray8, prelude::*};
use embedded_graphics_simulator::{
    sdl2::{Keycode, Mod, MouseButton},
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use soul_hal::{HardButton, InputEvent, KeyCode, Platform};

pub mod testing;

pub struct HostedPlatform {
    pub display: SimulatorDisplay<Gray8>,
    window: Window,
    start: Instant,
    stylus_down: bool,
    pub pending: VecDeque<InputEvent>,
}

impl HostedPlatform {
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        let display = SimulatorDisplay::<Gray8>::new(Size::new(width, height));
        let output = OutputSettingsBuilder::new().scale(2).build();
        let window = Window::new(title, &output);
        Self {
            display,
            window,
            start: Instant::now(),
            stylus_down: false,
            pending: VecDeque::new(),
        }
    }

    fn pump(&mut self) {
        for ev in self.window.events() {
            match ev {
                SimulatorEvent::Quit => self.pending.push_back(InputEvent::Quit),
                SimulatorEvent::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    point,
                } => {
                    self.stylus_down = true;
                    self.pending.push_back(InputEvent::StylusDown {
                        x: point.x as i16,
                        y: point.y as i16,
                    });
                }
                SimulatorEvent::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    point,
                } => {
                    self.stylus_down = false;
                    self.pending.push_back(InputEvent::StylusUp {
                        x: point.x as i16,
                        y: point.y as i16,
                    });
                }
                SimulatorEvent::MouseMove { point } => {
                    if self.stylus_down {
                        self.pending.push_back(InputEvent::StylusMove {
                            x: point.x as i16,
                            y: point.y as i16,
                        });
                    }
                }
                SimulatorEvent::KeyDown {
                    keycode, keymod, ..
                } => {
                    if let Some(b) = map_hard_button(keycode) {
                        self.pending.push_back(InputEvent::ButtonDown(b));
                    } else if let Some(kc) = map_keycode(keycode, keymod) {
                        self.pending.push_back(InputEvent::Key(kc));
                    }
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    if let Some(b) = map_hard_button(keycode) {
                        self.pending.push_back(InputEvent::ButtonUp(b));
                    }
                }
                _ => {}
            }
        }
    }
}

impl Platform for HostedPlatform {
    type Display = SimulatorDisplay<Gray8>;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.pending.pop_front()
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn flush(&mut self) {
        self.window.update(&self.display);
        self.pump();
    }

    fn sleep_ms(&mut self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }

    fn speak(&mut self, text: &str) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("say").arg(text).spawn();
        }
        #[cfg(not(target_os = "macos"))]
        {
            println!("[TTS]: {}", text);
        }
    }
}

// Hard buttons live on F-keys so letter/digit keys are free for text input.
fn map_hard_button(k: Keycode) -> Option<HardButton> {
    Some(match k {
        Keycode::Escape => HardButton::Power,
        Keycode::F1 => HardButton::AppA,
        Keycode::F2 => HardButton::AppB,
        Keycode::F3 => HardButton::AppC,
        Keycode::F4 => HardButton::AppD,
        Keycode::F5 | Keycode::Home => HardButton::Home,
        Keycode::F6 => HardButton::Menu,
        Keycode::PageUp => HardButton::PageUp,
        Keycode::PageDown => HardButton::PageDown,
        _ => return None,
    })
}

fn map_keycode(k: Keycode, m: Mod) -> Option<KeyCode> {
    use Keycode as K;
    let shift = m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    let caps = m.contains(Mod::CAPSMOD);
    match k {
        K::Backspace => Some(KeyCode::Backspace),
        K::Return | K::KpEnter => Some(KeyCode::Enter),
        K::Tab => Some(KeyCode::Tab),
        K::Left => Some(KeyCode::ArrowLeft),
        K::Right => Some(KeyCode::ArrowRight),
        K::Up => Some(KeyCode::ArrowUp),
        K::Down => Some(KeyCode::ArrowDown),
        _ => keycode_to_char(k, shift).map(|c| {
            // Letters honor shift XOR capslock; non-letters already
            // use `shift` to switch between the unshifted and shifted form.
            if c.is_ascii_alphabetic() && (shift ^ caps) {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                KeyCode::Char(c)
            }
        }),
    }
}

fn keycode_to_char(kc: Keycode, shift: bool) -> Option<char> {
    use Keycode as K;
    Some(match kc {
        K::A => 'a',
        K::B => 'b',
        K::C => 'c',
        K::D => 'd',
        K::E => 'e',
        K::F => 'f',
        K::G => 'g',
        K::H => 'h',
        K::I => 'i',
        K::J => 'j',
        K::K => 'k',
        K::L => 'l',
        K::M => 'm',
        K::N => 'n',
        K::O => 'o',
        K::P => 'p',
        K::Q => 'q',
        K::R => 'r',
        K::S => 's',
        K::T => 't',
        K::U => 'u',
        K::V => 'v',
        K::W => 'w',
        K::X => 'x',
        K::Y => 'y',
        K::Z => 'z',
        K::Num0 if shift => ')',
        K::Num0 => '0',
        K::Num1 if shift => '!',
        K::Num1 => '1',
        K::Num2 if shift => '@',
        K::Num2 => '2',
        K::Num3 if shift => '#',
        K::Num3 => '3',
        K::Num4 if shift => '$',
        K::Num4 => '4',
        K::Num5 if shift => '%',
        K::Num5 => '5',
        K::Num6 if shift => '^',
        K::Num6 => '6',
        K::Num7 if shift => '&',
        K::Num7 => '7',
        K::Num8 if shift => '*',
        K::Num8 => '8',
        K::Num9 if shift => '(',
        K::Num9 => '9',
        K::Space => ' ',
        K::Period if shift => '>',
        K::Period => '.',
        K::Comma if shift => '<',
        K::Comma => ',',
        K::Minus if shift => '_',
        K::Minus => '-',
        K::Equals if shift => '+',
        K::Equals => '=',
        K::Slash if shift => '?',
        K::Slash => '/',
        K::Backslash if shift => '|',
        K::Backslash => '\\',
        K::Semicolon if shift => ':',
        K::Semicolon => ';',
        K::Quote if shift => '"',
        K::Quote => '\'',
        K::Backquote if shift => '~',
        K::Backquote => '`',
        K::LeftBracket if shift => '{',
        K::LeftBracket => '[',
        K::RightBracket if shift => '}',
        K::RightBracket => ']',
        _ => return None,
    })
}
