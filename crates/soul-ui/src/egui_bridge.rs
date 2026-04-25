use egui::{Context, Ui};

pub static mut ACTIVE_UI: Option<*mut egui::Ui> = None;

pub struct EguiRhaiBridge {
    context: Context,
}

impl EguiRhaiBridge {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub fn create_scroll_area(
        &self,
        _id: &str,
        max_height: f32,
        content_fn: impl FnOnce(&mut Ui),
    ) {
        unsafe {
            if let Some(ui_ptr) = ACTIVE_UI {
                let ui = &mut *ui_ptr;
                egui::ScrollArea::vertical()
                    .max_height(max_height)
                    .show(ui, |ui| {
                        let old_ui = ACTIVE_UI;
                        ACTIVE_UI = Some(ui as *mut Ui);
                        content_fn(ui);
                        ACTIVE_UI = old_ui;
                    });
            }
        }
    }

    pub fn group(&self, title: &str, content_fn: impl FnOnce(&mut Ui)) {
        unsafe {
            if let Some(ui_ptr) = ACTIVE_UI {
                let ui = &mut *ui_ptr;
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    if !title.is_empty() {
                        ui.heading(title);
                    }
                    let old_ui = ACTIVE_UI;
                    ACTIVE_UI = Some(ui as *mut Ui);
                    content_fn(ui);
                    ACTIVE_UI = old_ui;
                });
            }
        }
    }

    pub fn horizontal_layout(&self, content_fn: impl FnOnce(&mut Ui)) {
        unsafe {
            if let Some(ui_ptr) = ACTIVE_UI {
                let ui = &mut *ui_ptr;
                ui.horizontal(|ui| {
                    let old_ui = ACTIVE_UI;
                    ACTIVE_UI = Some(ui as *mut Ui);
                    content_fn(ui);
                    ACTIVE_UI = old_ui;
                });
            }
        }
    }

    pub fn vertical_layout(&self, content_fn: impl FnOnce(&mut Ui)) {
        unsafe {
            if let Some(ui_ptr) = ACTIVE_UI {
                let ui = &mut *ui_ptr;
                ui.vertical(|ui| {
                    let old_ui = ACTIVE_UI;
                    ACTIVE_UI = Some(ui as *mut Ui);
                    content_fn(ui);
                    ACTIVE_UI = old_ui;
                });
            }
        }
    }

    pub fn run(&self, content_fn: impl FnOnce(&mut Ui)) -> egui::FullOutput {
        let mut content_fn_opt = Some(content_fn);
        self.context.run(Default::default(), |ctx| {
            // Use Frame::none() to avoid drawing a background that would cover manual drawing
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    if let Some(cf) = content_fn_opt.take() {
                        let old_ui = unsafe { ACTIVE_UI };
                        unsafe { ACTIVE_UI = Some(ui as *mut Ui); }
                        cf(ui);
                        unsafe { ACTIVE_UI = old_ui; }
                    }
                });
        })
    }
}
