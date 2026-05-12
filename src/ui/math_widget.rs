//! The egui widget that wraps the math editor: renders via `RaTeX` and handles input.

use egui::{Key, TextureHandle, TextureOptions};

use super::renderer::{png_to_color_image, RenderCache};
use crate::editor::{
    cursor::{Cursor, CursorStep},
    tree::MathNode,
    Editor,
};

const CURSOR_BLINK_INTERVAL_SECS: f64 = 0.53;
const RENDER_PADDING: f32 = 10.0;
const MIN_VISUAL_WIDTH: f32 = 1.0;
const EMPTY_VISUAL_OFFSET: f32 = 0.0;
const STRUCTURE_EXTRA_WIDTH: f32 = 1.5;
const SCRIPT_WIDTH_SCALE: f32 = 0.65;
const PARENS_EXTRA_WIDTH: f32 = 2.0;
const FRACTION_SLOT_OFFSET: f32 = 0.75;
const SQRT_INDEX_OFFSET: f32 = 0.5;
const INNER_CONTENT_OFFSET: f32 = 1.0;
const CURSOR_PATH_SLOT_OFFSET: usize = 1;
const CURSOR_PATH_SLOT_STRIDE: usize = 2;

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
        let desired_size = self
            .texture
            .as_ref()
            .map_or(egui::vec2(200.0, 60.0), |tex| {
                let s = tex.size_vec2();
                egui::vec2(s.x * 0.5, s.y * 0.5)
            });

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

                // Draw cursor indicator.
                if self.cursor_visible {
                    let cursor_x = cursor_x_for_rect(&self.editor, rect);
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
            Ok(png_bytes) => match png_to_color_image(png_bytes) {
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

fn cursor_x_for_rect(editor: &Editor, rect: egui::Rect) -> f32 {
    let total_width = visual_width(&editor.root).max(MIN_VISUAL_WIDTH);
    let cursor_offset = cursor_visual_offset(&editor.root, &editor.cursor)
        .unwrap_or_else(|| usize_to_f32(editor.cursor.seq_pos()));
    let content_left = rect.left() + RENDER_PADDING;
    let content_right = rect.right() - RENDER_PADDING;

    if content_right <= content_left {
        return rect.left();
    }

    let ratio = (cursor_offset / total_width).clamp(0.0, 1.0);
    content_left + ((content_right - content_left) * ratio)
}

fn cursor_visual_offset(root: &MathNode, cursor: &Cursor) -> Option<f32> {
    let mut node = root;
    let mut offset = EMPTY_VISUAL_OFFSET;
    let path = cursor.path();
    let mut i = 0;

    while i < path.len() {
        let CursorStep::SeqPos(pos) = path[i] else {
            return None;
        };
        let children = node.as_seq()?;
        offset += children.iter().take(pos).map(visual_width).sum::<f32>();

        if i == path.len() - 1 {
            return Some(offset);
        }

        let child = children.get(pos)?;
        let slot = path.get(i + CURSOR_PATH_SLOT_OFFSET).copied()?;
        offset += slot_visual_offset(child, slot);
        node = child_for_slot(child, slot)?;
        i += CURSOR_PATH_SLOT_STRIDE;
    }

    None
}

fn visual_width(node: &MathNode) -> f32 {
    match node {
        MathNode::Seq(children) => children.iter().map(visual_width).sum(),
        MathNode::Symbol(data) => usize_to_f32(data.ch.chars().count().max(1)),
        MathNode::Text(text) => usize_to_f32(text.chars().count().max(1)),
        MathNode::Fraction { num, den } => {
            visual_width(num).max(visual_width(den)) + STRUCTURE_EXTRA_WIDTH
        }
        MathNode::Sqrt { index, radicand } => {
            index.as_deref().map_or(EMPTY_VISUAL_OFFSET, visual_width)
                + visual_width(radicand)
                + STRUCTURE_EXTRA_WIDTH
        }
        MathNode::Sup { base, exp } => {
            visual_width(base) + (visual_width(exp) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::Sub { base, script } => {
            visual_width(base) + (visual_width(script) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::SupSub { base, sup, sub } => {
            visual_width(base) + (visual_width(sup).max(visual_width(sub)) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::Parens { body, .. } => visual_width(body) + PARENS_EXTRA_WIDTH,
        MathNode::Style { body, .. } => visual_width(body),
    }
}

fn slot_visual_offset(node: &MathNode, slot: CursorStep) -> f32 {
    match (node, slot) {
        (MathNode::Fraction { .. }, CursorStep::Numerator | CursorStep::Denominator) => {
            FRACTION_SLOT_OFFSET
        }
        (MathNode::Sqrt { index, .. }, CursorStep::Radicand) => {
            index.as_deref().map_or(EMPTY_VISUAL_OFFSET, visual_width) + STRUCTURE_EXTRA_WIDTH
        }
        (MathNode::Sqrt { .. }, CursorStep::Index) => SQRT_INDEX_OFFSET,
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Exponent | CursorStep::Subscript,
        ) => visual_width(base),
        (MathNode::Parens { .. }, CursorStep::Inner) => INNER_CONTENT_OFFSET,
        _ => EMPTY_VISUAL_OFFSET,
    }
}

#[allow(clippy::cast_precision_loss)]
fn usize_to_f32(value: usize) -> f32 {
    value as f32
}

fn child_for_slot(node: &MathNode, slot: CursorStep) -> Option<&MathNode> {
    match (node, slot) {
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => Some(num),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => Some(den),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => Some(radicand),
        (
            MathNode::Sqrt {
                index: Some(index), ..
            },
            CursorStep::Index,
        ) => Some(index),
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Base,
        ) => Some(base),
        (MathNode::Sup { exp, .. } | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => {
            Some(exp)
        }
        (
            MathNode::Sub { script, .. } | MathNode::SupSub { sub: script, .. },
            CursorStep::Subscript,
        ) => Some(script),
        (MathNode::Parens { body, .. } | MathNode::Style { body, .. }, CursorStep::Inner) => {
            Some(body)
        }
        _ => None,
    }
}

impl Default for MathWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{cursor_visual_offset, visual_width, EMPTY_VISUAL_OFFSET};
    use crate::editor::Editor;

    #[test]
    fn cursor_offset_moves_across_root_sequence() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');

        let offset = cursor_visual_offset(&editor.root, &editor.cursor).unwrap();

        assert!(offset > EMPTY_VISUAL_OFFSET);
        assert!((offset - visual_width(&editor.root)).abs() < f32::EPSILON);
    }

    #[test]
    fn cursor_offset_accounts_for_parent_nodes() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('+');
        editor.type_char('b');
        editor.type_char('/');
        editor.type_char('c');

        let offset = cursor_visual_offset(&editor.root, &editor.cursor).unwrap();

        assert!(offset > EMPTY_VISUAL_OFFSET);
    }
}
