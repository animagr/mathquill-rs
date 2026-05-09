//! Entry point: launches the eframe window with the math editor demo.

use eframe::egui;

mod app;

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_title("mathquill-rs"),
        ..Default::default()
    };

    eframe::run_native(
        "mathquill-rs",
        options,
        Box::new(|cc| Ok(Box::new(app::MathApp::new(cc)))),
    )
}
