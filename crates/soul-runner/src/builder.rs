use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_ui::{
    button, label, title_bar, A11yHints, Component, ComponentType, EditOverlay, Form, Rect,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::launcher::Launcher;

// ── State machine ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuilderState {
    /// Home screen: pick an existing app or create a new one.
    Home,
    /// Resource picker for a specific app (edit form / icon / script).
    ResourcePicker,
    /// Full form editor for a loaded form.
    EditingForm,
}

// ── Edit-field for form property editing ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Id,
    Class,
    Label,
    Binding,
}

// ── Menu definitions ──────────────────────────────────────────────────────────

const HOME_MENU: &[&str] = &[
    "Pick App",
    "New App",
];

const RESOURCE_MENU: &[&str] = &[
    "Edit Form",
    "Edit Icon",
    "Edit Script",
    "Back",
];

const FORM_MENU: &[&str] = &[
    "New Form",
    "Add Button",
    "Add Label",
    "Add Input",
    "Add Area",
    "Add Checkbox",
    "Edit Id",
    "Edit Class",
    "Edit Label",
    "Edit Binding",
    "Delete Element",
    "Save Form",
    "Back",
    "Close Menu",
];

// ── MobileBuilder ─────────────────────────────────────────────────────────────

pub struct MobileBuilder {
    state: BuilderState,
    /// ID of the app currently selected in the resource picker.
    selected_app_id: String,
    /// Display name of the selected app.
    selected_app_name: String,
    /// Form being edited in EditingForm mode.
    form: Form,
    edit_overlay: EditOverlay,
    db_path: PathBuf,
    menu_open: bool,
    menu_touch: Option<usize>,
    editing_value: Option<(soul_ui::TextInput, EditField)>,
    keyboard: soul_ui::Keyboard,
}

impl MobileBuilder {
    pub const APP_ID: &'static str = "com.soulos.builder";
    pub const NAME: &'static str = "Builder";

    pub fn new() -> Self {
        let db_path = std::env::var("SOUL_BUILDER_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".soulos/builder_temp.sdb"));

        Self {
            state: BuilderState::Home,
            selected_app_id: String::new(),
            selected_app_name: String::new(),
            form: Form::new("new_app"),
            edit_overlay: EditOverlay::new(),
            db_path,
            menu_open: false,
            menu_touch: None,
            editing_value: None,
            keyboard: soul_ui::Keyboard::new(APP_HEIGHT as i32 - soul_ui::KEYBOARD_HEIGHT as i32),
        }
    }

    pub fn persist(&self) {
        if self.state != BuilderState::EditingForm {
            return;
        }
        if let Some(parent) = self.db_path.parent() {
            let _ = crate::assets::create_dir_all(parent);
        }
        let mut db = soul_db::Database::new("builder_form");
        db.insert(0, self.form.to_json().into_bytes());
        let _ = crate::assets::write(&self.db_path, &db.encode());
    }

    // ── Exchange handling ────────────────────────────────────────────────────

    pub fn handle_event(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::AppStart => {
                // Return to home when re-activated so we don't get stuck in a sub-state.
                self.state = BuilderState::Home;
                ctx.invalidate_all();
                None
            }
            Event::Exchange { action, payload, .. } => {
                self.handle_exchange(&action, payload, ctx)
            }
            Event::Menu => {
                self.menu_open = !self.menu_open;
                ctx.invalidate_all();
                None
            }
            _ => match self.state {
                BuilderState::Home => self.handle_home(event, ctx),
                BuilderState::ResourcePicker => self.handle_resource_picker(event, ctx),
                BuilderState::EditingForm => self.handle_form_editor(event, ctx),
            },
        }
    }

    fn handle_exchange(
        &mut self,
        action: &str,
        payload: soul_core::ExchangePayload,
        ctx: &mut Ctx<'_>,
    ) -> Option<SystemRequest> {
        match action {
            // Launcher returned a picked app ID.
            "return_app" => {
                if let soul_core::ExchangePayload::Text(app_id) = payload {
                    self.selected_app_id = app_id.clone();
                    // Try to derive a display name from the app list.
                    self.selected_app_name = soul_script::app_list()
                        .iter()
                        .find(|e| e.app_id == app_id)
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| app_id.clone());
                    self.state = BuilderState::ResourcePicker;
                    ctx.invalidate_all();
                }
                None
            }
            // Launcher returned a resource (icon pixels or script text).
            "return_resource" => {
                match payload {
                    soul_core::ExchangePayload::Resource { kind, pixels, width, height, text, .. } => {
                        match kind.as_str() {
                            "icon" => {
                                // Open Draw with the fetched icon pixels.
                                Some(SystemRequest::Request {
                                    action: "open_bitmap".to_string(),
                                    payload: soul_core::ExchangePayload::Bitmap {
                                        width,
                                        height,
                                        pixels,
                                    },
                                })
                            }
                            "script" => {
                                // Open Notes with the fetched script text.
                                Some(SystemRequest::Request {
                                    action: "open_script".to_string(),
                                    payload: soul_core::ExchangePayload::Text(text),
                                })
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
            // Draw returned edited icon pixels → save back via Launcher.
            "return_bitmap" => {
                if let soul_core::ExchangePayload::Bitmap { width, height, pixels } = payload {
                    let app_id = self.selected_app_id.clone();
                    Some(SystemRequest::BackgroundSend {
                        action: "set_resource".to_string(),
                        payload: soul_core::ExchangePayload::Resource {
                            app_id,
                            kind: "icon".to_string(),
                            width,
                            height,
                            pixels,
                            text: String::new(),
                        },
                        target: Launcher::APP_ID.to_string(),
                    })
                } else {
                    None
                }
            }
            // Notes returned edited script text → save back via Launcher.
            "return_text" => {
                if let soul_core::ExchangePayload::Text(text) = payload {
                    let app_id = self.selected_app_id.clone();
                    Some(SystemRequest::BackgroundSend {
                        action: "set_resource".to_string(),
                        payload: soul_core::ExchangePayload::Resource {
                            app_id,
                            kind: "script".to_string(),
                            width: 0,
                            height: 0,
                            pixels: vec![],
                            text,
                        },
                        target: Launcher::APP_ID.to_string(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // ── Home state ───────────────────────────────────────────────────────────

    fn home_button_rect(i: usize) -> Rectangle {
        let y = 50 + i as i32 * 40;
        Rectangle::new(Point::new(40, y), Size::new(160, 30))
    }

    fn handle_home(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::PenDown { x, y } => {
                self.menu_touch = (0..HOME_MENU.len())
                    .find(|&i| soul_ui::hit_test(&Self::home_button_rect(i), x, y));
                ctx.invalidate_all();
                None
            }
            Event::PenUp { x, y } => {
                if let Some(i) = self.menu_touch.take() {
                    if soul_ui::hit_test(&Self::home_button_rect(i), x, y) {
                        ctx.invalidate_all();
                        return self.home_action(i, ctx);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn home_action(&mut self, idx: usize, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match idx {
            0 => {
                // "Pick App" — request the Launcher to show the picker.
                Some(SystemRequest::Request {
                    action: "pick_app".to_string(),
                    payload: soul_core::ExchangePayload::Text(String::new()),
                })
            }
            1 => {
                // "New App" — jump straight to the form editor with a blank form.
                self.selected_app_id.clear();
                self.selected_app_name = "new app".to_string();
                self.form = Form::new("new_app");
                self.edit_overlay = EditOverlay::new();
                self.state = BuilderState::EditingForm;
                ctx.invalidate_all();
                None
            }
            _ => None,
        }
    }

    // ── Resource picker state ────────────────────────────────────────────────

    fn resource_button_rect(i: usize) -> Rectangle {
        let y = 50 + i as i32 * 40;
        Rectangle::new(Point::new(40, y), Size::new(160, 30))
    }

    fn handle_resource_picker(
        &mut self,
        event: Event,
        ctx: &mut Ctx<'_>,
    ) -> Option<SystemRequest> {
        match event {
            Event::PenDown { x, y } => {
                self.menu_touch = (0..RESOURCE_MENU.len())
                    .find(|&i| soul_ui::hit_test(&Self::resource_button_rect(i), x, y));
                ctx.invalidate_all();
                None
            }
            Event::PenUp { x, y } => {
                if let Some(i) = self.menu_touch.take() {
                    if soul_ui::hit_test(&Self::resource_button_rect(i), x, y) {
                        ctx.invalidate_all();
                        return self.resource_action(i, ctx);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn resource_action(&mut self, idx: usize, _ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        let app_id = self.selected_app_id.clone();
        match idx {
            0 => {
                // "Edit Form" — for now enter the local form editor.
                // TODO: load form from app resource via get_resource "form".
                self.state = BuilderState::EditingForm;
                None
            }
            1 => {
                // "Edit Icon" — fetch icon pixels from Launcher, then open Draw.
                Some(SystemRequest::BackgroundSend {
                    action: "get_resource".to_string(),
                    payload: soul_core::ExchangePayload::Resource {
                        app_id,
                        kind: "icon".to_string(),
                        width: 0,
                        height: 0,
                        pixels: vec![],
                        text: String::new(),
                    },
                    target: Launcher::APP_ID.to_string(),
                })
            }
            2 => {
                // "Edit Script" — fetch script source from Launcher, then open Notes.
                Some(SystemRequest::BackgroundSend {
                    action: "get_resource".to_string(),
                    payload: soul_core::ExchangePayload::Resource {
                        app_id,
                        kind: "script".to_string(),
                        width: 0,
                        height: 0,
                        pixels: vec![],
                        text: String::new(),
                    },
                    target: Launcher::APP_ID.to_string(),
                })
            }
            3 => {
                // "Back" — return to Home.
                self.state = BuilderState::Home;
                None
            }
            _ => None,
        }
    }

    // ── Form editor state ────────────────────────────────────────────────────

    fn menu_item_rect(i: usize) -> Rectangle {
        let col = (i % 2) as i32;
        let row = (i / 2) as i32;
        Rectangle::new(
            Point::new(15 + col * 105, 25 + row * 24),
            Size::new(100, 22),
        )
    }

    fn start_editing(&mut self, field: EditField) {
        if let Some(id) = &self.edit_overlay.selected_id {
            if let Some(comp) = self.form.components.iter().find(|c| &c.id == id) {
                let mut input = soul_ui::TextInput::with_placeholder(
                    Rectangle::new(Point::new(20, 100), Size::new(200, 24)),
                    match field {
                        EditField::Id => "id",
                        EditField::Class => "class",
                        EditField::Label => "label",
                        EditField::Binding => "binding",
                    },
                );
                let current = match field {
                    EditField::Id => comp.id.clone(),
                    EditField::Class => comp.class.clone(),
                    EditField::Label => comp
                        .properties
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    EditField::Binding => comp.binding.clone().unwrap_or_default(),
                };
                let _ = input.set_text(current);
                self.editing_value = Some((input, field));
            }
        }
    }

    fn add_component(&mut self, type_: ComponentType) {
        let id = format!(
            "{}_{}",
            match type_ {
                ComponentType::Button => "btn",
                ComponentType::Label => "lbl",
                ComponentType::TextInput => "input",
                ComponentType::TextArea => "area",
                ComponentType::Canvas => "canvas",
                ComponentType::Checkbox => "check",
            },
            self.form.components.len()
        );

        let mut properties = BTreeMap::new();
        let label_text = match type_ {
            ComponentType::Button => "Button",
            ComponentType::Label => "Label",
            ComponentType::Checkbox => "Task",
            _ => "",
        };
        if !label_text.is_empty() {
            properties.insert("label".into(), label_text.into());
            properties.insert("text".into(), label_text.into());
        }
        if type_ == ComponentType::Checkbox {
            properties.insert("checked".into(), false.into());
        }

        let count = self.form.components.len() as i32;
        let offset = (count % 5) * 10;

        self.form.components.push(Component {
            id: id.clone(),
            class: String::new(),
            type_,
            bounds: Rect {
                x: 20 + offset,
                y: 40 + offset,
                w: 80,
                h: 24,
            },
            properties,
            a11y: A11yHints {
                label: id,
                role: "widget".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        self.edit_overlay.selected_id = Some(self.form.components.last().unwrap().id.clone());
    }

    fn handle_form_editor(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        // Text editing overlay intercepts all events when active.
        if let Some((input, field)) = &mut self.editing_value {
            match event {
                Event::Key(soul_core::KeyCode::Char(c)) => {
                    ctx.a11y.speak(&c.to_string());
                    let _ = input.insert_char(c);
                    ctx.invalidate_all();
                    return None;
                }
                Event::Key(soul_core::KeyCode::Backspace) => {
                    let _ = input.backspace();
                    ctx.invalidate_all();
                    return None;
                }
                Event::Key(soul_core::KeyCode::Enter) => {
                    let new_val = input.text().to_string();
                    let old_id = self.edit_overlay.selected_id.clone();
                    if let Some(id) = old_id {
                        if let Some(comp) = self.form.components.iter_mut().find(|c| c.id == id) {
                            match field {
                                EditField::Id => comp.id = new_val.clone(),
                                EditField::Class => comp.class = new_val.clone(),
                                EditField::Label => {
                                    comp.properties.insert("label".into(), new_val.clone().into());
                                    comp.properties.insert("text".into(), new_val.clone().into());
                                }
                                EditField::Binding => comp.binding = Some(new_val.clone()),
                            }
                            if matches!(field, EditField::Id) {
                                self.edit_overlay.selected_id = Some(new_val);
                            }
                        }
                    }
                    self.editing_value = None;
                    ctx.invalidate_all();
                    return None;
                }
                Event::PenDown { x, y } => {
                    if (y as i32) >= (APP_HEIGHT as i32 - soul_ui::KEYBOARD_HEIGHT as i32) {
                        let out = self.keyboard.pen_moved(x, y);
                        if let Some(r) = out {
                            ctx.invalidate(r);
                        }
                    } else if !input.contains(x, y) {
                        self.editing_value = None;
                        ctx.invalidate_all();
                    }
                    return None;
                }
                Event::PenMove { x, y } => {
                    if (y as i32) >= (APP_HEIGHT as i32 - soul_ui::KEYBOARD_HEIGHT as i32) {
                        let out = self.keyboard.pen_moved(x, y);
                        if let Some(r) = out {
                            ctx.invalidate(r);
                        }
                    }
                    return None;
                }
                Event::PenUp { x, y } => {
                    if (y as i32) >= (APP_HEIGHT as i32 - soul_ui::KEYBOARD_HEIGHT as i32) {
                        let out = self.keyboard.pen_released(x, y);
                        if let Some(r) = out.dirty {
                            ctx.invalidate(r);
                        }
                        if let Some(typed) = out.typed {
                            match typed {
                                soul_ui::TypedKey::Char(c) => {
                                    ctx.a11y.speak(&c.to_string());
                                    let _ = input.insert_char(c);
                                }
                                soul_ui::TypedKey::Backspace => {
                                    let _ = input.backspace();
                                }
                                soul_ui::TypedKey::Enter => {
                                    let new_val = input.text().to_string();
                                    let old_id = self.edit_overlay.selected_id.clone();
                                    if let Some(id) = old_id {
                                        if let Some(comp) = self.form.components.iter_mut().find(|c| c.id == id) {
                                            match field {
                                                EditField::Id => comp.id = new_val.clone(),
                                                EditField::Class => comp.class = new_val.clone(),
                                                EditField::Label => {
                                                    comp.properties.insert("label".into(), new_val.clone().into());
                                                    comp.properties.insert("text".into(), new_val.clone().into());
                                                }
                                                EditField::Binding => comp.binding = Some(new_val.clone()),
                                            }
                                            if matches!(field, EditField::Id) {
                                                self.edit_overlay.selected_id = Some(new_val);
                                            }
                                        }
                                    }
                                    self.editing_value = None;
                                }
                            }
                            ctx.invalidate_all();
                        }
                    }
                    return None;
                }
                _ => {}
            }
        }

        match event {
            Event::PenDown { x, y } => {
                if self.menu_open {
                    self.menu_touch = (0..FORM_MENU.len())
                        .find(|&i| soul_ui::hit_test(&Self::menu_item_rect(i), x, y));
                    ctx.invalidate_all();
                    return None;
                }
                if self.edit_overlay.pen_down(&self.form, x, y) {
                    ctx.invalidate_all();
                }
            }
            Event::PenMove { x, y } => {
                if !self.menu_open
                    && self.edit_overlay.pen_move(&mut self.form, x, y) {
                        ctx.invalidate_all();
                    }
            }
            Event::PenUp { x, y } => {
                if self.menu_open {
                    if let Some(i) = self.menu_touch.take() {
                        if soul_ui::hit_test(&Self::menu_item_rect(i), x, y) {
                            return self.form_menu_action(i, ctx);
                        }
                    }
                    self.menu_touch = None;
                    return None;
                }
                self.edit_overlay.pen_up();
                ctx.invalidate_all();
            }
            _ => {}
        }
        None
    }

    fn form_menu_action(&mut self, idx: usize, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        self.menu_open = false;
        ctx.invalidate_all();
        match idx {
            0 => { self.form = Form::new("new_app"); None }
            1 => { self.add_component(ComponentType::Button); None }
            2 => { self.add_component(ComponentType::Label); None }
            3 => { self.add_component(ComponentType::TextInput); None }
            4 => { self.add_component(ComponentType::TextArea); None }
            5 => { self.add_component(ComponentType::Checkbox); None }
            6 => { self.start_editing(EditField::Id); None }
            7 => { self.start_editing(EditField::Class); None }
            8 => { self.start_editing(EditField::Label); None }
            9 => { self.start_editing(EditField::Binding); None }
            10 => { self.edit_overlay.delete_selected(&mut self.form); None }
            11 => { self.persist(); None }
            12 => {
                // "Back" — return to resource picker (or home if no app selected).
                self.state = if self.selected_app_id.is_empty() {
                    BuilderState::Home
                } else {
                    BuilderState::ResourcePicker
                };
                None
            }
            _ => None,
        }
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    fn draw_home<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, Self::NAME);
        let _ = label(canvas, Point::new(10, 35), "Select or create an app:");
        for i in 0..HOME_MENU.len() {
            let pressed = self.menu_touch == Some(i);
            let _ = button(canvas, Self::home_button_rect(i), HOME_MENU[i], pressed);
        }
    }

    fn draw_resource_picker<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, Self::NAME);
        let name_line = format!("App: {}", self.selected_app_name);
        let _ = label(canvas, Point::new(10, 35), &name_line);
        for i in 0..RESOURCE_MENU.len() {
            let pressed = self.menu_touch == Some(i);
            let _ = button(canvas, Self::resource_button_rect(i), RESOURCE_MENU[i], pressed);
        }
    }

    fn draw_form_editor<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, Self::NAME);
        let _ = self.form.draw(canvas, None);
        let _ = self.edit_overlay.draw(canvas, &self.form);

        if let Some(id) = &self.edit_overlay.selected_id {
            if let Some(comp) = self.form.components.iter().find(|c| &c.id == id) {
                let bind = comp.binding.as_deref().unwrap_or("none");
                let info = format!(
                    "{} ({}): {},{} {}x{} [b:{}]",
                    comp.id,
                    comp.class,
                    comp.bounds.x,
                    comp.bounds.y,
                    comp.bounds.w,
                    comp.bounds.h,
                    bind
                );
                let _ = label(canvas, Point::new(4, APP_HEIGHT as i32 - 12), &info);
            }
        }

        if self.menu_open {
            let rect = Rectangle::new(Point::new(10, 15), Size::new(220, 224));
            let _ = rect
                .into_styled(PrimitiveStyle::with_fill(soul_ui::WHITE))
                .draw(canvas);
            let _ = rect
                .into_styled(PrimitiveStyle::with_stroke(soul_ui::BLACK, 1))
                .draw(canvas);
            for i in 0..FORM_MENU.len() {
                let pressed = self.menu_touch == Some(i);
                let _ = button(canvas, Self::menu_item_rect(i), FORM_MENU[i], pressed);
            }
        }

        if let Some((input, field)) = &mut self.editing_value {
            let rect = Rectangle::new(Point::new(10, 20), Size::new(220, 60));
            let _ = rect
                .into_styled(PrimitiveStyle::with_fill(soul_ui::WHITE))
                .draw(canvas);
            let _ = rect
                .into_styled(PrimitiveStyle::with_stroke(soul_ui::BLACK, 1))
                .draw(canvas);
            let title = match field {
                EditField::Id => "Edit Id:",
                EditField::Class => "Edit Class:",
                EditField::Label => "Edit Label:",
                EditField::Binding => "Edit Binding:",
            };
            let _ = label(canvas, Point::new(20, 25), title);
            let _ = input.draw(canvas);
            let _ = self.keyboard.draw(canvas);
        }
    }
}

// ── App trait impl ────────────────────────────────────────────────────────────

impl App for MobileBuilder {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        self.handle_event(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        match self.state {
            BuilderState::Home => self.draw_home(canvas),
            BuilderState::ResourcePicker => self.draw_resource_picker(canvas),
            BuilderState::EditingForm => self.draw_form_editor(canvas),
        }
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        use soul_core::a11y::{A11yNode, A11yRole};
        match self.state {
            BuilderState::Home => HOME_MENU
                .iter()
                .enumerate()
                .map(|(i, &label)| {
                    A11yNode::new(Self::home_button_rect(i), label, A11yRole::Button)
                })
                .collect(),
            BuilderState::ResourcePicker => RESOURCE_MENU
                .iter()
                .enumerate()
                .map(|(i, &label)| {
                    A11yNode::new(Self::resource_button_rect(i), label, A11yRole::Button)
                })
                .collect(),
            BuilderState::EditingForm => {
                let mut nodes = self.form.a11y_nodes();
                if self.menu_open {
                    for i in 0..FORM_MENU.len() {
                        nodes.push(A11yNode::new(
                            Self::menu_item_rect(i),
                            FORM_MENU[i],
                            A11yRole::MenuItem,
                        ));
                    }
                }
                nodes
            }
        }
    }
}
