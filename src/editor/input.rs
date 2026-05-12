//! Input handling: maps keystrokes to `MathNode` tree mutations.

use super::auto_cmds;
use super::commands;
use super::cursor::{Cursor, CursorStep};
use super::selection::Selection;
use super::tree::{MathNode, MatrixKind, SymbolData, SymbolKind};
use super::undo::UndoStack;

const PARSEABLE_SPACE_CTRL_SEQ: &str = "\\,";

/// The top-level editor: owns the math tree, cursor, and undo history.
pub struct Editor {
    pub root: MathNode,
    pub cursor: Cursor,
    pub selection: Option<Selection>,
    undo_stack: UndoStack,
    dirty: bool,
}

impl Editor {
    /// Create a new editor with an empty root sequence.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: MathNode::empty_seq(),
            cursor: Cursor::new(),
            selection: None,
            undo_stack: UndoStack::new(),
            dirty: true,
        }
    }

    /// Whether the tree has changed since the last render.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the editor as clean (call after rendering).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Record a snapshot for undo before making a change.
    fn snapshot(&mut self) {
        self.undo_stack.push(self.root.clone(), self.cursor.clone());
    }

    /// Undo the last edit.
    pub fn undo(&mut self) {
        if let Some((root, cursor)) = self.undo_stack.undo(self.root.clone(), self.cursor.clone()) {
            self.root = root;
            self.cursor = cursor;
            self.dirty = true;
        }
    }

    /// Redo a previously undone edit.
    pub fn redo(&mut self) {
        if let Some((root, cursor)) = self.undo_stack.redo() {
            self.root = root;
            self.cursor = cursor;
            self.dirty = true;
        }
    }

    /// Delete the current selection (if any) and collapse the cursor.
    /// Returns `true` if a selection was deleted.
    pub fn delete_selection(&mut self) -> bool {
        let Some(sel) = self.selection.take() else {
            return false;
        };
        if !sel.is_nonempty(self.cursor.seq_pos()) {
            return false;
        }
        self.snapshot();
        let cursor_pos = self.cursor.seq_pos();
        let (start, _end) = sel.range(cursor_pos);
        sel.take_nodes(&mut self.root, cursor_pos);
        if let Some(last) = self.cursor.path_mut().last_mut() {
            *last = CursorStep::SeqPos(start);
        }
        self.dirty = true;
        true
    }

    /// Start or extend a selection in the given direction.
    pub fn select_left(&mut self) {
        if self.selection.is_none() {
            self.selection = Some(Selection::from_cursor(&self.cursor));
        }
        let pos = self.cursor.seq_pos();
        if pos > 0 {
            if let Some(last) = self.cursor.path_mut().last_mut() {
                *last = CursorStep::SeqPos(pos - 1);
            }
        }
    }

    /// Start or extend a selection to the right.
    pub fn select_right(&mut self) {
        if self.selection.is_none() {
            self.selection = Some(Selection::from_cursor(&self.cursor));
        }
        let seq_len = self.cursor.resolve(&self.root).map_or(0, |r| r.seq.len());
        let pos = self.cursor.seq_pos();
        if pos < seq_len {
            if let Some(last) = self.cursor.path_mut().last_mut() {
                *last = CursorStep::SeqPos(pos + 1);
            }
        }
    }

    /// Select all nodes in the current `Seq`.
    pub fn select_all(&mut self) {
        let seq_len = self.cursor.resolve(&self.root).map_or(0, |r| r.seq.len());
        let path = self.cursor.path();
        let seq_path = path[..path.len() - 1].to_vec();
        self.selection = Some(Selection {
            seq_path,
            anchor: 0,
        });
        if let Some(last) = self.cursor.path_mut().last_mut() {
            *last = CursorStep::SeqPos(seq_len);
        }
    }

    /// Copy the current selection as a LaTeX string.
    #[must_use]
    pub fn copy_selection_latex(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        if !sel.is_nonempty(self.cursor.seq_pos()) {
            return None;
        }
        let (start, end) = sel.range(self.cursor.seq_pos());
        let resolved = self.cursor.resolve(&self.root)?;
        let end = end.min(resolved.seq.len());
        let selected_nodes = &resolved.seq[start..end];
        let temp_seq = MathNode::Seq(selected_nodes.to_vec());
        Some(crate::latex::to_latex(&temp_seq))
    }

    /// Handle a character typed by the user.
    pub fn type_char(&mut self, ch: char) {
        if self.selection.is_some() && matches!(ch, '/' | '^' | '_' | '(' | '[' | '{' | '|') {
            self.wrap_selection(ch);
            return;
        }
        self.delete_selection();
        self.snapshot();
        match ch {
            '/' => self.insert_fraction(),
            '^' => self.insert_superscript(),
            '_' => self.insert_subscript(),
            '(' => self.insert_parens(super::tree::BracketKind::Round),
            '[' => self.insert_parens(super::tree::BracketKind::Square),
            '{' => self.insert_parens(super::tree::BracketKind::Curly),
            '|' => self.insert_parens(super::tree::BracketKind::Pipe),
            ')' | ']' | '}' => self.close_bracket(),
            c if c.is_ascii_digit() => self.insert_symbol(SymbolData::digit(c)),
            c if c.is_ascii_alphabetic() => {
                self.insert_symbol(SymbolData::variable(&c.to_string()));
            }
            '+' => self.insert_symbol(SymbolData::binary_op("+", "+")),
            '-' => self.insert_symbol(SymbolData::binary_op("\u{2212}", "-")),
            '*' => self.insert_symbol(SymbolData::binary_op("\u{22C5}", "\\cdot ")),
            '=' => self.insert_symbol(SymbolData::relation("=", "=")),
            '<' => self.insert_symbol(SymbolData::relation("<", "<")),
            '>' => self.insert_symbol(SymbolData::relation(">", ">")),
            ',' => self.insert_symbol(SymbolData {
                ch: ",".to_string(),
                ctrl_seq: ",".to_string(),
                kind: SymbolKind::Punctuation,
            }),
            '.' => self.insert_symbol(SymbolData {
                ch: ".".to_string(),
                ctrl_seq: ".".to_string(),
                kind: SymbolKind::Digit,
            }),
            '!' => self.insert_symbol(SymbolData {
                ch: "!".to_string(),
                ctrl_seq: "!".to_string(),
                kind: SymbolKind::Punctuation,
            }),
            '%' => self.insert_symbol(SymbolData {
                ch: "%".to_string(),
                ctrl_seq: "\\%".to_string(),
                kind: SymbolKind::Punctuation,
            }),
            ':' => self.insert_symbol(SymbolData {
                ch: ":".to_string(),
                ctrl_seq: ":".to_string(),
                kind: SymbolKind::Relation,
            }),
            ';' => self.insert_symbol(SymbolData {
                ch: ";".to_string(),
                ctrl_seq: ";".to_string(),
                kind: SymbolKind::Punctuation,
            }),
            '~' => self.insert_symbol(SymbolData {
                ch: "~".to_string(),
                ctrl_seq: "\\sim ".to_string(),
                kind: SymbolKind::Relation,
            }),
            ' ' => self.insert_symbol(SymbolData {
                ch: " ".to_string(),
                ctrl_seq: PARSEABLE_SPACE_CTRL_SEQ.to_string(),
                kind: SymbolKind::Space,
            }),
            _ => {
                self.undo_stack.pop_last();
            }
        }
    }

    /// Wrap the current selection in a structure determined by the character.
    fn wrap_selection(&mut self, ch: char) {
        let Some(sel) = self.selection.take() else {
            return;
        };
        if !sel.is_nonempty(self.cursor.seq_pos()) {
            return;
        }
        self.snapshot();
        let cursor_pos = self.cursor.seq_pos();
        let (start, _end) = sel.range(cursor_pos);
        let taken = sel.take_nodes(&mut self.root, cursor_pos);
        let content = MathNode::Seq(taken);

        let (wrapper, cursor_slot) = match ch {
            '/' => (
                MathNode::Fraction {
                    num: Box::new(content),
                    den: Box::new(MathNode::empty_seq()),
                },
                CursorStep::Denominator,
            ),
            '^' => (
                MathNode::Sup {
                    base: Box::new(content),
                    exp: Box::new(MathNode::empty_seq()),
                },
                CursorStep::Exponent,
            ),
            '_' => (
                MathNode::Sub {
                    base: Box::new(content),
                    script: Box::new(MathNode::empty_seq()),
                },
                CursorStep::Subscript,
            ),
            '(' | '[' | '{' | '|' => {
                let kind = match ch {
                    '(' => super::tree::BracketKind::Round,
                    '[' => super::tree::BracketKind::Square,
                    '{' => super::tree::BracketKind::Curly,
                    _ => super::tree::BracketKind::Pipe,
                };
                (
                    MathNode::Parens {
                        open: kind,
                        close: kind,
                        body: Box::new(content),
                    },
                    CursorStep::Inner,
                )
            }
            _ => return,
        };

        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            resolved.seq.insert(start, wrapper);
            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(start));
            new_path.push(cursor_slot);
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Insert a symbol at the cursor position.
    fn insert_symbol(&mut self, data: SymbolData) {
        let is_variable = data.kind == SymbolKind::Variable;
        let node = MathNode::Symbol(data);
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            resolved.seq.insert(resolved.pos, node);
            self.cursor.move_right(&self.root);
            self.dirty = true;
        }
        if is_variable {
            self.try_auto_replace();
        }
    }

    /// Check if the last few symbols form an auto-operator, auto-symbol, or
    /// auto-structure, and replace them if so.
    fn try_auto_replace(&mut self) {
        let pos = self.cursor.seq_pos();
        let replacement = {
            let Some(resolved) = self.cursor.resolve(&self.root) else {
                return;
            };
            auto_cmds::check_all(resolved.seq, pos)
        };

        if let Some((start, replacement_node)) = replacement {
            let enter_slot = cursor_entry_slot(&replacement_node);
            if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
                let end = pos.min(resolved.seq.len());
                resolved.seq.drain(start..end);
                resolved.seq.insert(start, replacement_node);

                if let Some(slot) = enter_slot {
                    let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
                    let mut new_path: Vec<CursorStep> = base_path.to_vec();
                    new_path.push(CursorStep::SeqPos(start));
                    new_path.push(slot);
                    new_path.push(CursorStep::SeqPos(0));
                    self.cursor = Cursor::from_path(new_path);
                } else if let Some(last) = self.cursor.path_mut().last_mut() {
                    *last = CursorStep::SeqPos(start + 1);
                }
            }
        }
    }

    /// Insert a fraction: wrap leftward content into the numerator (`LiveFraction` behavior).
    fn insert_fraction(&mut self) {
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let scan_start = commands::scan_left_for_fraction(resolved.seq, pos);
            let num_nodes: Vec<MathNode> = resolved.seq.drain(scan_start..pos).collect();
            let insert_pos = scan_start;

            let frac = MathNode::Fraction {
                num: Box::new(MathNode::Seq(num_nodes)),
                den: Box::new(MathNode::empty_seq()),
            };
            resolved.seq.insert(insert_pos, frac);

            // Place cursor in the denominator.
            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(insert_pos));
            new_path.push(CursorStep::Denominator);
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Insert a superscript: wrap the node to the left into `Sup`.
    fn insert_superscript(&mut self) {
        commands::insert_script(&mut self.root, &mut self.cursor, true);
        self.dirty = true;
    }

    /// Insert a subscript: wrap the node to the left into `Sub`.
    fn insert_subscript(&mut self) {
        commands::insert_script(&mut self.root, &mut self.cursor, false);
        self.dirty = true;
    }

    /// Insert parentheses around an empty body.
    fn insert_parens(&mut self, kind: super::tree::BracketKind) {
        let close = kind;
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let parens = MathNode::Parens {
                open: kind,
                close,
                body: Box::new(MathNode::empty_seq()),
            };
            resolved.seq.insert(pos, parens);

            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(pos));
            new_path.push(CursorStep::Inner);
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Handle closing bracket: exit the nearest matching Parens.
    fn close_bracket(&mut self) {
        // Walk up the cursor path looking for a Parens to close.
        self.cursor.move_right(&self.root);
        // Try to exit the innermost Parens.
        let path = self.cursor.path().to_vec();
        for i in (0..path.len()).rev() {
            if path[i] == CursorStep::Inner {
                // Exit this Parens: truncate path up to and past the Inner step.
                let mut new_path = path[..i - 1].to_vec();
                if let CursorStep::SeqPos(idx) = path[i - 1] {
                    new_path.push(CursorStep::SeqPos(idx + 1));
                }
                self.cursor = Cursor::from_path(new_path);
                self.dirty = true;
                return;
            }
        }
    }

    /// Handle backspace.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            if pos > 0 {
                let removed = resolved.seq.remove(pos - 1);
                let mut new_pos = pos - 1;

                // If we removed a compound node, unwrap its first child's content.
                match removed {
                    MathNode::Fraction { num, .. } => {
                        if let MathNode::Seq(children) = *num {
                            let count = children.len();
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(new_pos + i, child);
                            }
                            new_pos += count;
                        }
                    }
                    MathNode::Sup { base, .. }
                    | MathNode::Sub { base, .. }
                    | MathNode::SupSub { base, .. } => {
                        if let MathNode::Seq(children) = *base {
                            let count = children.len();
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(new_pos + i, child);
                            }
                            new_pos += count;
                        }
                    }
                    MathNode::Parens { body, .. } | MathNode::Style { body, .. } => {
                        if let MathNode::Seq(children) = *body {
                            let count = children.len();
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(new_pos + i, child);
                            }
                            new_pos += count;
                        }
                    }
                    MathNode::Sqrt { radicand, .. } => {
                        if let MathNode::Seq(children) = *radicand {
                            let count = children.len();
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(new_pos + i, child);
                            }
                            new_pos += count;
                        }
                    }
                    _ => {}
                }

                // Update cursor position.
                if let Some(last) = self.cursor.path_mut().last_mut() {
                    *last = CursorStep::SeqPos(new_pos);
                }
                self.dirty = true;
            } else if self.cursor.depth() > 0 {
                self.cursor.move_left(&self.root);
                self.dirty = true;
            } else {
                self.undo_stack.pop_last();
            }
        }
    }

    /// Handle delete (forward).
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            if pos < resolved.seq.len() {
                let removed = resolved.seq.remove(pos);
                let insert_pos = pos;

                match removed {
                    MathNode::Fraction { num, .. } => {
                        if let MathNode::Seq(children) = *num {
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(insert_pos + i, child);
                            }
                        }
                    }
                    MathNode::Sup { base, .. }
                    | MathNode::Sub { base, .. }
                    | MathNode::SupSub { base, .. } => {
                        if let MathNode::Seq(children) = *base {
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(insert_pos + i, child);
                            }
                        }
                    }
                    MathNode::Parens { body, .. } | MathNode::Style { body, .. } => {
                        if let MathNode::Seq(children) = *body {
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(insert_pos + i, child);
                            }
                        }
                    }
                    MathNode::Sqrt { radicand, .. } => {
                        if let MathNode::Seq(children) = *radicand {
                            for (i, child) in children.into_iter().enumerate() {
                                resolved.seq.insert(insert_pos + i, child);
                            }
                        }
                    }
                    _ => {}
                }

                self.dirty = true;
            } else {
                self.undo_stack.pop_last();
            }
        }
    }

    /// Move cursor left (clears selection).
    pub fn move_left(&mut self) {
        self.selection = None;
        self.cursor.move_left(&self.root);
    }

    /// Move cursor right (clears selection).
    pub fn move_right(&mut self) {
        self.selection = None;
        self.cursor.move_right(&self.root);
    }

    /// Move cursor up (clears selection).
    pub fn move_up(&mut self) {
        self.selection = None;
        self.cursor.move_up(&self.root);
    }

    /// Move cursor down (clears selection).
    pub fn move_down(&mut self) {
        self.selection = None;
        self.cursor.move_down(&self.root);
    }

    /// Move cursor to start of current `Seq` (Home key).
    pub fn move_home(&mut self) {
        self.selection = None;
        self.cursor.move_to_start();
    }

    /// Move cursor to end of current `Seq` (End key).
    pub fn move_end(&mut self) {
        self.selection = None;
        self.cursor.move_to_end(&self.root);
    }

    /// Tab: advance to next field in compound node, or exit right.
    pub fn tab(&mut self) {
        self.selection = None;
        self.cursor.tab(&self.root);
    }

    /// Shift+Tab: go to previous field in compound node, or exit left.
    pub fn shift_tab(&mut self) {
        self.selection = None;
        self.cursor.shift_tab(&self.root);
    }

    /// Insert a `\sqrt{}` at the cursor position.
    pub fn insert_sqrt(&mut self) {
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let sqrt = MathNode::Sqrt {
                index: None,
                radicand: Box::new(MathNode::empty_seq()),
            };
            resolved.seq.insert(pos, sqrt);

            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(pos));
            new_path.push(CursorStep::Radicand);
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Insert a `\sqrt[n]{}` (nth root) at the cursor position, with cursor in the index.
    pub fn insert_nth_root(&mut self) {
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let sqrt = MathNode::Sqrt {
                index: Some(Box::new(MathNode::empty_seq())),
                radicand: Box::new(MathNode::empty_seq()),
            };
            resolved.seq.insert(pos, sqrt);

            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(pos));
            new_path.push(CursorStep::Index);
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Insert a matrix-like environment at the cursor position.
    pub fn insert_matrix(&mut self, kind: MatrixKind, rows: usize, cols: usize) {
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let matrix = MathNode::matrix(kind, rows, cols);
            resolved.seq.insert(pos, matrix);

            let base_path = &self.cursor.path()[..self.cursor.path().len() - 1];
            let mut new_path: Vec<CursorStep> = base_path.to_vec();
            new_path.push(CursorStep::SeqPos(pos));
            new_path.push(CursorStep::MatrixCell { row: 0, col: 0 });
            new_path.push(CursorStep::SeqPos(0));
            self.cursor = Cursor::from_path(new_path);
            self.dirty = true;
        }
    }

    /// Insert a `\text{}` block at the cursor position.
    pub fn insert_text_block(&mut self) {
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            let text = MathNode::Text(String::new());
            resolved.seq.insert(pos, text);
            self.cursor.move_right(&self.root);
            self.dirty = true;
        }
    }

    /// Paste a LaTeX string by replaying its characters through `type_char`.
    pub fn paste_latex(&mut self, latex: &str) {
        self.delete_selection();
        for ch in latex.chars() {
            match ch {
                '\\' | '{' | '}' => {}
                _ => self.type_char(ch),
            }
        }
    }

    /// Get the current LaTeX string.
    #[must_use]
    pub fn to_latex(&self) -> String {
        crate::latex::to_latex(&self.root)
    }
}

/// Determine which child slot the cursor should enter after an auto-structure
/// replacement. Returns `None` for non-structure nodes (cursor stays after).
fn cursor_entry_slot(node: &MathNode) -> Option<CursorStep> {
    match node {
        MathNode::Sqrt { .. } => Some(CursorStep::Radicand),
        MathNode::Parens { .. } => Some(CursorStep::Inner),
        MathNode::Sub { .. } => Some(CursorStep::Subscript),
        MathNode::Matrix { .. } => Some(CursorStep::MatrixCell { row: 0, col: 0 }),
        _ => None,
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Editor;
    use crate::editor::cursor::CursorStep;
    use crate::editor::tree::MatrixKind;

    #[test]
    fn type_letters() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('y');
        assert_eq!(editor.to_latex(), "xy");
    }

    #[test]
    fn type_fraction() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('+');
        editor.type_char('b');
        editor.type_char('/');
        // Should create \frac{b}{} with cursor in denominator.
        editor.type_char('c');
        let latex = editor.to_latex();
        assert!(latex.contains("\\frac"), "latex was: {latex}");
    }

    #[test]
    fn type_superscript() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('^');
        editor.type_char('2');
        let latex = editor.to_latex();
        assert!(latex.contains("^{"), "latex was: {latex}");
    }

    #[test]
    fn backspace_removes_symbol() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.backspace();
        assert_eq!(editor.to_latex(), "a");
    }

    #[test]
    fn undo_redo() {
        let mut editor = Editor::new();
        editor.type_char('x');
        assert_eq!(editor.to_latex(), "x");
        editor.undo();
        assert_eq!(editor.to_latex(), "");
        editor.redo();
        assert_eq!(editor.to_latex(), "x");
    }

    #[test]
    fn auto_operator_sin() {
        let mut editor = Editor::new();
        editor.type_char('s');
        editor.type_char('i');
        editor.type_char('n');
        assert_eq!(editor.to_latex(), "\\sin ");
    }

    #[test]
    fn auto_operator_cos_after_content() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('+');
        editor.type_char('c');
        editor.type_char('o');
        editor.type_char('s');
        let latex = editor.to_latex();
        assert!(latex.contains("\\cos "), "latex was: {latex}");
    }

    #[test]
    fn auto_symbol_pi() {
        let mut editor = Editor::new();
        editor.type_char('p');
        editor.type_char('i');
        assert_eq!(editor.to_latex(), "\\pi ");
    }

    #[test]
    fn auto_symbol_alpha() {
        let mut editor = Editor::new();
        for ch in "alpha".chars() {
            editor.type_char(ch);
        }
        assert_eq!(editor.to_latex(), "\\alpha ");
    }

    #[test]
    fn space_after_auto_symbol_uses_parseable_spacing_command() {
        let mut editor = Editor::new();
        for ch in "alpha ".chars() {
            editor.type_char(ch);
        }
        assert_eq!(editor.to_latex(), "\\alpha \\,");
    }

    #[test]
    fn select_and_delete() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        // Cursor at pos 3. Select left twice to select "bc".
        editor.select_left();
        editor.select_left();
        assert!(editor.selection.is_some());
        editor.backspace();
        assert_eq!(editor.to_latex(), "a");
    }

    #[test]
    fn select_all_and_delete() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('y');
        editor.select_all();
        editor.backspace();
        assert_eq!(editor.to_latex(), "");
    }

    #[test]
    fn copy_selection_latex() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        editor.select_left();
        editor.select_left();
        let copied = editor.copy_selection_latex();
        assert_eq!(copied, Some("bc".to_string()));
    }

    #[test]
    fn auto_structure_sqrt() {
        let mut editor = Editor::new();
        for ch in "sqrt".chars() {
            editor.type_char(ch);
        }
        let latex = editor.to_latex();
        assert!(latex.contains("\\sqrt{"), "latex was: {latex}");
    }

    #[test]
    fn auto_structure_sum() {
        let mut editor = Editor::new();
        for ch in "sum".chars() {
            editor.type_char(ch);
        }
        let latex = editor.to_latex();
        assert!(latex.contains("\\sum "), "latex was: {latex}");
    }

    #[test]
    fn paste_latex_simple() {
        let mut editor = Editor::new();
        editor.paste_latex("abc");
        assert_eq!(editor.to_latex(), "abc");
    }

    #[test]
    fn paste_latex_with_braces() {
        let mut editor = Editor::new();
        editor.paste_latex("x+y");
        assert_eq!(editor.to_latex(), "x+y");
    }

    #[test]
    fn backspace_unwraps_supsub() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('^');
        editor.type_char('2');
        // Exit exponent, then add subscript.
        editor.move_right();
        editor.type_char('_');
        editor.type_char('i');
        // Exit subscript.
        editor.move_right();
        let latex = editor.to_latex();
        assert!(latex.contains("_{"), "latex was: {latex}");
        // Backspace should unwrap SupSub, leaving just the base "x".
        editor.backspace();
        let latex = editor.to_latex();
        assert_eq!(latex, "x");
    }

    #[test]
    fn delete_forward_unwraps_fraction() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        editor.type_char('b');
        // Move cursor back to before the fraction.
        editor.move_left();
        editor.move_left();
        // Delete forward should unwrap the fraction, keeping its numerator.
        editor.delete_forward();
        let latex = editor.to_latex();
        assert!(!latex.contains("\\frac"), "latex was: {latex}");
        assert!(latex.contains('a'), "latex was: {latex}");
    }

    #[test]
    fn tab_moves_between_fraction_fields() {
        let mut editor = Editor::new();
        editor.type_char('/');
        // Cursor is in denominator. Type something.
        editor.type_char('b');
        // Tab should exit the fraction.
        editor.tab();
        editor.type_char('c');
        let latex = editor.to_latex();
        assert!(latex.contains("\\frac"), "latex was: {latex}");
        assert!(latex.ends_with('c'), "latex was: {latex}");
    }

    #[test]
    fn home_end_keys() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        assert_eq!(editor.cursor.seq_pos(), 3);
        editor.move_home();
        assert_eq!(editor.cursor.seq_pos(), 0);
        editor.move_end();
        assert_eq!(editor.cursor.seq_pos(), 3);
    }

    #[test]
    fn pipe_creates_abs_parens() {
        let mut editor = Editor::new();
        editor.type_char('|');
        editor.type_char('x');
        let latex = editor.to_latex();
        assert!(latex.contains("\\left|"), "latex was: {latex}");
    }

    #[test]
    fn curly_braces() {
        let mut editor = Editor::new();
        editor.type_char('{');
        editor.type_char('x');
        let latex = editor.to_latex();
        assert!(latex.contains("\\left\\{"), "latex was: {latex}");
    }

    #[test]
    fn backspace_at_pos0_exits_compound() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        // Cursor is in denominator at pos 0. Backspace should exit left.
        editor.backspace();
        assert!(editor.cursor.depth() < 2, "should have exited the fraction");
    }

    #[test]
    fn shift_tab_reverse() {
        let mut editor = Editor::new();
        editor.type_char('/');
        // Cursor is in denominator. Shift+Tab should go back to numerator.
        editor.shift_tab();
        let path = editor.cursor.path();
        assert!(
            path.contains(&CursorStep::Numerator),
            "should be in numerator, path: {path:?}"
        );
    }

    #[test]
    fn nth_root_insert() {
        let mut editor = Editor::new();
        editor.insert_nth_root();
        editor.type_char('3');
        editor.tab();
        editor.type_char('x');
        let latex = editor.to_latex();
        assert_eq!(latex, "\\sqrt[3]{x}");
    }

    #[test]
    fn matrix_insert_places_cursor_in_first_cell() {
        let mut editor = Editor::new();
        editor.insert_matrix(MatrixKind::Parenthesized, 2, 2);

        assert_eq!(
            editor.cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::MatrixCell { row: 0, col: 0 },
                CursorStep::SeqPos(0),
            ],
        );
        assert_eq!(
            editor.to_latex(),
            "\\begin{pmatrix}  &  \\\\  & \\end{pmatrix}"
        );
    }

    #[test]
    fn factorial_input() {
        let mut editor = Editor::new();
        editor.type_char('n');
        editor.type_char('!');
        assert_eq!(editor.to_latex(), "n!");
    }

    #[test]
    fn tilde_becomes_sim() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('~');
        editor.type_char('y');
        assert_eq!(editor.to_latex(), "x\\sim y");
    }

    #[test]
    fn percent_input() {
        let mut editor = Editor::new();
        editor.type_char('5');
        editor.type_char('%');
        assert_eq!(editor.to_latex(), "5\\%");
    }

    #[test]
    fn backspace_unwraps_sqrt() {
        let mut editor = Editor::new();
        editor.insert_sqrt();
        editor.type_char('x');
        // Exit sqrt.
        editor.move_right();
        // Now backspace the sqrt node.
        editor.backspace();
        assert_eq!(editor.to_latex(), "x");
    }

    #[test]
    fn delete_forward_unwraps_sqrt() {
        let mut editor = Editor::new();
        editor.insert_sqrt();
        editor.type_char('x');
        editor.move_right();
        // Cursor is after sqrt. Go back to before it.
        editor.move_home();
        editor.delete_forward();
        assert_eq!(editor.to_latex(), "x");
    }

    #[test]
    fn auto_symbol_to_arrow() {
        let mut editor = Editor::new();
        for ch in "to".chars() {
            editor.type_char(ch);
        }
        assert_eq!(editor.to_latex(), "\\to ");
    }

    #[test]
    fn auto_symbol_implies() {
        let mut editor = Editor::new();
        for ch in "implies".chars() {
            editor.type_char(ch);
        }
        assert_eq!(editor.to_latex(), "\\implies ");
    }

    #[test]
    fn auto_symbol_subset() {
        let mut editor = Editor::new();
        for ch in "subset".chars() {
            editor.type_char(ch);
        }
        assert_eq!(editor.to_latex(), "\\subset ");
    }

    #[test]
    fn typing_clears_selection() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.select_left();
        assert!(editor.selection.is_some());
        editor.type_char('x');
        assert!(editor.selection.is_none());
        assert_eq!(editor.to_latex(), "ax");
    }

    #[test]
    fn wrap_selection_in_fraction() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        // Select "ab".
        editor.select_all();
        // Pressing / should wrap selection as numerator.
        editor.type_char('/');
        let latex = editor.to_latex();
        assert!(latex.contains("\\frac{ab}"), "latex was: {latex}");
    }

    #[test]
    fn wrap_selection_in_parens() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('+');
        editor.type_char('y');
        editor.select_all();
        editor.type_char('(');
        let latex = editor.to_latex();
        assert!(latex.contains("\\left("), "latex was: {latex}");
        assert!(latex.contains("\\right)"), "latex was: {latex}");
    }

    #[test]
    fn wrap_selection_in_superscript() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.select_all();
        editor.type_char('^');
        // "x" should be the base. Cursor in exponent.
        editor.type_char('2');
        let latex = editor.to_latex();
        assert!(latex.contains("^{2}"), "latex was: {latex}");
    }

    #[test]
    fn auto_structure_norm() {
        let mut editor = Editor::new();
        for ch in "norm".chars() {
            editor.type_char(ch);
        }
        let latex = editor.to_latex();
        assert!(latex.contains("\\lVert"), "latex was: {latex}");
    }
}
