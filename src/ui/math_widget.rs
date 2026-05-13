//! The egui widget that wraps the math editor: renders via `RaTeX` and handles input.

use egui::{Key, TextureHandle, TextureOptions};

use super::cursor_overlay::{cursor_for_point, cursor_position_for_path, cursor_position_for_rect};
use super::renderer::{png_to_color_image, RenderCache};
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::editor::Editor;

const CURSOR_BLINK_INTERVAL_SECS: f64 = 0.53;

/// An interactive math editor widget for egui.
pub struct MathWidget {
    editor: Editor,
    render_cache: RenderCache,
    texture: Option<TextureHandle>,
    focused: bool,
    cursor_visible: bool,
    last_blink: f64,
    drag_anticursor: Option<Cursor>,
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
            drag_anticursor: None,
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
        let desired_size = self
            .texture
            .as_ref()
            .map_or(egui::vec2(200.0, 60.0), |tex| {
                let s = tex.size_vec2();
                egui::vec2(s.x * 0.5, s.y * 0.5)
            });

        let (rect, response) =
            ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        self.ensure_rendered(ui);

        if response.drag_started() {
            self.focused = true;
            if let Some(point) = response.interact_pointer_pos() {
                if let Some(cursor) =
                    cursor_for_point(&self.editor, self.render_cache.last_rendered(), rect, point)
                {
                    self.editor.cursor = cursor.clone();
                    self.editor.selection = None;
                    self.drag_anticursor = Some(cursor);
                }
            }
            self.cursor_visible = true;
            self.last_blink = ui.input(|i| i.time);
        } else if response.dragged() {
            if let Some(point) = response.interact_pointer_pos() {
                if let Some(cursor) =
                    cursor_for_point(&self.editor, self.render_cache.last_rendered(), rect, point)
                {
                    self.editor.cursor = cursor;
                    self.update_drag_selection();
                }
            }
        } else if response.drag_stopped() {
            self.drag_anticursor = None;
        } else if response.clicked() {
            self.focused = true;
            if let Some(point) = response.interact_pointer_pos() {
                if let Some(cursor) =
                    cursor_for_point(&self.editor, self.render_cache.last_rendered(), rect, point)
                {
                    self.editor.cursor = cursor;
                    self.editor.selection = None;
                }
            }
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
                ui.painter()
                    .rect_filled(rect, 4.0, egui::Color32::from_gray(240));
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

                let cursor_pos = cursor_position_for_rect(
                    &self.editor,
                    self.render_cache.last_rendered(),
                    rect,
                );

                self.draw_selection_highlight(ui, rect, cursor_pos);

                if self.cursor_visible {
                    ui.painter().line_segment(
                        [
                            egui::pos2(cursor_pos.x(), cursor_pos.top()),
                            egui::pos2(cursor_pos.x(), cursor_pos.bottom()),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(66, 133, 244)),
                    );
                }
            }
        }

        response
    }

    /// Draw a translucent highlight rectangle over the selected range.
    fn draw_selection_highlight(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        cursor_pos: super::cursor_overlay::CursorPosition,
    ) {
        let Some(sel) = &self.editor.selection else {
            return;
        };
        if !sel.is_same_seq(&self.editor.cursor) || !sel.is_nonempty(self.editor.cursor.seq_pos())
        {
            return;
        }

        let anchor_cursor = Cursor::from_path({
            let mut path = sel.seq_path.clone();
            path.push(crate::editor::cursor::CursorStep::SeqPos(sel.anchor));
            path
        });
        let anchor_pos = cursor_position_for_path(
            &self.editor.root,
            &anchor_cursor,
            self.render_cache.last_rendered(),
            rect,
        );

        let left = cursor_pos.x().min(anchor_pos.x());
        let right = cursor_pos.x().max(anchor_pos.x());
        if (right - left) > 0.5 {
            let top = cursor_pos.top().min(anchor_pos.top());
            let bottom = cursor_pos.bottom().max(anchor_pos.bottom());
            let sel_rect = egui::Rect::from_min_max(
                egui::pos2(left, top),
                egui::pos2(right, bottom),
            );
            ui.painter().rect_filled(
                sel_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(190, 215, 244, 225),
            );
        }
    }

    /// Update selection state during a drag gesture (same-Seq only).
    fn update_drag_selection(&mut self) {
        let Some(anticursor) = &self.drag_anticursor else {
            return;
        };
        let sel = Selection::from_cursor(anticursor);
        if sel.is_same_seq(&self.editor.cursor) {
            self.editor.selection = Some(sel);
        }
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
                    self.handle_key(*key, *modifiers, ui);
                    self.cursor_visible = true;
                    self.last_blink = ui.input(|i| i.time);
                }
                egui::Event::Paste(text) => {
                    self.editor.paste_latex(text);
                    self.cursor_visible = true;
                    self.last_blink = ui.input(|i| i.time);
                }
                _ => {}
            }
        }
    }

    /// Handle a single key press.
    fn handle_key(&mut self, key: Key, modifiers: egui::Modifiers, ui: &egui::Ui) {
        match key {
            Key::ArrowLeft if modifiers.shift => self.editor.select_left(),
            Key::ArrowRight if modifiers.shift => self.editor.select_right(),
            Key::ArrowLeft => self.editor.move_left(),
            Key::ArrowRight => self.editor.move_right(),
            Key::ArrowUp => self.editor.move_up(),
            Key::ArrowDown => self.editor.move_down(),
            Key::Backspace => self.editor.backspace(),
            Key::Delete => self.editor.delete_forward(),
            Key::A if modifiers.command => self.editor.select_all(),
            Key::C if modifiers.command => {
                if let Some(latex) = self.editor.copy_selection_latex() {
                    ui.ctx().copy_text(latex);
                }
            }
            Key::X if modifiers.command => {
                if let Some(latex) = self.editor.copy_selection_latex() {
                    ui.ctx().copy_text(latex);
                    self.editor.delete_selection();
                }
            }
            Key::Z if modifiers.command => {
                if modifiers.shift {
                    self.editor.redo();
                } else {
                    self.editor.undo();
                }
            }
            Key::Y if modifiers.command => self.editor.redo(),
            Key::Home => self.editor.move_home(),
            Key::End => self.editor.move_end(),
            Key::Tab if modifiers.shift => self.editor.shift_tab(),
            Key::Tab => self.editor.tab(),
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
            Ok(rendered) => match png_to_color_image(rendered.png_bytes()) {
                Ok(image) => {
                    let tex = ui
                        .ctx()
                        .load_texture("math-render", image, TextureOptions::LINEAR);
                    self.texture = Some(tex);
                }
                Err(e) => {
                    tracing::warn!("Failed to decode PNG: {e}");
                }
            },
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
