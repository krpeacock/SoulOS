use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH};
use soul_ui::{
    button, label, title_bar, A11yHints, Component, ComponentType, EditOverlay, Form, Rect,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Id,
    Class,
    Label,
    Binding,
}

pub struct MobileBuilder {
    form: Form,
    edit_overlay: EditOverlay,
    db_path: PathBuf,
    menu_open: bool,
    menu_touch: Option<usize>,
    editing_value: Option<(soul_ui::TextInput, EditField)>,
    keyboard: soul_ui::Keyboard,
}

const BUILDER_MENU: &[&str] = &[
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
    "Close Menu",
];

impl MobileBuilder {
    pub const APP_ID: &'static str = "com.soulos.builder";
    pub const NAME: &'static str = "Builder";

    pub fn new() -> Self {
        let db_path = std::env::var("SOUL_BUILDER_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".soulos/builder_temp.sdb"));

        Self {
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
        if let Some(parent) = self.db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut db = soul_db::Database::new("builder_form");
        db.insert(0, self.form.to_json().into_bytes());
        let _ = std::fs::write(&self.db_path, db.encode());
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
}

impl App for MobileBuilder {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        if let Some((input, field)) = &mut self.editing_value {
            match event {
                Event::Key(soul_core::KeyCode::Char(c)) => {
                    ctx.a11y.speak(&c.to_string());
                    let _ = input.insert_char(c);
                    ctx.invalidate_all();
                    return;
                }
                Event::Key(soul_core::KeyCode::Backspace) => {
                    let _ = input.backspace();
                    ctx.invalidate_all();
                    return;
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
                                    comp.properties
                                        .insert("label".into(), new_val.clone().into());
                                    comp.properties
                                        .insert("text".into(), new_val.clone().into());
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
                    return;
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
                    return;
                }
                Event::PenMove { x, y } => {
                    if (y as i32) >= (APP_HEIGHT as i32 - soul_ui::KEYBOARD_HEIGHT as i32) {
                        let out = self.keyboard.pen_moved(x, y);
                        if let Some(r) = out {
                            ctx.invalidate(r);
                        }
                    }
                    return;
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
                                        if let Some(comp) =
                                            self.form.components.iter_mut().find(|c| c.id == id)
                                        {
                                            match field {
                                                EditField::Id => comp.id = new_val.clone(),
                                                EditField::Class => comp.class = new_val.clone(),
                                                EditField::Label => {
                                                    comp.properties.insert(
                                                        "label".into(),
                                                        new_val.clone().into(),
                                                    );
                                                    comp.properties.insert(
                                                        "text".into(),
                                                        new_val.clone().into(),
                                                    );
                                                }
                                                EditField::Binding => {
                                                    comp.binding = Some(new_val.clone())
                                                }
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
                    return;
                }
                _ => {}
            }
        }

        match event {
            Event::Menu => {
                self.menu_open = !self.menu_open;
                ctx.invalidate_all();
            }
            Event::PenDown { x, y } => {
                if self.menu_open {
                    self.menu_touch = (0..BUILDER_MENU.len())
                        .find(|&i| soul_ui::hit_test(&Self::menu_item_rect(i), x, y));
                    ctx.invalidate_all();
                    return;
                }
                if self.edit_overlay.pen_down(&self.form, x, y) {
                    ctx.invalidate_all();
                }
            }
            Event::PenMove { x, y } => {
                if !self.menu_open {
                    if self.edit_overlay.pen_move(&mut self.form, x, y) {
                        ctx.invalidate_all();
                    }
                }
            }
            Event::PenUp { x, y } => {
                if self.menu_open {
                    if let Some(i) = self.menu_touch {
                        let hit = soul_ui::hit_test(&Self::menu_item_rect(i), x, y);
                        if hit {
                            match i {
                                0 => self.form = Form::new("new_app"),
                                1 => self.add_component(ComponentType::Button),
                                2 => self.add_component(ComponentType::Label),
                                3 => self.add_component(ComponentType::TextInput),
                                4 => self.add_component(ComponentType::TextArea),
                                5 => self.add_component(ComponentType::Checkbox),
                                6 => self.start_editing(EditField::Id),
                                7 => self.start_editing(EditField::Class),
                                8 => self.start_editing(EditField::Label),
                                9 => self.start_editing(EditField::Binding),
                                10 => {
                                    self.edit_overlay.delete_selected(&mut self.form);
                                }
                                11 => self.persist(),
                                _ => {}
                            }
                            self.menu_open = false;
                            ctx.invalidate_all();
                        }
                    }
                    self.menu_touch = None;
                    return;
                }
                self.edit_overlay.pen_up();
                ctx.invalidate_all();
            }
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "MobileBuilder");
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
            let rect = Rectangle::new(Point::new(10, 15), Size::new(220, 210));
            let _ = rect
                .into_styled(PrimitiveStyle::with_fill(soul_ui::WHITE))
                .draw(canvas);
            let _ = rect
                .into_styled(PrimitiveStyle::with_stroke(soul_ui::BLACK, 1))
                .draw(canvas);
            for i in 0..BUILDER_MENU.len() {
                let pressed = self.menu_touch == Some(i);
                let _ = button(canvas, Self::menu_item_rect(i), BUILDER_MENU[i], pressed);
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

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        let mut nodes = self.form.a11y_nodes();
        if self.menu_open {
            for i in 0..BUILDER_MENU.len() {
                nodes.push(soul_core::a11y::A11yNode {
                    bounds: Self::menu_item_rect(i),
                    label: BUILDER_MENU[i].to_string(),
                    role: "menuitem".into(),
                });
            }
        }
        nodes
    }
}
