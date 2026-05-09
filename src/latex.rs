//! Serialize a `MathNode` tree to a LaTeX string compatible with `RaTeX`'s parser.

use crate::editor::tree::{BracketKind, MathNode, StyleKind};

/// Serialize a `MathNode` to a LaTeX string.
#[must_use]
pub fn to_latex(node: &MathNode) -> String {
    let mut buf = String::new();
    write_node(node, &mut buf);
    buf
}

fn write_node(node: &MathNode, buf: &mut String) {
    match node {
        MathNode::Seq(children) => {
            for child in children {
                write_node(child, buf);
            }
        }
        MathNode::Symbol(data) => {
            buf.push_str(&data.ctrl_seq);
        }
        MathNode::Fraction { num, den } => {
            buf.push_str("\\frac{");
            write_node(num, buf);
            buf.push_str("}{");
            write_node(den, buf);
            buf.push('}');
        }
        MathNode::Sqrt {
            index: None,
            radicand,
        } => {
            buf.push_str("\\sqrt{");
            write_node(radicand, buf);
            buf.push('}');
        }
        MathNode::Sqrt {
            index: Some(idx),
            radicand,
        } => {
            buf.push_str("\\sqrt[");
            write_node(idx, buf);
            buf.push_str("]{");
            write_node(radicand, buf);
            buf.push('}');
        }
        MathNode::Sup { base, exp } => {
            write_braced_if_compound(base, buf);
            buf.push_str("^{");
            write_node(exp, buf);
            buf.push('}');
        }
        MathNode::Sub { base, script } => {
            write_braced_if_compound(base, buf);
            buf.push_str("_{");
            write_node(script, buf);
            buf.push('}');
        }
        MathNode::SupSub { base, sup, sub } => {
            write_braced_if_compound(base, buf);
            buf.push_str("_{");
            write_node(sub, buf);
            buf.push_str("}^{");
            write_node(sup, buf);
            buf.push('}');
        }
        MathNode::Parens { open, close, body } => {
            buf.push_str("\\left");
            buf.push_str(bracket_latex(*open, true));
            write_node(body, buf);
            buf.push_str("\\right");
            buf.push_str(bracket_latex(*close, false));
        }
        MathNode::Style { kind, body } => {
            buf.push_str(style_command(*kind));
            buf.push('{');
            write_node(body, buf);
            buf.push('}');
        }
        MathNode::Text(text) => {
            buf.push_str("\\text{");
            buf.push_str(text);
            buf.push('}');
        }
    }
}

/// Wrap in braces if the base is a multi-child Seq (more than one node).
fn write_braced_if_compound(node: &MathNode, buf: &mut String) {
    match node {
        MathNode::Seq(children) if children.len() == 1 => {
            write_node(&children[0], buf);
        }
        MathNode::Seq(children) if children.is_empty() => {
            buf.push_str("{}");
        }
        MathNode::Seq(_) => {
            buf.push('{');
            write_node(node, buf);
            buf.push('}');
        }
        _ => {
            write_node(node, buf);
        }
    }
}

/// Map a bracket kind to its LaTeX delimiter string.
fn bracket_latex(kind: BracketKind, is_open: bool) -> &'static str {
    match (kind, is_open) {
        (BracketKind::Round, true) => "(",
        (BracketKind::Round, false) => ")",
        (BracketKind::Square, true) => "[",
        (BracketKind::Square, false) => "]",
        (BracketKind::Curly, true) => "\\{",
        (BracketKind::Curly, false) => "\\}",
        (BracketKind::Angle, true) => "\\langle ",
        (BracketKind::Angle, false) => "\\rangle ",
        (BracketKind::Pipe, _) => "|",
        (BracketKind::DoublePipe, true) => "\\lVert ",
        (BracketKind::DoublePipe, false) => "\\rVert ",
    }
}

/// Map a style kind to its LaTeX command.
fn style_command(kind: StyleKind) -> &'static str {
    match kind {
        StyleKind::Roman => "\\mathrm",
        StyleKind::Italic => "\\mathit",
        StyleKind::Bold => "\\mathbf",
        StyleKind::SansSerif => "\\mathsf",
        StyleKind::Monospace => "\\mathtt",
        StyleKind::Underline => "\\underline",
        StyleKind::Overline => "\\overline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::tree::{SymbolData, SymbolKind};

    fn sym(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    fn digit(d: char) -> MathNode {
        MathNode::Symbol(SymbolData::digit(d))
    }

    #[test]
    fn simple_seq() {
        let tree = MathNode::Seq(vec![sym("x"), sym("y"), sym("z")]);
        assert_eq!(to_latex(&tree), "xyz");
    }

    #[test]
    fn fraction() {
        let tree = MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        };
        assert_eq!(to_latex(&tree), "\\frac{a}{b}");
    }

    #[test]
    fn nested_fraction() {
        let tree = MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![digit('1')])),
            den: Box::new(MathNode::Seq(vec![MathNode::Fraction {
                num: Box::new(MathNode::Seq(vec![digit('2')])),
                den: Box::new(MathNode::Seq(vec![digit('3')])),
            }])),
        };
        assert_eq!(to_latex(&tree), "\\frac{1}{\\frac{2}{3}}");
    }

    #[test]
    fn superscript() {
        let tree = MathNode::Sup {
            base: Box::new(MathNode::Seq(vec![sym("x")])),
            exp: Box::new(MathNode::Seq(vec![digit('2')])),
        };
        assert_eq!(to_latex(&tree), "x^{2}");
    }

    #[test]
    fn subscript() {
        let tree = MathNode::Sub {
            base: Box::new(MathNode::Seq(vec![sym("x")])),
            script: Box::new(MathNode::Seq(vec![digit('0')])),
        };
        assert_eq!(to_latex(&tree), "x_{0}");
    }

    #[test]
    fn supsub() {
        let tree = MathNode::SupSub {
            base: Box::new(MathNode::Seq(vec![sym("x")])),
            sup: Box::new(MathNode::Seq(vec![digit('2')])),
            sub: Box::new(MathNode::Seq(vec![sym("i")])),
        };
        assert_eq!(to_latex(&tree), "x_{i}^{2}");
    }

    #[test]
    fn sqrt_simple() {
        let tree = MathNode::Sqrt {
            index: None,
            radicand: Box::new(MathNode::Seq(vec![sym("x")])),
        };
        assert_eq!(to_latex(&tree), "\\sqrt{x}");
    }

    #[test]
    fn sqrt_with_index() {
        let tree = MathNode::Sqrt {
            index: Some(Box::new(MathNode::Seq(vec![digit('3')]))),
            radicand: Box::new(MathNode::Seq(vec![sym("x")])),
        };
        assert_eq!(to_latex(&tree), "\\sqrt[3]{x}");
    }

    #[test]
    fn parens() {
        let tree = MathNode::Parens {
            open: BracketKind::Round,
            close: BracketKind::Round,
            body: Box::new(MathNode::Seq(vec![
                sym("a"),
                MathNode::Symbol(SymbolData::binary_op("+", "+")),
                sym("b"),
            ])),
        };
        assert_eq!(to_latex(&tree), "\\left(a+b\\right)");
    }

    #[test]
    fn empty_fraction() {
        let tree = MathNode::Fraction {
            num: Box::new(MathNode::empty_seq()),
            den: Box::new(MathNode::empty_seq()),
        };
        assert_eq!(to_latex(&tree), "\\frac{}{}");
    }

    #[test]
    fn complex_expression() {
        // x^2 + \frac{1}{\sqrt{y}}
        let tree = MathNode::Seq(vec![
            MathNode::Sup {
                base: Box::new(MathNode::Seq(vec![sym("x")])),
                exp: Box::new(MathNode::Seq(vec![digit('2')])),
            },
            MathNode::Symbol(SymbolData::binary_op("+", "+")),
            MathNode::Fraction {
                num: Box::new(MathNode::Seq(vec![digit('1')])),
                den: Box::new(MathNode::Seq(vec![MathNode::Sqrt {
                    index: None,
                    radicand: Box::new(MathNode::Seq(vec![sym("y")])),
                }])),
            },
        ]);
        assert_eq!(to_latex(&tree), "x^{2}+\\frac{1}{\\sqrt{y}}");
    }

    #[test]
    fn greek_letter() {
        let tree = MathNode::Symbol(SymbolData {
            ch: "α".to_string(),
            ctrl_seq: "\\alpha ".to_string(),
            kind: SymbolKind::Variable,
        });
        assert_eq!(to_latex(&tree), "\\alpha ");
    }
}
