//! Demo application: a single math widget in an eframe window.

use eframe::egui;
use mathquill_rs::ui::MathWidget;

pub struct MathApp {
    widget: MathWidget,
}

impl MathApp {
    #[must_use]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            widget: MathWidget::new(),
        }
    }
}

impl eframe::App for MathApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("mathquill-rs");
            ui.separator();
            self.widget.show(ui);
            ui.separator();
            ui.label(format!("LaTeX: {}", self.widget.editor().to_latex()));
        });
    }
}
