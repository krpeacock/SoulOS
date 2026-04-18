use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Gray8, prelude::*,
};
use soul_core::{App, Ctx, Event, SCREEN_WIDTH};
use soul_ui::{Form, title_bar, TITLE_BAR_H};
use std::path::PathBuf;

pub struct MyTodoApp {
    form: Form,
    db_path: PathBuf,
}

impl MyTodoApp {
    pub fn new() -> Self {
        let db_path = std::env::var("SOUL_TODO_UI_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".soulos/my_todo_ui.sdb"));

        Self {
            form: Self::load_ui(&db_path),
            db_path,
        }
    }

    fn load_ui(path: &std::path::Path) -> Form {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(db) = soul_db::Database::decode(&bytes) {
                if let Some(rec) = db.iter().next() {
                    if let Ok(json) = std::str::from_utf8(&rec.data) {
                        if let Some(form) = Form::from_json(json) {
                            return form;
                        }
                    }
                }
            }
        }
        Self::default_ui()
    }

    fn persist(&self) {
        if let Some(parent) = self.db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut db = soul_db::Database::new("todo_ui");
        db.insert(0, self.form.to_json().into_bytes());
        let _ = std::fs::write(&self.db_path, db.encode());
    }

    fn default_ui() -> Form {
        use soul_ui::{Component, ComponentType, Rect, A11yHints, Interaction, Trigger, Action};
        use std::collections::BTreeMap;
        let mut form = Form::new("my_todo");

        form.components.push(Component {
            id: "title".into(),
            class: "header".into(),
            type_: ComponentType::Label,
            bounds: Rect { x: 10, y: 20, w: 200, h: 20 },
            properties: BTreeMap::from([("text".into(), "My Todos".into())]),
            a11y: A11yHints { label: "Todo list title".into(), role: "heading".into() },
            interactions: Vec::new(),
            binding: None,
        });

        // Add some default tasks
        for i in 0..3 {
            form.components.push(Component {
                id: format!("todo_{}", i),
                class: "task".into(),
                type_: ComponentType::Checkbox,
                bounds: Rect { x: 10, y: 50 + i * 30, w: 220, h: 24 },
                properties: BTreeMap::from([
                    ("label".into(), format!("Task {}", i + 1).into()),
                    ("checked".into(), false.into())
                ]),
                a11y: A11yHints { label: format!("Task {}", i + 1), role: "checkbox".into() },
                interactions: vec![Interaction {
                    trigger: Trigger::OnTap,
                    action: Action::SaveRecord(0), // Placeholder for "toggle" logic
                }],
                binding: Some(format!("todos:{}", i)),
            });
        }

        form
    }
}

impl App for MyTodoApp {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::PenDown { x, y } => {
                if let Some(comp) = self.form.hit_test_mut(x, y) {
                    match comp.type_ {
                        soul_ui::ComponentType::Checkbox => {
                            let current = comp.properties.get("checked").and_then(|v| v.as_bool()).unwrap_or(false);
                            comp.properties.insert("checked".into(), (!current).into());
                            
                            if let Some(binding) = &comp.binding {
                                // In a real app, this would update a specific soul_db record
                                // based on the binding string (e.g. "todos:0")
                                eprintln!("Todo binding updated: {} -> {}", binding, !current);
                            }

                            self.persist();
                            ctx.invalidate_all();
                        }
                        soul_ui::ComponentType::Button => {
                            // Execute interactions
                            for inter in &comp.interactions {
                                if inter.trigger == soul_ui::Trigger::OnTap {
                                    match &inter.action {
                                        soul_ui::Action::CloseApp => {
                                            // How to close app? Maybe a dedicated event.
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "My Todo App");
        let _ = self.form.draw(canvas, None);
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        self.form.a11y_nodes()
    }
}
