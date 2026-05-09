//! Structured command creation: fraction, superscript, subscript, sqrt, etc.
//!
//! These functions implement the tree mutations that correspond to `MathQuill`'s
//! `createLeftOf` methods on `LiveFraction`, `SuperscriptCommand`, `SubscriptCommand`, etc.

use super::cursor::{Cursor, CursorStep};
use super::tree::{MathNode, SymbolKind};

/// Scan leftward from `pos` to find where the fraction numerator should start.
///
/// Mirrors `MathQuill`'s `LiveFraction::createLeftOf`: walks left past symbols
/// and compound nodes, stopping at a binary operator, separator, or the start
/// of the sequence.
#[must_use]
pub fn scan_left_for_fraction(seq: &[MathNode], pos: usize) -> usize {
    let mut i = pos;
    while i > 0 {
        let node = &seq[i - 1];
        if is_fraction_boundary(node) {
            break;
        }
        i -= 1;
    }
    i
}

/// Returns `true` if this node should stop the leftward scan for fraction wrapping.
fn is_fraction_boundary(node: &MathNode) -> bool {
    match node {
        MathNode::Symbol(data) => matches!(
            data.kind,
            SymbolKind::BinaryOperator
                | SymbolKind::Relation
                | SymbolKind::Punctuation
                | SymbolKind::Space
        ),
        _ => false,
    }
}

/// Insert a superscript (`is_sup = true`) or subscript (`is_sup = false`).
///
/// Behavior:
/// - If there is a node to the left, wrap it as the base.
/// - If the left node is already a `Sup` and we're adding a `Sub` (or vice versa),
///   upgrade to `SupSub`.
/// - Otherwise, create with an empty base.
pub fn insert_script(root: &mut MathNode, cursor: &mut Cursor, is_sup: bool) {
    let Some(resolved) = cursor.resolve_mut(root) else {
        return;
    };

    let pos = resolved.pos;
    let seq = resolved.seq;

    if pos > 0 {
        let left = seq.remove(pos - 1);
        let insert_pos = pos - 1;

        let new_node = match left {
            // Upgrade Sup to SupSub.
            MathNode::Sup { base, exp } if !is_sup => MathNode::SupSub {
                base,
                sup: exp,
                sub: Box::new(MathNode::empty_seq()),
            },
            // Upgrade Sub to SupSub.
            MathNode::Sub { base, script } if is_sup => MathNode::SupSub {
                base,
                sup: Box::new(MathNode::empty_seq()),
                sub: script,
            },
            other => {
                let base = Box::new(MathNode::Seq(vec![other]));
                if is_sup {
                    MathNode::Sup {
                        base,
                        exp: Box::new(MathNode::empty_seq()),
                    }
                } else {
                    MathNode::Sub {
                        base,
                        script: Box::new(MathNode::empty_seq()),
                    }
                }
            }
        };

        seq.insert(insert_pos, new_node);

        let script_step = if is_sup {
            CursorStep::Exponent
        } else {
            CursorStep::Subscript
        };

        let base_path = &cursor.path()[..cursor.path().len() - 1];
        let mut new_path: Vec<CursorStep> = base_path.to_vec();
        new_path.push(CursorStep::SeqPos(insert_pos));
        new_path.push(script_step);
        new_path.push(CursorStep::SeqPos(0));
        *cursor = Cursor::from_path(new_path);
    } else {
        // No node to the left: create with empty base.
        let new_node = if is_sup {
            MathNode::Sup {
                base: Box::new(MathNode::empty_seq()),
                exp: Box::new(MathNode::empty_seq()),
            }
        } else {
            MathNode::Sub {
                base: Box::new(MathNode::empty_seq()),
                script: Box::new(MathNode::empty_seq()),
            }
        };

        seq.insert(0, new_node);

        let script_step = if is_sup {
            CursorStep::Exponent
        } else {
            CursorStep::Subscript
        };

        let base_path = &cursor.path()[..cursor.path().len() - 1];
        let mut new_path: Vec<CursorStep> = base_path.to_vec();
        new_path.push(CursorStep::SeqPos(0));
        new_path.push(script_step);
        new_path.push(CursorStep::SeqPos(0));
        *cursor = Cursor::from_path(new_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tree::SymbolData;

    fn sym(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    fn op(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::binary_op(ch, ch))
    }

    #[test]
    fn scan_left_stops_at_operator() {
        let seq = vec![sym("a"), op("+"), sym("b"), sym("c")];
        assert_eq!(scan_left_for_fraction(&seq, 4), 2);
    }

    #[test]
    fn scan_left_wraps_all_when_no_operator() {
        let seq = vec![sym("a"), sym("b"), sym("c")];
        assert_eq!(scan_left_for_fraction(&seq, 3), 0);
    }

    #[test]
    fn scan_left_from_middle() {
        let seq = vec![sym("a"), op("+"), sym("b")];
        assert_eq!(scan_left_for_fraction(&seq, 3), 2);
    }

    #[test]
    fn insert_sup_wraps_left_node() {
        let mut root = MathNode::Seq(vec![sym("x")]);
        let mut cursor = Cursor::at_root_pos(1);
        insert_script(&mut root, &mut cursor, true);

        match &root.as_seq().unwrap()[0] {
            MathNode::Sup { base, exp } => {
                assert!(base.as_seq().unwrap()[0] == sym("x"));
                assert!(exp.is_empty_seq());
            }
            other => panic!("expected Sup, got {other:?}"),
        }
    }

    #[test]
    fn insert_sub_after_sup_makes_supsub() {
        let mut root = MathNode::Seq(vec![MathNode::Sup {
            base: Box::new(MathNode::Seq(vec![sym("x")])),
            exp: Box::new(MathNode::Seq(vec![sym("2")])),
        }]);
        let mut cursor = Cursor::at_root_pos(1);
        insert_script(&mut root, &mut cursor, false);

        match &root.as_seq().unwrap()[0] {
            MathNode::SupSub { base, sup, sub } => {
                assert!(base.as_seq().unwrap()[0] == sym("x"));
                assert!(sup.as_seq().unwrap()[0] == sym("2"));
                assert!(sub.is_empty_seq());
            }
            other => panic!("expected SupSub, got {other:?}"),
        }
    }
}
