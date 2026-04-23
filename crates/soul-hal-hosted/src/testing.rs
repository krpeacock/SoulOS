//! Testing utilities for SoulOS applications
//! Provides programmatic input injection and state inspection

use soul_hal::InputEvent;

/// A test scenario that can be programmatically executed
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub events: Vec<TestEvent>,
}

/// A single test event with optional timing
#[derive(Debug, Clone)]
pub struct TestEvent {
    pub event: InputEvent,
    pub delay_ms: u64,
    pub description: String,
}

impl TestEvent {
    pub fn click(x: i16, y: i16, description: &str) -> Vec<Self> {
        vec![
            Self {
                event: InputEvent::StylusDown { x, y },
                delay_ms: 50,
                description: format!("{} - pen down", description),
            },
            Self {
                event: InputEvent::StylusUp { x, y },
                delay_ms: 100,
                description: format!("{} - pen up", description),
            },
        ]
    }

    pub fn type_string(text: &str) -> Vec<Self> {
        text.chars()
            .map(|c| Self {
                event: InputEvent::Key(soul_hal::KeyCode::Char(c)),
                delay_ms: 50,
                description: format!("Type '{}'", c),
            })
            .collect()
    }

    pub fn key(key: soul_hal::KeyCode, description: &str) -> Self {
        Self {
            event: InputEvent::Key(key),
            delay_ms: 100,
            description: description.to_string(),
        }
    }

    pub fn hard_button(button: soul_hal::HardButton, description: &str) -> Vec<Self> {
        vec![
            Self {
                event: InputEvent::ButtonDown(button),
                delay_ms: 50,
                description: format!("{} - button down", description),
            },
            Self {
                event: InputEvent::ButtonUp(button),
                delay_ms: 100,
                description: format!("{} - button up", description),
            },
        ]
    }
}

/// Testing extension for HostedPlatform
pub trait TestingPlatform {
    /// Inject a sequence of test events
    fn inject_events(&mut self, events: Vec<TestEvent>);

    /// Take a screenshot and save to file
    fn screenshot(&self, filename: &str) -> Result<(), std::io::Error>;

    /// Get the current display for analysis (implements `DrawTarget<Color = Gray8>`).
    fn get_display_buffer(&self) -> &crate::MiniFbDisplay;

    /// Wait for a specific number of frames
    fn wait_frames(&mut self, frames: u32);
}

impl TestingPlatform for crate::HostedPlatform {
    fn inject_events(&mut self, events: Vec<TestEvent>) {
        for test_event in events {
            self.pending.push_back(test_event.event);
        }
    }

    fn screenshot(&self, filename: &str) -> Result<(), std::io::Error> {
        Ok(())
    }

    fn get_display_buffer(&self) -> &crate::MiniFbDisplay {
        &self.display
    }

    fn wait_frames(&mut self, frames: u32) {
        for _ in 0..frames {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

/// Predefined test scenarios for common SoulOS operations
pub mod scenarios {
    use super::*;

    pub fn test_notes_app() -> TestScenario {
        let mut events = Vec::new();
        events.extend(TestEvent::click(363, 170, "Click Notes app"));
        events.push(TestEvent {
            event: InputEvent::StylusDown { x: 0, y: 0 },
            delay_ms: 500,
            description: "Wait for Notes app to load".to_string(),
        });
        events.extend(TestEvent::type_string("Hello from automated test!"));
        events.push(TestEvent::key(
            soul_hal::KeyCode::Enter,
            "Press Enter to confirm",
        ));
        TestScenario {
            name: "Test Notes App".to_string(),
            events,
        }
    }

    pub fn return_to_home() -> TestScenario {
        TestScenario {
            name: "Return to Home".to_string(),
            events: TestEvent::hard_button(soul_hal::HardButton::Home, "Press Home button"),
        }
    }

    pub fn test_address_app() -> TestScenario {
        let mut events = Vec::new();
        events.extend(TestEvent::click(320, 170, "Click Address app"));
        TestScenario {
            name: "Test Address App".to_string(),
            events,
        }
    }

    pub fn build_todo_app() -> TestScenario {
        let mut events = Vec::new();
        events.extend(TestEvent::click(86, 119, "Open Builder"));
        events.extend(TestEvent::hard_button(
            soul_hal::HardButton::Menu,
            "Open Menu",
        ));
        events.extend(TestEvent::click(100, 89, "Add Label"));
        events.extend(TestEvent::hard_button(
            soul_hal::HardButton::Menu,
            "Open Menu",
        ));
        events.extend(TestEvent::click(100, 193, "Edit Label"));
        events.extend(TestEvent::type_string("My Tasks"));
        events.push(TestEvent::key(soul_hal::KeyCode::Enter, "Confirm Label"));
        events.extend(TestEvent::hard_button(
            soul_hal::HardButton::Menu,
            "Open Menu",
        ));
        events.extend(TestEvent::click(100, 167, "Add Checkbox"));
        events.push(TestEvent {
            event: InputEvent::StylusDown { x: 30, y: 50 },
            delay_ms: 100,
            description: "Select Checkbox".to_string(),
        });
        events.push(TestEvent {
            event: InputEvent::StylusMove { x: 30, y: 100 },
            delay_ms: 100,
            description: "Drag Checkbox".to_string(),
        });
        events.push(TestEvent {
            event: InputEvent::StylusUp { x: 30, y: 100 },
            delay_ms: 100,
            description: "Drop Checkbox".to_string(),
        });
        events.extend(TestEvent::hard_button(
            soul_hal::HardButton::Menu,
            "Open Menu",
        ));
        events.extend(TestEvent::click(100, 245, "Save Form"));
        TestScenario {
            name: "Build Todo App".to_string(),
            events,
        }
    }

    pub fn verify_todo_app() -> TestScenario {
        let mut events = Vec::new();
        events.extend(TestEvent::click(122, 140, "Open Todo App"));
        events.extend(TestEvent::click(40, 104, "Toggle Task"));
        TestScenario {
            name: "Verify Todo App".to_string(),
            events,
        }
    }
}
