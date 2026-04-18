//! Testing utilities for SoulOS applications
//! Provides programmatic input injection and state inspection

use std::collections::VecDeque;
use embedded_graphics::{pixelcolor::Gray8, prelude::*};
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
    
    /// Get the current display buffer for analysis
    fn get_display_buffer(&self) -> &embedded_graphics_simulator::SimulatorDisplay<Gray8>;
    
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
        // For now, we'd need to implement screenshot saving
        // This would save the current display buffer as an image
        Ok(())
    }
    
    fn get_display_buffer(&self) -> &embedded_graphics_simulator::SimulatorDisplay<Gray8> {
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
    
    /// Test scenario to open the Notes app and type some text
    pub fn test_notes_app() -> TestScenario {
        let mut events = Vec::new();
        
        // Click on Notes app (coordinates from launcher)
        events.extend(TestEvent::click(363, 170, "Click Notes app"));
        
        // Wait a bit for app to load
        events.push(TestEvent {
            event: InputEvent::StylusDown { x: 0, y: 0 }, // Dummy event for delay
            delay_ms: 500,
            description: "Wait for Notes app to load".to_string(),
        });
        
        // Type some test text
        events.extend(TestEvent::type_string("Hello from automated test!"));
        
        // Press Enter
        events.push(TestEvent::key(
            soul_hal::KeyCode::Enter,
            "Press Enter to confirm"
        ));
        
        TestScenario {
            name: "Test Notes App".to_string(),
            events,
        }
    }
    
    /// Test scenario to navigate back to home
    pub fn return_to_home() -> TestScenario {
        TestScenario {
            name: "Return to Home".to_string(),
            events: TestEvent::hard_button(
                soul_hal::HardButton::Home,
                "Press Home button"
            ),
        }
    }
    
    /// Test scenario for address book
    pub fn test_address_app() -> TestScenario {
        let mut events = Vec::new();
        
        events.extend(TestEvent::click(320, 170, "Click Address app"));
        
        TestScenario {
            name: "Test Address App".to_string(),
            events,
        }
    }
}