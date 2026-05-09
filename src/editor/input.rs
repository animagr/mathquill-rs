//! Input handling: maps keystrokes to `MathNode` tree mutations.

use super::commands;
use super::cursor::{Cursor, CursorStep};
use super::tree::{MathNode, SymbolData, SymbolKind};
use super::undo::UndoStack;

/// The top-level editor: owns the math tree, cursor, and undo history.
pub struct Editor {
    pub root: MathNode,
    pub cursor: Cursor,
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
        self.undo_stack
            .push(self.root.clone(), self.cursor.clone());
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

    /// Handle a character typed by the user.
    pub fn type_char(&mut self, ch: char) {
        self.snapshot();
        match ch {
            '/' => self.insert_fraction(),
            '^' => self.insert_superscript(),
            '_' => self.insert_subscript(),
            '(' => self.insert_parens(super::tree::BracketKind::Round),
            '[' => self.insert_parens(super::tree::BracketKind::Square),
            ')' | ']' => self.close_bracket(),
            c if c.is_ascii_digit() => self.insert_symbol(SymbolData::digit(c)),
            c if c.is_ascii_alphabetic() => self.insert_symbol(SymbolData::variable(&c.to_string())),
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
            ' ' => self.insert_symbol(SymbolData {
                ch: " ".to_string(),
                ctrl_seq: "\\ ".to_string(),
                kind: SymbolKind::Space,
            }),
            _ => {
                self.undo_stack.pop_last();
            }
        }
    }

    /// Insert a symbol at the cursor position.
    fn insert_symbol(&mut self, data: SymbolData) {
        let node = MathNode::Symbol(data);
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            resolved.seq.insert(resolved.pos, node);
            self.cursor.move_right(&self.root);
            self.dirty = true;
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
                    | MathNode::Sub { base, .. } => {
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
                    _ => {}
                }

                // Update cursor position.
                if let Some(last) = self.cursor.path_mut().last_mut() {
                    *last = CursorStep::SeqPos(new_pos);
                }
                self.dirty = true;
            } else {
                // At position 0 inside a compound node: exit and delete the wrapper.
                self.undo_stack.pop_last();
            }
        }
    }

    /// Handle delete (forward).
    pub fn delete_forward(&mut self) {
        self.snapshot();
        if let Some(resolved) = self.cursor.resolve_mut(&mut self.root) {
            let pos = resolved.pos;
            if pos < resolved.seq.len() {
                resolved.seq.remove(pos);
                self.dirty = true;
            } else {
                self.undo_stack.pop_last();
            }
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        self.cursor.move_left(&self.root);
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        self.cursor.move_right(&self.root);
    }

    /// Move cursor up.
    pub fn move_up(&mut self) {
        self.cursor.move_up(&self.root);
    }

    /// Move cursor down.
    pub fn move_down(&mut self) {
        self.cursor.move_down(&self.root);
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

    /// Get the current LaTeX string.
    #[must_use]
    pub fn to_latex(&self) -> String {
        crate::latex::to_latex(&self.root)
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
