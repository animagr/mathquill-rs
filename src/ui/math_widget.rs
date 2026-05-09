//! The egui widget that wraps the math editor: renders via `RaTeX` and handles input.

use egui::{Key, TextureHandle, TextureOptions};

use crate::editor::Editor;
use super::renderer::{png_to_color_image, RenderCache};

const CURSOR_BLINK_INTERVAL_SECS: f64 = 0.53;

/// An interactive math editor widget for egui.
pub struct MathWidget {
    editor: Editor,
    render_cache: RenderCache,
    texture: Option<TextureHandle>,
    focused: bool,
    cursor_visible: bool,
    last_blink: f64,
}

impl MathWidget {
    /// Create a new math widget with an empty editor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            editor: Editor::new(),
            render_cache: RenderCache::new(),
            texture: None,
            focused: false,
            cursor_visible: true,
            last_blink: 0.0,
        }
    }

    /// Access the underlying editor.
    #[must_use]
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Mutable access to the underlying editor.
    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    /// Show the math widget in the given UI area.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = self.texture.as_ref().map_or(
            egui::vec2(200.0, 60.0),
            |tex| {
                let s = tex.size_vec2();
                egui::vec2(s.x * 0.5, s.y * 0.5)
            },
        );

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if response.clicked() {
            self.focused = true;
            self.cursor_visible = true;
            self.last_blink = ui.input(|i| i.time);
        }

        if self.focused && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.focused = false;
        }

        if self.focused {
            self.handle_input(ui);
            self.update_cursor_blink(ui);
            ui.ctx().request_repaint();
        }

        self.ensure_rendered(ui);

        if ui.is_rect_visible(rect) {
            if let Some(tex) = &self.texture {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
            } else {
                ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(240));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Click to edit",
                    egui::FontId::default(),
                    egui::Color32::GRAY,
                );
            }

            // Focus indicator.
            if self.focused {
                ui.painter().rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(66, 133, 244)),
                    egui::StrokeKind::Outside,
                );

                // Draw cursor indicator.
                if self.cursor_visible {
                    #[allow(clippy::cast_precision_loss)]
                    let cursor_x = rect.left() + 4.0 + (self.editor.cursor.seq_pos() as f32 * 8.0);
                    let cursor_x = cursor_x.min(rect.right() - 2.0);
                    ui.painter().line_segment(
                        [
                            egui::pos2(cursor_x, rect.top() + 4.0),
                            egui::pos2(cursor_x, rect.bottom() - 4.0),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(66, 133, 244)),
                    );
                }
            }
        }

        response
    }

    /// Handle keyboard input when focused.
    fn handle_input(&mut self, ui: &mut egui::Ui) {
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());

        for event in &events {
            match event {
                egui::Event::Text(text) => {
                    for ch in text.chars() {
                        self.editor.type_char(ch);
                    }
                    self.cursor_visible = true;
                    self.last_blink = ui.input(|i| i.time);
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    self.handle_key(*key, *modifiers);
                    self.cursor_visible = true;
                    self.last_blink = ui.input(|i| i.time);
                }
                _ => {}
            }
        }
    }

    /// Handle a single key press.
    fn handle_key(&mut self, key: Key, modifiers: egui::Modifiers) {
        match key {
            Key::ArrowLeft => self.editor.move_left(),
            Key::ArrowRight => self.editor.move_right(),
            Key::ArrowUp => self.editor.move_up(),
            Key::ArrowDown => self.editor.move_down(),
            Key::Backspace => self.editor.backspace(),
            Key::Delete => self.editor.delete_forward(),
            Key::Z if modifiers.command => {
                if modifiers.shift {
                    self.editor.redo();
                } else {
                    self.editor.undo();
                }
            }
            Key::Y if modifiers.command => self.editor.redo(),
            _ => {}
        }
    }

    /// Update cursor blink state.
    fn update_cursor_blink(&mut self, ui: &egui::Ui) {
        let now = ui.input(|i| i.time);
        if now - self.last_blink >= CURSOR_BLINK_INTERVAL_SECS {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = now;
        }
    }

    /// Ensure the rendered texture is up-to-date with the editor's LaTeX.
    fn ensure_rendered(&mut self, ui: &mut egui::Ui) {
        if !self.editor.is_dirty() && self.texture.is_some() {
            return;
        }

        let latex = self.editor.to_latex();
        match self.render_cache.render(&latex) {
            Ok(png_bytes) => {
                match png_to_color_image(png_bytes) {
                    Ok(image) => {
                        let tex = ui.ctx().load_texture(
                            "math-render",
                            image,
                            TextureOptions::LINEAR,
                        );
                        self.texture = Some(tex);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to decode PNG: {e}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to render LaTeX '{latex}': {e}");
            }
        }

        self.editor.mark_clean();
    }
}

impl Default for MathWidget {
    fn default() -> Self {
        Self::new()
    }
}
