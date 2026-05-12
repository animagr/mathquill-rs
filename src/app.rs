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

            ui.horizontal(|ui| {
                if ui
                    .button("a/b")
                    .on_hover_text("Fraction (or type /)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('/');
                }
                if ui
                    .button("x\u{207F}")
                    .on_hover_text("Superscript (or type ^)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('^');
                }
                if ui
                    .button("x\u{2099}")
                    .on_hover_text("Subscript (or type _)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('_');
                }
                if ui
                    .button("\u{221A}")
                    .on_hover_text("Square root (or type sqrt)")
                    .clicked()
                {
                    self.widget.editor_mut().insert_sqrt();
                }
                if ui
                    .button("\u{207F}\u{221A}")
                    .on_hover_text("Nth root")
                    .clicked()
                {
                    self.widget.editor_mut().insert_nth_root();
                }
                if ui
                    .button("( )")
                    .on_hover_text("Parentheses (or type ()")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('(');
                }
                if ui
                    .button("[ ]")
                    .on_hover_text("Square brackets (or type [)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('[');
                }
                if ui
                    .button("{ }")
                    .on_hover_text("Curly braces (or type {)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('{');
                }
                if ui
                    .button("| |")
                    .on_hover_text("Absolute value (or type | or abs)")
                    .clicked()
                {
                    self.widget.editor_mut().type_char('|');
                }
            });

            ui.separator();
            self.widget.show(ui);
            ui.separator();

            let latex = self.widget.editor().to_latex();
            ui.horizontal(|ui| {
                ui.label("LaTeX:");
                ui.monospace(&latex);
            });

            ui.add_space(8.0);
            ui.collapsing("Keyboard shortcuts", |ui| {
                ui.label("Tab / Shift+Tab: move between fields");
                ui.label("Home / End: jump to start/end");
                ui.label("Ctrl+Z / Ctrl+Shift+Z: undo/redo");
                ui.label("Ctrl+C / Ctrl+X / Ctrl+V: copy/cut/paste");
                ui.label("Escape: unfocus editor");
            });

            ui.collapsing("Auto-commands", |ui| {
                ui.label("Type these letter sequences to auto-convert:");
                ui.label("  Operators: sin, cos, tan, log, ln, lim, exp, ...");
                ui.label("  Greek: alpha, beta, pi, theta, sigma, omega, ...");
                ui.label("  Symbols: infty, leq, geq, neq, approx, pm, times, ...");
                ui.label("  Arrows: to, implies, iff, mapsto, gets");
                ui.label("  Sets: subset, supset, cup, cap, emptyset");
                ui.label("  Structures: sqrt, abs, sum, prod, int");
            });
        });
    }
}
