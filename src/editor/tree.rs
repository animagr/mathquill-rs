//! The editable math AST: a recursive tree of `MathNode` variants.

/// The kind of bracket delimiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketKind {
    Round,
    Square,
    Curly,
    Angle,
    Pipe,
    DoublePipe,
}

/// Font/style commands that wrap a child block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Roman,
    Italic,
    Bold,
    SansSerif,
    Monospace,
    Underline,
    Overline,
}

/// Metadata for a single symbol (letter, digit, operator, Greek, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolData {
    /// The display character(s): `x`, `+`, `α`, `\sin`, etc.
    pub ch: String,
    /// The LaTeX control sequence: `x`, `+`, `\\alpha `, `\\sin `, etc.
    pub ctrl_seq: String,
    /// Semantic kind for spacing and input behavior.
    pub kind: SymbolKind,
}

/// Semantic classification of a symbol for spacing and cursor behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Digit,
    BinaryOperator,
    Relation,
    Punctuation,
    Operator,
    Open,
    Close,
    Space,
}

/// A node in the editable math tree.
///
/// `Seq` is the fundamental container — it holds an ordered list of
/// children (like `MathQuill`'s `MathBlock`). Every compound node
/// (`Fraction`, `Sqrt`, etc.) stores its child slots as boxed `MathNode`
/// values which are always `Seq` at the top level.
#[derive(Debug, Clone, PartialEq)]
pub enum MathNode {
    /// An ordered sequence of sibling nodes (the "block" concept from `MathQuill`).
    Seq(Vec<MathNode>),
    /// A single symbol: letter, digit, operator, Greek, etc.
    Symbol(SymbolData),
    /// `\frac{num}{den}`
    Fraction {
        num: Box<MathNode>,
        den: Box<MathNode>,
    },
    /// `\sqrt{radicand}` or `\sqrt[index]{radicand}`
    Sqrt {
        index: Option<Box<MathNode>>,
        radicand: Box<MathNode>,
    },
    /// `{base}^{exp}` — superscript only.
    Sup {
        base: Box<MathNode>,
        exp: Box<MathNode>,
    },
    /// `{base}_{sub}` — subscript only.
    Sub {
        base: Box<MathNode>,
        script: Box<MathNode>,
    },
    /// `{base}_{sub}^{sup}` — both superscript and subscript.
    SupSub {
        base: Box<MathNode>,
        sup: Box<MathNode>,
        sub: Box<MathNode>,
    },
    /// `\left( body \right)` etc.
    Parens {
        open: BracketKind,
        close: BracketKind,
        body: Box<MathNode>,
    },
    /// `\mathrm{...}`, `\mathbf{...}`, etc.
    Style {
        kind: StyleKind,
        body: Box<MathNode>,
    },
    /// `\text{...}` — plain text block.
    Text(String),
}

impl MathNode {
    /// Create an empty sequence.
    #[must_use]
    pub fn empty_seq() -> Self {
        Self::Seq(Vec::new())
    }

    /// Create a `Seq` containing a single symbol.
    #[must_use]
    pub fn seq_of(nodes: Vec<Self>) -> Self {
        Self::Seq(nodes)
    }

    /// Returns `true` if this is an empty `Seq`.
    #[must_use]
    pub fn is_empty_seq(&self) -> bool {
        matches!(self, Self::Seq(children) if children.is_empty())
    }

    /// Returns the children slice if this is a `Seq`, `None` otherwise.
    #[must_use]
    pub fn as_seq(&self) -> Option<&[Self]> {
        match self {
            Self::Seq(children) => Some(children),
            _ => None,
        }
    }

    /// Returns a mutable reference to children if this is a `Seq`.
    pub fn as_seq_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Seq(children) => Some(children),
            _ => None,
        }
    }

    /// Count of "slots" this node exposes for cursor navigation.
    #[must_use]
    pub fn child_slot_count(&self) -> usize {
        match self {
            Self::Seq(_) | Self::Symbol(_) | Self::Text(_) => 0,
            Self::Sqrt { index: None, .. } | Self::Parens { .. } | Self::Style { .. } => 1,
            Self::Fraction { .. }
            | Self::Sqrt { index: Some(_), .. }
            | Self::Sup { .. }
            | Self::Sub { .. } => 2,
            Self::SupSub { .. } => 3,
        }
    }
}

impl SymbolData {
    /// Shorthand for a variable symbol like `x`, `y`, `z`.
    #[must_use]
    pub fn variable(ch: &str) -> Self {
        Self {
            ch: ch.to_string(),
            ctrl_seq: ch.to_string(),
            kind: SymbolKind::Variable,
        }
    }

    /// Shorthand for a digit symbol like `0`–`9`.
    #[must_use]
    pub fn digit(d: char) -> Self {
        Self {
            ch: d.to_string(),
            ctrl_seq: d.to_string(),
            kind: SymbolKind::Digit,
        }
    }

    /// Shorthand for a binary operator like `+`, `-`.
    #[must_use]
    pub fn binary_op(ch: &str, ctrl_seq: &str) -> Self {
        Self {
            ch: ch.to_string(),
            ctrl_seq: ctrl_seq.to_string(),
            kind: SymbolKind::BinaryOperator,
        }
    }

    /// Shorthand for a relation operator like `=`, `<`, `>`.
    #[must_use]
    pub fn relation(ch: &str, ctrl_seq: &str) -> Self {
        Self {
            ch: ch.to_string(),
            ctrl_seq: ctrl_seq.to_string(),
            kind: SymbolKind::Relation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_seq_is_empty() {
        let node = MathNode::empty_seq();
        assert!(node.is_empty_seq());
        assert_eq!(node.as_seq(), Some([].as_slice()));
    }

    #[test]
    fn fraction_has_two_slots() {
        let frac = MathNode::Fraction {
            num: Box::new(MathNode::empty_seq()),
            den: Box::new(MathNode::empty_seq()),
        };
        assert_eq!(frac.child_slot_count(), 2);
    }

    #[test]
    fn symbol_data_constructors() {
        let v = SymbolData::variable("x");
        assert_eq!(v.ch, "x");
        assert_eq!(v.kind, SymbolKind::Variable);

        let d = SymbolData::digit('3');
        assert_eq!(d.ch, "3");
        assert_eq!(d.kind, SymbolKind::Digit);
    }
}
