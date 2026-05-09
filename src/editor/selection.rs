//! Selection model: a contiguous range of nodes within a `Seq`.

use super::cursor::{Cursor, CursorStep};
use super::tree::MathNode;

/// A selection of contiguous nodes within a single `Seq`.
///
/// The `anchor` is where the selection started (Shift was first pressed),
/// and the cursor's current `SeqPos` is the other end. The selected range
/// is `min(anchor, cursor_pos)..max(anchor, cursor_pos)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// Path to the `Seq` containing the selection (everything except the final `SeqPos`).
    pub seq_path: Vec<CursorStep>,
    /// The anchor position (where Shift was first pressed).
    pub anchor: usize,
}

impl Selection {
    /// Create a new selection starting at the cursor's current position.
    #[must_use]
    pub fn from_cursor(cursor: &Cursor) -> Self {
        let path = cursor.path();
        let anchor = cursor.seq_pos();
        let seq_path = path[..path.len() - 1].to_vec();
        Self { seq_path, anchor }
    }

    /// The ordered range `(start, end)` given the current cursor position.
    #[must_use]
    pub fn range(&self, cursor_pos: usize) -> (usize, usize) {
        if cursor_pos < self.anchor {
            (cursor_pos, self.anchor)
        } else {
            (self.anchor, cursor_pos)
        }
    }

    /// Whether the selection is non-empty.
    #[must_use]
    pub fn is_nonempty(&self, cursor_pos: usize) -> bool {
        cursor_pos != self.anchor
    }

    /// Whether the cursor is still in the same `Seq` as the selection anchor.
    #[must_use]
    pub fn is_same_seq(&self, cursor: &Cursor) -> bool {
        let path = cursor.path();
        if path.is_empty() {
            return false;
        }
        let cursor_seq_path = &path[..path.len() - 1];
        self.seq_path == cursor_seq_path
    }

    /// Extract the selected nodes from the tree (consuming them).
    pub fn take_nodes(&self, root: &mut MathNode, cursor_pos: usize) -> Vec<MathNode> {
        let (start, end) = self.range(cursor_pos);
        let mut node = &mut *root;
        for step in &self.seq_path {
            node = match descend_mut_step(node, *step) {
                Some(n) => n,
                None => return Vec::new(),
            };
        }
        if let Some(seq) = node.as_seq_mut() {
            let end = end.min(seq.len());
            let start = start.min(end);
            seq.drain(start..end).collect()
        } else {
            Vec::new()
        }
    }
}

fn descend_mut_step(node: &mut MathNode, step: CursorStep) -> Option<&mut MathNode> {
    match (node, step) {
        (MathNode::Seq(children), CursorStep::SeqPos(idx)) => children.get_mut(idx),
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => Some(num),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => Some(den),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => Some(radicand),
        (MathNode::Sqrt { index: Some(idx), .. }, CursorStep::Index) => Some(idx),
        (MathNode::Sup { base, .. }
        | MathNode::Sub { base, .. }
        | MathNode::SupSub { base, .. }, CursorStep::Base) => Some(base),
        (MathNode::Sup { exp, .. }
        | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => Some(exp),
        (MathNode::Sub { script, .. }
        | MathNode::SupSub { sub: script, .. }, CursorStep::Subscript) => Some(script),
        (MathNode::Parens { body, .. }
        | MathNode::Style { body, .. }, CursorStep::Inner) => Some(body),
        _ => None,
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
    fn selection_range_forward() {
        let sel = Selection {
            seq_path: vec![],
            anchor: 1,
        };
        assert_eq!(sel.range(3), (1, 3));
    }

    #[test]
    fn selection_range_backward() {
        let sel = Selection {
            seq_path: vec![],
            anchor: 3,
        };
        assert_eq!(sel.range(1), (1, 3));
    }

    #[test]
    fn take_nodes_from_root() {
        let mut root = MathNode::Seq(vec![sym("a"), sym("b"), sym("c"), sym("d")]);
        let sel = Selection {
            seq_path: vec![],
            anchor: 1,
        };
        let taken = sel.take_nodes(&mut root, 3);
        assert_eq!(taken.len(), 2);
        assert_eq!(root.as_seq().unwrap().len(), 2);
    }

    #[test]
    fn is_same_seq_checks_path() {
        let sel = Selection {
            seq_path: vec![CursorStep::SeqPos(0), CursorStep::Numerator],
            anchor: 0,
        };
        let cursor_same = Cursor::from_path(vec![
            CursorStep::SeqPos(0),
            CursorStep::Numerator,
            CursorStep::SeqPos(1),
        ]);
        let cursor_diff = Cursor::from_path(vec![
            CursorStep::SeqPos(0),
            CursorStep::Denominator,
            CursorStep::SeqPos(0),
        ]);
        assert!(sel.is_same_seq(&cursor_same));
        assert!(!sel.is_same_seq(&cursor_diff));
    }
}
