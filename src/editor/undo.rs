//! Undo/redo stack for the editor.

use super::cursor::Cursor;
use super::tree::MathNode;

const MAX_UNDO_DEPTH: usize = 200;

/// A snapshot of editor state for undo/redo.
#[derive(Clone)]
struct Snapshot {
    root: MathNode,
    cursor: Cursor,
}

/// Undo/redo stack storing `(MathNode, Cursor)` snapshots.
pub struct UndoStack {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl UndoStack {
    /// Create an empty undo stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Push a snapshot onto the undo stack (clears redo history).
    pub fn push(&mut self, root: MathNode, cursor: Cursor) {
        self.redo.clear();
        self.undo.push(Snapshot { root, cursor });
        if self.undo.len() > MAX_UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    /// Remove the last pushed snapshot (used when an operation turns out to be a no-op).
    pub fn pop_last(&mut self) {
        self.undo.pop();
    }

    /// Undo: pop from undo stack, push current state to redo, return the restored state.
    pub fn undo(
        &mut self,
        current_root: MathNode,
        current_cursor: Cursor,
    ) -> Option<(MathNode, Cursor)> {
        let snapshot = self.undo.pop()?;
        self.redo.push(Snapshot {
            root: current_root,
            cursor: current_cursor,
        });
        Some((snapshot.root, snapshot.cursor))
    }

    /// Redo: pop from redo stack and return the state.
    pub fn redo(&mut self) -> Option<(MathNode, Cursor)> {
        let snapshot = self.redo.pop()?;
        Some((snapshot.root, snapshot.cursor))
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tree::SymbolData;

    fn sym(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut stack = UndoStack::new();

        let state0 = MathNode::empty_seq();
        let cursor0 = Cursor::new();

        stack.push(state0.clone(), cursor0.clone());

        let state1 = MathNode::Seq(vec![sym("x")]);
        let cursor1 = Cursor::at_root_pos(1);

        let (restored_root, _restored_cursor) =
            stack.undo(state1.clone(), cursor1.clone()).unwrap();
        assert_eq!(restored_root, state0);

        let (redo_root, _redo_cursor) = stack.redo().unwrap();
        assert_eq!(redo_root, state1);
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = UndoStack::new();
        stack.push(MathNode::empty_seq(), Cursor::new());
        stack.undo(MathNode::Seq(vec![sym("x")]), Cursor::at_root_pos(1));
        assert!(stack.redo().is_some());
        // After another push, redo should be cleared.
        stack.push(MathNode::Seq(vec![sym("y")]), Cursor::at_root_pos(1));
        assert!(stack.redo().is_none());
    }
}
