//! Desktop runner: hosts all apps and the system strip.

mod builder;
mod draw;
mod launcher;
mod launcher_store;

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{
    run, App, Ctx, Event, HardButton, KeyCode, APP_HEIGHT, SCREEN_HEIGHT, SCREEN_WIDTH,
    SYSTEM_STRIP_H,
};
use soul_hal_hosted::HostedPlatform;
use soul_script::ScriptedApp;
use soul_ui::{hit_test, BLACK, WHITE};
use rhai::Position;
use std::path::{Path, PathBuf};

use builder::MobileBuilder;
use draw::Draw;
use launcher::Launcher;

/// Square PGM icon size; must match `generate_icons.py` export size.
pub(crate) const ICON_CELL: u32 = 32;

// --- Native apps --------------------------------------------------------

/// All native (non-scripted) app instances.  Stored inline in the enum —
/// no heap allocation, no vtable, generic `draw<D>` passes the real canvas
/// straight through with zero indirection or intermediate buffer.
pub(crate) enum NativeKind {
    Launcher(Launcher),
    Draw(Box<Draw>),
    Builder(MobileBuilder),
}

impl NativeKind {
    fn app_id(&self) -> &str {
        match self {
            NativeKind::Launcher(_) => Launcher::APP_ID,
            NativeKind::Draw(_) => Draw::APP_ID,
            NativeKind::Builder(_) => MobileBuilder::APP_ID,
        }
    }

    fn name(&self) -> &str {
        match self {
            NativeKind::Launcher(_) => Launcher::NAME,
            NativeKind::Draw(_) => Draw::NAME,
            NativeKind::Builder(_) => MobileBuilder::NAME,
        }
    }

    /// Stem for loading `assets/sprites/{stem}_icon.pgm`.
    /// Launcher returns `None` — it is not listed in the app registry.
    fn icon_stem(&self) -> Option<&str> {
        match self {
            NativeKind::Launcher(_) => None,
            NativeKind::Draw(_) => Some("draw"),
            NativeKind::Builder(_) => Some("builder"),
        }
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<soul_script::SystemRequest> {
        match self {
            NativeKind::Launcher(l) => l.handle(event, ctx),
            NativeKind::Draw(d) => {
                d.handle(event, ctx);
                None
            }
            NativeKind::Builder(b) => {
                b.handle(event, ctx);
                None
            }
        }
    }

    fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        match self {
            NativeKind::Launcher(l) => l.draw(canvas),
            NativeKind::Draw(d) => d.draw(canvas),
            NativeKind::Builder(b) => b.draw(canvas),
        }
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        match self {
            NativeKind::Launcher(l) => l.a11y_nodes(),
            NativeKind::Draw(d) => d.a11y_nodes(),
            NativeKind::Builder(b) => b.a11y_nodes(),
        }
    }

    fn persist(&mut self) {
        match self {
            NativeKind::Launcher(_) => {}
            NativeKind::Draw(d) => d.persist(),
            NativeKind::Builder(b) => b.persist(),
        }
    }
}

// --- App registry -------------------------------------------------------

/// Static declaration for one app slot in the manifest.
/// Launcher is NOT listed here — it is created separately at `apps[0]`.
///
/// Scripted apps provide their identity (`app_id`, `app_name`, `app_icon`)
/// as top-level `let` declarations inside the script itself. The manifest
/// only needs to know where to find the script and where to persist its DB.
pub(crate) struct AppDescriptor {
    kind: AppKind,
}

enum AppKind {
    /// A Rhai script. The script must declare `app_id` and `app_name`.
    /// `app_icon` is optional (falls back to a generic white square).
    Scripted {
        script: &'static str,
        db: &'static str,
    },
    Draw,
    Builder,
}

/// The app manifest. Only the minimum needed to locate and load each app.
/// Launcher is excluded — Host inserts it at apps[0].
pub(crate) const APP_MANIFEST: &[AppDescriptor] = &[
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/notes.rhai",
            db: ".soulos/notes_v2.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/address.rhai",
            db: ".soulos/address_v2.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/date.rhai",
            db: ".soulos/date.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/todo.rhai",
            db: ".soulos/todo_v2.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/egui_demo.rhai",
            db: ".soulos/egui_demo.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/kitchen_sink.rhai",
            db: ".soulos/kitchen_sink.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/mail.rhai",
            db: ".soulos/mail.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/calc.rhai",
            db: ".soulos/calc.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/prefs.rhai",
            db: ".soulos/prefs.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Draw,
    },
    AppDescriptor {
        kind: AppKind::Scripted {
            script: "assets/scripts/sync.rhai",
            db: ".soulos/sync.sdb",
        },
    },
    AppDescriptor {
        kind: AppKind::Builder,
    },
];

/// A live app instance.
/// `apps[0]` is always `Native(NativeKind::Launcher)`; the rest correspond to
/// `APP_MANIFEST` in order.
enum AppSlot {
    /// A Rhai scripted app. Identity is declared inside the script.
    Scripted { app: Box<ScriptedApp>, db_path: PathBuf },
    /// Any native app — stored inline, dispatched statically, no heap overhead.
    Native(NativeKind),
}

impl AppSlot {
    fn from_descriptor(desc: &AppDescriptor) -> Self {
        match &desc.kind {
            AppKind::Scripted { script, db } => {
                let script_stem = Path::new(script)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("app");
                let db_path = PathBuf::from(db);
                let script_src = std::fs::read_to_string(script).unwrap_or_else(|e| {
                    log::error!("Failed to load {}: {}", script, e);
                    format!(
                        "fn on_draw() {{ title_bar(\"{}: Load Error\"); }} fn on_event(ev) {{}}",
                        script_stem
                    )
                });
                let soul_db = if let Ok(bytes) = std::fs::read(&db_path) {
                    soul_db::Database::decode(&bytes)
                        .unwrap_or_else(|| soul_db::Database::new(script_stem))
                } else {
                    soul_db::Database::new(script_stem)
                };
                match ScriptedApp::new(script_stem, &script_src, soul_db) {
                    Ok(app) => {
                        log::info!(
                            "✅ {} ({}) loaded",
                            app.declared_name().as_deref().unwrap_or(script_stem),
                            script_stem
                        );
                        AppSlot::Scripted { app: Box::new(app), db_path }
                    }
                    Err(e) => {
                        // Enhanced error reporting with detailed position information
                        let error_details = match e.position() {
                            Position::NONE => {
                                format!("Failed to compile {}: {}", script_stem, e)
                            }
                            pos => {
                                let line = pos.line().unwrap_or(0);
                                let col = pos.position().unwrap_or(0);
                                format!(
                                    "Failed to compile {} at line {}, column {}: {}",
                                    script_stem, line, col, e
                                )
                            }
                        };
                        
                        log::error!("{}", error_details);
                        
                        // Try to show context around the error
                        if let Some(line_num) = e.position().line() {
                            let script_lines: Vec<&str> = script.lines().collect();
                            let line_idx = line_num - 1;
                            
                            if line_idx < script_lines.len() {
                                log::error!("Context around error:");
                                
                                // Show 2 lines before
                                if line_idx >= 2 {
                                    log::error!("  {} | {}", line_num - 2, script_lines[line_idx - 2]);
                                }
                                if line_idx >= 1 {
                                    log::error!("  {} | {}", line_num - 1, script_lines[line_idx - 1]);
                                }
                                
                                // Show the error line with pointer
                                log::error!("▶ {} | {}", line_num, script_lines[line_idx]);
                                
                                if let Some(col) = e.position().position() {
                                    let pointer = " ".repeat((col - 1) + format!("▶ {} | ", line_num).len()) + "^";
                                    log::error!("{}", pointer);
                                }
                                
                                // Show 2 lines after
                                if line_idx + 1 < script_lines.len() {
                                    log::error!("  {} | {}", line_num + 1, script_lines[line_idx + 1]);
                                }
                                if line_idx + 2 < script_lines.len() {
                                    log::error!("  {} | {}", line_num + 2, script_lines[line_idx + 2]);
                                }
                            }
                        }
                        
                        let err_script = format!(
                            "let app_id = \"error.{script_stem}\";\
                             let app_name = \"{script_stem}\";\
                             fn on_draw() {{ title_bar(\"{script_stem}\"); label(10, 80, \"Script error.\"); }}\
                             fn on_event(ev) {{}}"
                        );
                        let err_db = soul_db::Database::new(script_stem);
                        let err_app = ScriptedApp::new(script_stem, &err_script, err_db)
                            .expect("error fallback script is always valid");
                        AppSlot::Scripted {
                            app: Box::new(err_app),
                            db_path: PathBuf::new(),
                        }
                    }
                }
            }
            AppKind::Draw => AppSlot::Native(NativeKind::Draw(Box::new(Draw::new()))),
            AppKind::Builder => AppSlot::Native(NativeKind::Builder(MobileBuilder::new())),
        }
    }

    /// The stable, app-assigned unique identifier.
    /// For scripted apps this comes from the script's own `let app_id = "..."`.
    fn app_id(&self) -> String {
        match self {
            AppSlot::Scripted { app, .. } => app
                .declared_app_id()
                .unwrap_or_else(|| format!("unknown.{}", app.script_name())),
            AppSlot::Native(n) => n.app_id().to_string(),
        }
    }

    /// The display name. For scripted apps comes from `let app_name = "..."`.
    fn name(&self) -> String {
        match self {
            AppSlot::Scripted { app, .. } => app
                .declared_name()
                .unwrap_or_else(|| app.script_name().to_string()),
            AppSlot::Native(n) => n.name().to_string(),
        }
    }

    /// The icon stem for loading `assets/sprites/{stem}_icon.pgm`.
    /// Returns `None` for the Launcher (not listed in the app registry).
    fn icon_stem(&self) -> Option<String> {
        match self {
            AppSlot::Scripted { app, .. } => Some(
                app.declared_icon_name()
                    .unwrap_or_else(|| app.script_name().to_string()),
            ),
            AppSlot::Native(n) => n.icon_stem().map(str::to_string),
        }
    }

    /// Handle an event. Returns any system request the app emitted.
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<soul_script::SystemRequest> {
        match self {
            AppSlot::Scripted { app, .. } => {
                app.handle(event, ctx);
                Self::drain_script_errors(app);
                soul_script::take_system_request()
            }
            AppSlot::Native(n) => n.handle(event, ctx),
        }
    }

    fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        match self {
            AppSlot::Scripted { app, .. } => {
                app.draw(canvas);
                Self::drain_script_errors(app);
            }
            AppSlot::Native(n) => n.draw(canvas),
        }
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        match self {
            AppSlot::Scripted { app, .. } => app.a11y_nodes(),
            AppSlot::Native(n) => n.a11y_nodes(),
        }
    }

    fn persist(&mut self) {
        match self {
            AppSlot::Scripted { app, db_path } => {
                if !db_path.as_os_str().is_empty() {
                    let _ = std::fs::write(db_path, app.db.encode());
                }
            }
            AppSlot::Native(n) => n.persist(),
        }
    }

    fn drain_script_errors(app: &mut ScriptedApp) {
        if let Some(error) = app.last_error() {
            log::error!(
                "🔥 RHAI ERROR in {} -> {}()",
                error.script_name,
                error.function_name
            );
            log::error!("   Error: {}", error.error_message);
            if let Some(line) = error.line {
                log::error!("   Line: {}", line);
                let lines: Vec<&str> = app.script_source().lines().collect();
                if line > 0 && line <= lines.len() {
                    let start = (line.saturating_sub(3)).max(1);
                    let end = (line + 2).min(lines.len());
                    log::error!("   Source context:");
                    for i in start..=end {
                        if let Some(src) = lines.get(i.saturating_sub(1)) {
                            let marker = if i == line { " >>> " } else { "     " };
                            log::error!("{}{:4} | {}", marker, i, src);
                        }
                    }
                }
            }
            log::error!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            app.clear_error();
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

    let home = "Home";
    let home_x = (STRIP_SEGMENT_W - home.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(home, Point::new(home_x, y), style, Baseline::Top).draw(canvas);

    let mid_x =
        STRIP_SEGMENT_W + (STRIP_SEGMENT_W - active_label.chars().count() as i32 * FONT_W) / 2;
    let _ =
        Text::with_baseline(active_label, Point::new(mid_x, y), style, Baseline::Top).draw(canvas);

    let menu = "Menu";
    let menu_x = 2 * STRIP_SEGMENT_W + (STRIP_SEGMENT_W - menu.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(menu, Point::new(menu_x, y), style, Baseline::Top).draw(canvas);
}

// --- Host ---------------------------------------------------------------

struct Host {
    /// All app instances. Index 0 is always the Launcher.
    /// The rest correspond to APP_MANIFEST entries in order.
    apps: Vec<AppSlot>,

    /// Navigation stack of indices into `apps`.
    /// The bottom entry is always 0 (Launcher). Launching pushes; returning pops.
    stack: Vec<usize>,

    /// Heap-stable app registry for Rhai's `system_list_apps()`.
    _app_meta: Box<Vec<soul_script::AppEntry>>,

    strip_pressed: bool,
    a11y_enabled: bool,
    a11y_focus: Option<usize>,
    pen_start: Option<(i16, i16, u64)>,
    last_tap: Option<(i16, i16, u64)>,
    tap_count: u8,
}

impl Host {
    fn new() -> Self {
        log::info!("🏠 Initializing Host...");

        // apps[0] = Launcher; apps[1..] from APP_MANIFEST
        let mut apps: Vec<AppSlot> = Vec::with_capacity(APP_MANIFEST.len() + 1);
        apps.push(AppSlot::Native(NativeKind::Launcher(Launcher::new())));
        for desc in APP_MANIFEST {
            apps.push(AppSlot::from_descriptor(desc));
        }

        // Build and register the app metadata for Rhai's `system_list_apps()`.
        // Excludes Launcher (index 0). `icon_stem` lets the Launcher load PGM icons.
        let app_meta: Box<Vec<soul_script::AppEntry>> = Box::new(
            apps.iter()
                .enumerate()
                .skip(1)
                .map(|(slot_idx, slot)| soul_script::AppEntry {
                    app_id: slot.app_id(),
                    name: slot.name(),
                    slot_idx,
                    icon_stem: slot.icon_stem().unwrap_or_default(),
                })
                .collect(),
        );
        // SAFETY: app_meta is boxed (heap-stable) and lives as long as Host.
        unsafe {
            soul_script::set_app_list(app_meta.as_ref() as *const _);
        }

        log::info!("🎉 All apps loaded ({} + launcher).", APP_MANIFEST.len());
        Self {
            apps,
            stack: vec![0], // start at Launcher
            _app_meta: app_meta,
            strip_pressed: false,
            a11y_enabled: false,
            a11y_focus: None,
            pen_start: None,
            last_tap: None,
            tap_count: 0,
        }
    }

    fn active_idx(&self) -> usize {
        *self.stack.last().unwrap_or(&0)
    }

    /// Push an app onto the navigation stack. Index 0 (Launcher) cannot be pushed.
    fn launch_app(&mut self, idx: usize, ctx: &mut Ctx<'_>) {
        if idx > 0 && idx < self.apps.len() {
            self.stack.push(idx);
            ctx.invalidate_all();
        }
    }

    /// Resolve a stable app ID to a slot index and launch it.
    fn launch_by_id(&mut self, id: &str, ctx: &mut Ctx<'_>) {
        if let Some(idx) = self.apps.iter().position(|s| s.app_id() == id) {
            self.launch_app(idx, ctx);
        }
    }

    /// Pop one level from the stack (return to caller).
    fn go_back(&mut self, ctx: &mut Ctx<'_>) {
        if self.stack.len() > 1 {
            self.stack.pop();
            ctx.invalidate_all();
        }
    }

    /// Clear the stack back to the Launcher (apps[0]).
    fn go_home(&mut self, ctx: &mut Ctx<'_>) {
        if self.stack != [0] {
            self.stack.clear();
            self.stack.push(0);
            ctx.invalidate_all();
        }
    }

    fn active_label(&self) -> String {
        self.apps[self.active_idx()].name()
    }

    fn active_a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        let mut nodes = self.apps[self.active_idx()].a11y_nodes();
        nodes.push(soul_core::a11y::A11yNode {
            bounds: strip_home_rect(),
            label: "Home".into(),
            role: "system_button".into(),
        });
        nodes.push(soul_core::a11y::A11yNode {
            bounds: strip_menu_rect(),
            label: "Menu".into(),
            role: "system_button".into(),
        });
        nodes
    }

    fn speak_focused(&self, ctx: &mut Ctx<'_>) {
        if let Some(idx) = self.a11y_focus {
            if let Some(node) = self.active_a11y_nodes().get(idx) {
                ctx.a11y.speak(&format!("{}, {}", node.label, node.role));
            }
        }
    }

    fn activate_focused(&mut self, ctx: &mut Ctx<'_>) {
        if let Some(idx) = self.a11y_focus {
            let nodes = self.active_a11y_nodes();
            if let Some(node) = nodes.get(idx) {
                let center = node.bounds.center();
                let x = center.x as i16;
                let y = center.y as i16;
                if node.role == "system_button" {
                    if node.label == "Home" {
                        self.go_home(ctx);
                    } else if node.label == "Menu" {
                        self.dispatch_event(Event::Menu, ctx);
                    }
                } else {
                    self.dispatch_event(Event::PenDown { x, y }, ctx);
                    self.dispatch_event(Event::PenUp { x, y }, ctx);
                }
            }
        }
    }

    /// Dispatch an event to the active app and process any system request it emits.
    fn dispatch_event(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        let active = self.active_idx();
        let request = self.apps[active].handle(event, ctx);

        if let Some(req) = request {
            match req {
                soul_script::SystemRequest::Launch(idx) => self.launch_app(idx, ctx),
                soul_script::SystemRequest::LaunchById(id) => self.launch_by_id(&id, ctx),
                soul_script::SystemRequest::Return => self.go_back(ctx),
            }
        }
    }

    fn toggle_a11y(&mut self, ctx: &mut Ctx<'_>) {
        self.a11y_enabled = !self.a11y_enabled;
        if self.a11y_enabled {
            self.a11y_focus = Some(0);
            ctx.a11y.speak("Accessibility mode enabled");
            self.speak_focused(ctx);
        } else {
            self.a11y_focus = None;
            ctx.a11y.speak("Accessibility mode disabled");
        }
        ctx.invalidate_all();
    }
}

impl App for Host {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        if matches!(event, Event::AppStop) {
            for slot in &mut self.apps {
                slot.persist();
            }
            self.dispatch_event(event, ctx);
            return;
        }

        if matches!(event, Event::Key(KeyCode::Tab)) {
            self.toggle_a11y(ctx);
            return;
        }

        match event {
            Event::PenDown { x, y } => {
                self.pen_start = Some((x, y, ctx.now_ms));
                if (y as i32) >= STRIP_TOP {
                    self.strip_pressed = true;
                }
                if self.a11y_enabled {
                    return;
                }
            }
            Event::PenUp { x, y } => {
                if let Some((x0, y0, _)) = self.pen_start.take() {
                    let dx = x - x0;
                    let dy = y - y0;

                    if self.a11y_enabled && dx.abs() > 40 && dy.abs() < 40 {
                        let nodes = self.active_a11y_nodes();
                        if !nodes.is_empty() {
                            let mut idx = self.a11y_focus.unwrap_or(0);
                            if dx > 0 {
                                idx = (idx + 1) % nodes.len();
                            } else if idx == 0 {
                                idx = nodes.len() - 1;
                            } else {
                                idx -= 1;
                            }
                            self.a11y_focus = Some(idx);
                            self.speak_focused(ctx);
                            ctx.invalidate_all();
                        }
                        return;
                    } else if dx.abs() < 10 && dy.abs() < 10 {
                        let mut triple_tap = false;
                        if let Some((lx, ly, lt)) = self.last_tap {
                            if (x - lx).abs() < 15 && (y - ly).abs() < 15 && (ctx.now_ms - lt) < 400
                            {
                                self.tap_count += 1;
                                if self.tap_count >= 3 {
                                    triple_tap = true;
                                    self.tap_count = 0;
                                }
                            } else {
                                self.tap_count = 1;
                            }
                        } else {
                            self.tap_count = 1;
                        }
                        self.last_tap = Some((x, y, ctx.now_ms));

                        if triple_tap {
                            self.toggle_a11y(ctx);
                            return;
                        }
                        if self.a11y_enabled {
                            if self.tap_count == 2 {
                                self.activate_focused(ctx);
                            }
                            return;
                        }
                    }
                }

                if self.strip_pressed {
                    self.strip_pressed = false;
                    if hit_test(&strip_home_rect(), x, y) {
                        self.go_home(ctx);
                    } else if hit_test(&strip_menu_rect(), x, y) {
                        self.dispatch_event(Event::Menu, ctx);
                    }
                    return;
                }
            }
            _ => {}
        }

        if self.a11y_enabled {
            return;
        }

        // Home button: deliver event to the active app first (so it can save
        // state), then enforce kernel-level navigation back to the home app.
        if matches!(event, Event::ButtonDown(HardButton::Home)) {
            self.dispatch_event(event, ctx);
            self.go_home(ctx);
            return;
        }

        self.dispatch_event(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let active = self.active_idx();
        self.apps[active].draw(canvas);
        let label = self.active_label().to_string();
        draw_system_strip(canvas, &label);

        if self.a11y_enabled {
            if let Some(idx) = self.a11y_focus {
                let nodes = self.active_a11y_nodes();
                if let Some(node) = nodes.get(idx) {
                    let _ = node
                        .bounds
                        .into_styled(PrimitiveStyle::with_stroke(BLACK, 2))
                        .draw(canvas);
                    let inner = Rectangle::new(
                        node.bounds.top_left + Point::new(1, 1),
                        node.bounds.size.saturating_sub(Size::new(2, 2)),
                    );
                    let _ = inner
                        .into_styled(PrimitiveStyle::with_stroke(WHITE, 1))
                        .draw(canvas);
                }
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();
    log::info!("🚀 SoulOS starting up...");

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 && args[1] == "--test" {
        run_headless_test(&args[2]);
        return;
    }

    let mut platform = HostedPlatform::new("SoulOS", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    run(&mut platform, Host::new());
}

fn run_headless_test(name: &str) {
    use soul_hal_hosted::testing::{scenarios, TestingPlatform};
    let scenario = match name {
        "build-todo" => scenarios::build_todo_app(),
        "verify-todo" => scenarios::verify_todo_app(),
        _ => {
            eprintln!("Unknown test scenario: {}", name);
            return;
        }
    };

    println!("Running headless test: {}", scenario.name);
    let platform = HostedPlatform::new("SoulOS Test", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    let mut host = Host::new();
    let mut dirty = soul_core::Dirty::full();
    let mut a11y = soul_core::a11y::A11yManager::new();

    for event in scenario.events {
        println!(
            "  → {} (Active: {})",
            event.description,
            host.active_label()
        );
        let mut ctx = soul_core::Ctx {
            now_ms: 0,
            dirty: &mut dirty,
            a11y: &mut a11y,
        };

        let core_event = match event.event {
            soul_hal::InputEvent::StylusDown { x, y } => soul_core::Event::PenDown { x, y },
            soul_hal::InputEvent::StylusMove { x, y } => soul_core::Event::PenMove { x, y },
            soul_hal::InputEvent::StylusUp { x, y } => soul_core::Event::PenUp { x, y },
            soul_hal::InputEvent::Key(k) => soul_core::Event::Key(k),
            soul_hal::InputEvent::ButtonDown(soul_hal::HardButton::Menu) => soul_core::Event::Menu,
            soul_hal::InputEvent::ButtonDown(b) => soul_core::Event::ButtonDown(b),
            soul_hal::InputEvent::ButtonUp(b) => soul_core::Event::ButtonUp(b),
            _ => continue,
        };

        host.handle(core_event, &mut ctx);
        let mut display = platform.get_display_buffer().clone();
        host.draw(&mut display);
        std::thread::sleep(std::time::Duration::from_millis(event.delay_ms));
    }
    println!("Headless test finished.");
}
