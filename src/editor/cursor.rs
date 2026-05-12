//! Cursor model: tracks a position within the `MathNode` tree as a path of steps.

use super::tree::MathNode;

/// A single step in a cursor path, identifying which child slot we entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStep {
    /// Position within a `Seq`: the cursor sits *before* the child at this index.
    /// Index `0` means before the first child; index `len` means after the last.
    SeqPos(usize),
    /// Inside a `Fraction`'s numerator.
    Numerator,
    /// Inside a `Fraction`'s denominator.
    Denominator,
    /// Inside a `Sup`/`Sub`/`SupSub` base.
    Base,
    /// Inside a `Sup`/`SupSub` exponent.
    Exponent,
    /// Inside a `Sub`/`SupSub` subscript.
    Subscript,
    /// Inside a `Sqrt` radicand.
    Radicand,
    /// Inside a `Sqrt` index (nth-root).
    Index,
    /// Inside a `Parens` body or `Style` body.
    Inner,
}

/// Direction for cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// The cursor's position in the tree: a stack of steps from the root.
///
/// The path always ends with a `SeqPos` step — the cursor is always
/// positioned within some `Seq` (the gap between children).
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub(crate) path: Vec<CursorStep>,
}

/// Resolved cursor location: a reference to the `Seq` and position within it.
pub struct ResolvedCursor<'a> {
    pub seq: &'a [MathNode],
    pub pos: usize,
}

/// Mutable resolved cursor location.
pub struct ResolvedCursorMut<'a> {
    pub seq: &'a mut Vec<MathNode>,
    pub pos: usize,
}

impl Cursor {
    /// Create a cursor at position 0 in the root `Seq`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            path: vec![CursorStep::SeqPos(0)],
        }
    }

    /// Create a cursor at a specific position in the root `Seq`.
    #[must_use]
    pub fn at_root_pos(pos: usize) -> Self {
        Self {
            path: vec![CursorStep::SeqPos(pos)],
        }
    }

    /// The full path from root to the current position.
    #[must_use]
    pub fn path(&self) -> &[CursorStep] {
        &self.path
    }

    /// The current `SeqPos` index (the last step in the path).
    #[must_use]
    pub fn seq_pos(&self) -> usize {
        match self.path.last() {
            Some(CursorStep::SeqPos(p)) => *p,
            _ => 0,
        }
    }

    /// Depth of the cursor (number of nested structures we're inside).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.path.len().saturating_sub(1)
    }

    /// Resolve the cursor against a tree, returning the `Seq` and position.
    #[must_use]
    pub fn resolve<'a>(&self, root: &'a MathNode) -> Option<ResolvedCursor<'a>> {
        let mut node = root;
        for (i, step) in self.path.iter().enumerate() {
            if i == self.path.len() - 1 {
                if let CursorStep::SeqPos(pos) = step {
                    if let Some(seq) = node.as_seq() {
                        return Some(ResolvedCursor { seq, pos: *pos });
                    }
                }
                return None;
            }
            node = descend(node, *step)?;
        }
        None
    }

    /// Resolve the cursor mutably.
    pub fn resolve_mut<'a>(&self, root: &'a mut MathNode) -> Option<ResolvedCursorMut<'a>> {
        let mut node = root;
        for (i, step) in self.path.iter().enumerate() {
            if i == self.path.len() - 1 {
                if let CursorStep::SeqPos(pos) = step {
                    if let Some(seq) = node.as_seq_mut() {
                        return Some(ResolvedCursorMut { seq, pos: *pos });
                    }
                }
                return None;
            }
            node = descend_mut(node, *step)?;
        }
        None
    }

    /// Move left within the tree.
    pub fn move_left(&mut self, root: &MathNode) {
        self.move_dir(root, Direction::Left);
    }

    /// Move right within the tree.
    pub fn move_right(&mut self, root: &MathNode) {
        self.move_dir(root, Direction::Right);
    }

    /// Move in the given direction.
    pub fn move_dir(&mut self, root: &MathNode, dir: Direction) {
        let pos = self.seq_pos();
        let seq_len = self.resolve(root).map_or(0, |r| r.seq.len());

        match dir {
            Direction::Left => {
                if pos > 0 {
                    let new_pos = pos - 1;
                    if let Some(seq) = self.resolve(root).map(|r| r.seq) {
                        if let Some(child) = seq.get(new_pos) {
                            if try_enter_right(child, &mut self.path, new_pos) {
                                return;
                            }
                        }
                    }
                    self.set_seq_pos(new_pos);
                } else {
                    self.exit_left(root);
                }
            }
            Direction::Right => {
                if pos < seq_len {
                    if let Some(seq) = self.resolve(root).map(|r| r.seq) {
                        if let Some(child) = seq.get(pos) {
                            if try_enter_left(child, &mut self.path, pos) {
                                return;
                            }
                        }
                    }
                    self.set_seq_pos(pos + 1);
                } else {
                    self.exit_right(root);
                }
            }
        }
    }

    /// Move up: fraction denominator→numerator, subscript→base, etc.
    pub fn move_up(&mut self, root: &MathNode) {
        if self.path.len() < 2 {
            return;
        }
        let parent_idx = self.path.len() - 2;
        let parent_step = self.path[parent_idx];
        match parent_step {
            CursorStep::Denominator => {
                self.path[parent_idx] = CursorStep::Numerator;
                self.set_seq_pos(0);
            }
            CursorStep::Subscript => {
                if let Some(node) = self.resolve_parent_node(root) {
                    if matches!(node, MathNode::SupSub { .. }) {
                        let idx = self.path.len() - 2;
                        self.path[idx] = CursorStep::Exponent;
                        self.set_seq_pos(0);
                        return;
                    }
                }
                let idx = self.path.len() - 2;
                self.path[idx] = CursorStep::Base;
                self.set_seq_pos(0);
            }
            CursorStep::Exponent | CursorStep::Base => {
                self.exit_left(root);
            }
            _ => {}
        }
    }

    /// Move down: fraction numerator→denominator, base→subscript, etc.
    pub fn move_down(&mut self, root: &MathNode) {
        if self.path.len() < 2 {
            return;
        }
        let parent_idx = self.path.len() - 2;
        let parent_step = self.path[parent_idx];
        match parent_step {
            CursorStep::Numerator => {
                self.path[parent_idx] = CursorStep::Denominator;
                self.set_seq_pos(0);
            }
            CursorStep::Exponent => {
                if let Some(node) = self.resolve_parent_node(root) {
                    if matches!(node, MathNode::SupSub { .. }) {
                        let idx = self.path.len() - 2;
                        self.path[idx] = CursorStep::Subscript;
                        self.set_seq_pos(0);
                        return;
                    }
                }
                let idx = self.path.len() - 2;
                self.path[idx] = CursorStep::Base;
                self.set_seq_pos(0);
            }
            CursorStep::Base | CursorStep::Subscript => {
                self.exit_right(root);
            }
            _ => {}
        }
    }

    /// Move cursor to the start of the current `Seq`.
    pub fn move_to_start(&mut self) {
        self.set_seq_pos(0);
    }

    /// Move cursor to the end of the current `Seq`.
    pub fn move_to_end(&mut self, root: &MathNode) {
        let seq_len = self.resolve(root).map_or(0, |r| r.seq.len());
        self.set_seq_pos(seq_len);
    }

    /// Tab: move to the next sibling slot in the parent compound node,
    /// or exit right if at the last slot.
    pub fn tab(&mut self, root: &MathNode) {
        if self.path.len() < 2 {
            return;
        }
        let parent_idx = self.path.len() - 2;
        let parent_step = self.path[parent_idx];
        let next = match parent_step {
            CursorStep::Numerator => Some(CursorStep::Denominator),
            CursorStep::Base => {
                if let Some(node) = self.resolve_parent_node(root) {
                    match node {
                        MathNode::Sub { .. } => Some(CursorStep::Subscript),
                        MathNode::Sup { .. } | MathNode::SupSub { .. } => {
                            Some(CursorStep::Exponent)
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            CursorStep::Exponent => {
                if let Some(node) = self.resolve_parent_node(root) {
                    if matches!(node, MathNode::SupSub { .. }) {
                        Some(CursorStep::Subscript)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            CursorStep::Index => Some(CursorStep::Radicand),
            _ => None,
        };

        if let Some(next_step) = next {
            self.path[parent_idx] = next_step;
            self.set_seq_pos(0);
        } else {
            self.exit_right(root);
        }
    }

    /// Shift+Tab: move to the previous sibling slot in the parent compound node,
    /// or exit left if at the first slot.
    pub fn shift_tab(&mut self, root: &MathNode) {
        if self.path.len() < 2 {
            return;
        }
        let parent_idx = self.path.len() - 2;
        let parent_step = self.path[parent_idx];
        let prev = match parent_step {
            CursorStep::Denominator => Some(CursorStep::Numerator),
            CursorStep::Subscript => {
                if let Some(node) = self.resolve_parent_node(root) {
                    if matches!(node, MathNode::SupSub { .. }) {
                        Some(CursorStep::Exponent)
                    } else {
                        Some(CursorStep::Base)
                    }
                } else {
                    None
                }
            }
            CursorStep::Exponent => Some(CursorStep::Base),
            CursorStep::Radicand => {
                if let Some(node) = self.resolve_parent_node(root) {
                    if matches!(node, MathNode::Sqrt { index: Some(_), .. }) {
                        Some(CursorStep::Index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(prev_step) = prev {
            let end_pos = {
                self.path[parent_idx] = prev_step;
                self.set_seq_pos(0);
                self.resolve(root).map_or(0, |r| r.seq.len())
            };
            self.set_seq_pos(end_pos);
        } else {
            self.exit_left(root);
        }
    }

    fn set_seq_pos(&mut self, pos: usize) {
        if let Some(last) = self.path.last_mut() {
            *last = CursorStep::SeqPos(pos);
        }
    }

    /// Exit the current `Seq` to the left (move to parent's left side).
    fn exit_left(&mut self, _root: &MathNode) {
        if self.path.len() <= 1 {
            return;
        }
        self.path.pop(); // remove SeqPos
        let parent_step = self.path.pop(); // remove parent slot step

        // Now we need to find what index the parent node is at in the grandparent Seq.
        // The step before (now the last) should be SeqPos(idx) where idx is
        // the position of the compound node in the grandparent.
        // The cursor should land at that position (before the compound node).
        if let Some(CursorStep::SeqPos(_idx)) = parent_step {
            // already at the right position — the grandparent SeqPos is already set
        }
    }

    /// Exit the current `Seq` to the right.
    fn exit_right(&mut self, _root: &MathNode) {
        if self.path.len() <= 1 {
            return;
        }
        self.path.pop(); // remove SeqPos
        self.path.pop(); // remove parent slot step

        // Move to position after the compound node: increment the grandparent SeqPos by 1.
        if let Some(CursorStep::SeqPos(idx)) = self.path.last_mut() {
            *idx += 1;
        }
    }

    /// Resolve the parent compound node (the node containing the current Seq).
    fn resolve_parent_node<'a>(&self, root: &'a MathNode) -> Option<&'a MathNode> {
        if self.path.len() < 3 {
            return None;
        }
        // Walk to the grandparent Seq, then index into it.
        let mut node = root;
        for step in &self.path[..self.path.len() - 3] {
            node = descend(node, *step)?;
        }
        // The step at len-3 should be SeqPos(idx), giving us the compound node.
        if let CursorStep::SeqPos(idx) = self.path[self.path.len() - 3] {
            if let Some(seq) = node.as_seq() {
                return seq.get(idx);
            }
        }
        None
    }

    /// Create a cursor from an explicit path.
    #[must_use]
    pub fn from_path(path: Vec<CursorStep>) -> Self {
        Self { path }
    }

    /// Mutable access to the path.
    pub fn path_mut(&mut self) -> &mut Vec<CursorStep> {
        &mut self.path
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Descend into a child slot of a node (immutable).
fn descend(node: &MathNode, step: CursorStep) -> Option<&MathNode> {
    match (node, step) {
        (MathNode::Seq(children), CursorStep::SeqPos(idx)) => children.get(idx),
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => Some(num),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => Some(den),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => Some(radicand),
        (
            MathNode::Sqrt {
                index: Some(idx), ..
            },
            CursorStep::Index,
        ) => Some(idx),
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

/// Descend into a child slot of a node (mutable).
fn descend_mut(node: &mut MathNode, step: CursorStep) -> Option<&mut MathNode> {
    match (node, step) {
        (MathNode::Seq(children), CursorStep::SeqPos(idx)) => children.get_mut(idx),
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => Some(num),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => Some(den),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => Some(radicand),
        (
            MathNode::Sqrt {
                index: Some(idx), ..
            },
            CursorStep::Index,
        ) => Some(idx),
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

/// Try to enter a compound node from the left (moving right into it).
/// Returns `true` if we entered the node and modified the path.
fn try_enter_left(node: &MathNode, path: &mut Vec<CursorStep>, _idx: usize) -> bool {
    let slot = match node {
        MathNode::Fraction { .. } => CursorStep::Numerator,
        MathNode::Sqrt { index: Some(_), .. } => CursorStep::Index,
        MathNode::Sqrt { .. } => CursorStep::Radicand,
        MathNode::Sup { .. } | MathNode::SupSub { .. } => CursorStep::Exponent,
        MathNode::Sub { .. } => CursorStep::Subscript,
        MathNode::Parens { .. } | MathNode::Style { .. } => CursorStep::Inner,
        _ => return false,
    };
    path.push(slot);
    path.push(CursorStep::SeqPos(0));
    true
}

/// Try to enter a compound node from the right (moving left into it).
fn try_enter_right(node: &MathNode, path: &mut Vec<CursorStep>, _idx: usize) -> bool {
    let (slot, child) = match node {
        MathNode::Fraction { den, .. } => (CursorStep::Denominator, den.as_ref()),
        MathNode::Sqrt { radicand, .. } => (CursorStep::Radicand, radicand.as_ref()),
        MathNode::Sup { exp, .. } => (CursorStep::Exponent, exp.as_ref()),
        MathNode::Sub { script, .. } => (CursorStep::Subscript, script.as_ref()),
        MathNode::SupSub { sub, .. } => (CursorStep::Subscript, sub.as_ref()),
        MathNode::Parens { body, .. } | MathNode::Style { body, .. } => {
            (CursorStep::Inner, body.as_ref())
        }
        _ => return false,
    };
    let len = child.as_seq().map_or(0, <[MathNode]>::len);
    path.push(slot);
    path.push(CursorStep::SeqPos(len));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tree::SymbolData;

    fn sym(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    #[test]
    fn cursor_starts_at_zero() {
        let cursor = Cursor::new();
        assert_eq!(cursor.seq_pos(), 0);
        assert_eq!(cursor.depth(), 0);
    }

    #[test]
    fn resolve_in_simple_seq() {
        let root = MathNode::Seq(vec![sym("x"), sym("y")]);
        let cursor = Cursor::at_root_pos(1);
        let resolved = cursor.resolve(&root).unwrap();
        assert_eq!(resolved.pos, 1);
        assert_eq!(resolved.seq.len(), 2);
    }

    #[test]
    fn move_right_through_seq() {
        let root = MathNode::Seq(vec![sym("a"), sym("b"), sym("c")]);
        let mut cursor = Cursor::new();

        cursor.move_right(&root);
        assert_eq!(cursor.seq_pos(), 1);

        cursor.move_right(&root);
        assert_eq!(cursor.seq_pos(), 2);

        cursor.move_right(&root);
        assert_eq!(cursor.seq_pos(), 3);

        // At end, move_right should be a no-op (root level).
        cursor.move_right(&root);
        assert_eq!(cursor.seq_pos(), 3);
    }

    #[test]
    fn move_left_through_seq() {
        let root = MathNode::Seq(vec![sym("a"), sym("b")]);
        let mut cursor = Cursor::at_root_pos(2);

        cursor.move_left(&root);
        assert_eq!(cursor.seq_pos(), 1);

        cursor.move_left(&root);
        assert_eq!(cursor.seq_pos(), 0);

        // At start, move_left should be a no-op (root level).
        cursor.move_left(&root);
        assert_eq!(cursor.seq_pos(), 0);
    }

    #[test]
    fn move_right_enters_fraction_numerator() {
        let root = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mut cursor = Cursor::new();

        // Move right should enter the fraction's numerator.
        cursor.move_right(&root);
        assert_eq!(
            cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::Numerator,
                CursorStep::SeqPos(0),
            ]
        );
    }

    #[test]
    fn tab_moves_numerator_to_denominator() {
        let root = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mut cursor = Cursor {
            path: vec![
                CursorStep::SeqPos(0),
                CursorStep::Numerator,
                CursorStep::SeqPos(0),
            ],
        };
        cursor.tab(&root);
        assert_eq!(
            cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::Denominator,
                CursorStep::SeqPos(0),
            ]
        );
    }

    #[test]
    fn tab_exits_denominator() {
        let root = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mut cursor = Cursor {
            path: vec![
                CursorStep::SeqPos(0),
                CursorStep::Denominator,
                CursorStep::SeqPos(1),
            ],
        };
        cursor.tab(&root);
        assert_eq!(cursor.path(), &[CursorStep::SeqPos(1)]);
    }

    #[test]
    fn shift_tab_denominator_to_numerator() {
        let root = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mut cursor = Cursor {
            path: vec![
                CursorStep::SeqPos(0),
                CursorStep::Denominator,
                CursorStep::SeqPos(0),
            ],
        };
        cursor.shift_tab(&root);
        assert_eq!(
            cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::Numerator,
                CursorStep::SeqPos(1),
            ]
        );
    }

    #[test]
    fn home_end() {
        let root = MathNode::Seq(vec![sym("a"), sym("b"), sym("c")]);
        let mut cursor = Cursor::at_root_pos(2);
        cursor.move_to_start();
        assert_eq!(cursor.seq_pos(), 0);
        cursor.move_to_end(&root);
        assert_eq!(cursor.seq_pos(), 3);
    }

    #[test]
    fn move_up_down_in_fraction() {
        let root = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mut cursor = Cursor {
            path: vec![
                CursorStep::SeqPos(0),
                CursorStep::Numerator,
                CursorStep::SeqPos(0),
            ],
        };

        // Move down from numerator → denominator.
        cursor.move_down(&root);
        assert_eq!(
            cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::Denominator,
                CursorStep::SeqPos(0),
            ]
        );

        // Move up from denominator → numerator.
        cursor.move_up(&root);
        assert_eq!(
            cursor.path(),
            &[
                CursorStep::SeqPos(0),
                CursorStep::Numerator,
                CursorStep::SeqPos(0),
            ]
        );
    }
}
