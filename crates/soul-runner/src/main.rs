//! Desktop runner: hosts the launcher and the system strip.

mod draw;
mod notes;

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{run, App, Ctx, Event, APP_HEIGHT, SCREEN_HEIGHT, SCREEN_WIDTH, SYSTEM_STRIP_H};
use soul_hal::HardButton;
use soul_hal_hosted::HostedPlatform;
use soul_ui::{button, hit_test, title_bar, BLACK, TITLE_BAR_H, WHITE};

use draw::Draw;
use notes::Notes;

const APPS: &[&str] = &[
    "Notes", "Address", "Date", "ToDo", "Mail", "Calc", "Prefs", "Draw", "Sync",
];
const NOTES_IDX: usize = 0;
const DRAW_IDX: usize = 7;

// --- Launcher -----------------------------------------------------------

struct Launcher {
    touched: Option<usize>,
    pending: Option<usize>,
}

impl Launcher {
    fn new() -> Self {
        Self {
            touched: None,
            pending: None,
        }
    }

    fn take_launch(&mut self) -> Option<usize> {
        self.pending.take()
    }

    fn icon_rect(idx: usize) -> Rectangle {
        let cols = 3i32;
        let cell_w: i32 = 68;
        let cell_h: i32 = 68;
        let gutter: i32 = 8;
        let grid_w = cols * cell_w + (cols - 1) * gutter;
        let x_off = (SCREEN_WIDTH as i32 - grid_w) / 2;
        let i = idx as i32;
        let col = i % cols;
        let row = i / cols;
        let x = x_off + col * (cell_w + gutter);
        let y = TITLE_BAR_H as i32 + 12 + row * (cell_h + gutter);
        Rectangle::new(Point::new(x, y), Size::new(cell_w as u32, cell_h as u32))
    }

    fn find_hit(x: i16, y: i16) -> Option<usize> {
        (0..APPS.len()).find(|&i| hit_test(&Self::icon_rect(i), x, y))
    }

    fn set_touched(&mut self, new: Option<usize>, ctx: &mut Ctx<'_>) {
        if new == self.touched {
            return;
        }
        if let Some(i) = self.touched {
            ctx.invalidate(Self::icon_rect(i));
        }
        if let Some(i) = new {
            ctx.invalidate(Self::icon_rect(i));
        }
        self.touched = new;
    }
}

impl App for Launcher {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::PenDown { x, y } | Event::PenMove { x, y } => {
                self.set_touched(Self::find_hit(x, y), ctx);
            }
            Event::PenUp { x, y } => {
                let hit = Self::find_hit(x, y);
                let was = self.touched;
                self.set_touched(None, ctx);
                if hit.is_some() && hit == was {
                    self.pending = hit;
                }
            }
            Event::ButtonDown(HardButton::AppA) => self.pending = Some(0),
            Event::ButtonDown(HardButton::AppB) => self.pending = Some(1),
            Event::ButtonDown(HardButton::AppC) => self.pending = Some(2),
            Event::ButtonDown(HardButton::AppD) => self.pending = Some(3),
            Event::Menu => {}
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Launcher");
        for (i, name) in APPS.iter().enumerate() {
            let r = Self::icon_rect(i);
            let pressed = self.touched == Some(i);
            let _ = button(canvas, r, name, pressed);
        }
    }
}

// --- System strip -------------------------------------------------------

const STRIP_H: i32 = SYSTEM_STRIP_H as i32;
const STRIP_TOP: i32 = APP_HEIGHT as i32;
const STRIP_SEGMENT_W: i32 = SCREEN_WIDTH as i32 / 3;
const FONT_W: i32 = 6;
const FONT_H: i32 = 10;

fn strip_home_rect() -> Rectangle {
    Rectangle::new(
        Point::new(0, STRIP_TOP),
        Size::new(STRIP_SEGMENT_W as u32, STRIP_H as u32),
    )
}

fn strip_menu_rect() -> Rectangle {
    Rectangle::new(
        Point::new(2 * STRIP_SEGMENT_W, STRIP_TOP),
        Size::new(STRIP_SEGMENT_W as u32, STRIP_H as u32),
    )
}

fn strip_rect() -> Rectangle {
    Rectangle::new(
        Point::new(0, STRIP_TOP),
        Size::new(SCREEN_WIDTH as u32, STRIP_H as u32),
    )
}

fn draw_system_strip<D>(canvas: &mut D, active_label: &str)
where
    D: DrawTarget<Color = Gray8>,
{
    let _ = strip_rect()
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(canvas);
    let style = MonoTextStyle::new(&FONT_6X10, WHITE);
    let y = STRIP_TOP + (STRIP_H - FONT_H) / 2;

    // Home label, centered in left third.
    let home = "Home";
    let home_x = (STRIP_SEGMENT_W - home.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(home, Point::new(home_x, y), style, Baseline::Top).draw(canvas);

    // Active-app name, centered in middle third.
    let mid_x =
        STRIP_SEGMENT_W + (STRIP_SEGMENT_W - active_label.chars().count() as i32 * FONT_W) / 2;
    let _ =
        Text::with_baseline(active_label, Point::new(mid_x, y), style, Baseline::Top).draw(canvas);

    // Menu label, centered in right third.
    let menu = "Menu";
    let menu_x = 2 * STRIP_SEGMENT_W + (STRIP_SEGMENT_W - menu.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(menu, Point::new(menu_x, y), style, Baseline::Top).draw(canvas);
}

// --- Host ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Launcher,
    Notes,
    Draw,
}

struct Host {
    launcher: Launcher,
    notes: Notes,
    draw: Draw,
    active: Slot,
    /// `true` while a press that began inside the system strip is
    /// in flight. Child apps don't see any event during this window.
    strip_pressed: bool,
}

impl Host {
    fn new() -> Self {
        Self {
            launcher: Launcher::new(),
            notes: Notes::new(),
            draw: Draw::new(),
            active: Slot::Launcher,
            strip_pressed: false,
        }
    }

    fn switch_to(&mut self, slot: Slot, ctx: &mut Ctx<'_>) {
        if self.active != slot {
            self.active = slot;
            ctx.invalidate_all();
        }
    }

    fn active_label(&self) -> &'static str {
        match self.active {
            Slot::Launcher => "Launcher",
            Slot::Notes => "Notes",
            Slot::Draw => "Draw",
        }
    }

    fn forward_to_child(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match self.active {
            Slot::Launcher => {
                self.launcher.handle(event, ctx);
                if let Some(idx) = self.launcher.take_launch() {
                    if idx == NOTES_IDX {
                        self.switch_to(Slot::Notes, ctx);
                    } else if idx == DRAW_IDX {
                        self.switch_to(Slot::Draw, ctx);
                    }
                }
            }
            Slot::Notes => self.notes.handle(event, ctx),
            Slot::Draw => self.draw.handle(event, ctx),
        }
    }
}

impl App for Host {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        // Hardware fallback still works.
        if matches!(event, Event::ButtonDown(HardButton::Home)) {
            self.switch_to(Slot::Launcher, ctx);
            return;
        }

        // Capture presses that begin in the system strip.
        if let Event::PenDown { y, .. } = event {
            if (y as i32) >= STRIP_TOP {
                self.strip_pressed = true;
                return;
            }
        }

        if self.strip_pressed {
            if let Event::PenUp { x, y } = event {
                self.strip_pressed = false;
                if hit_test(&strip_home_rect(), x, y) {
                    self.switch_to(Slot::Launcher, ctx);
                } else if hit_test(&strip_menu_rect(), x, y) {
                    self.forward_to_child(Event::Menu, ctx);
                }
            }
            return;
        }

        self.forward_to_child(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        match self.active {
            Slot::Launcher => self.launcher.draw(canvas),
            Slot::Notes => self.notes.draw(canvas),
            Slot::Draw => self.draw.draw(canvas),
        }
        draw_system_strip(canvas, self.active_label());
    }
}

fn main() {
    let mut platform = HostedPlatform::new("SoulOS", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    run(&mut platform, Host::new());
}
