//! Serialize a `MathNode` tree to a LaTeX string compatible with `RaTeX`'s parser.

use std::ops::Range;

use crate::editor::cursor::CursorStep;
use crate::editor::tree::{BracketKind, MathNode, MatrixKind, StyleKind};

const EMPTY_GROUP_LATEX: &str = "{}";
const GROUP_OPEN: char = '{';
const GROUP_CLOSE: char = '}';
const FRACTION_OPEN: &str = "\\frac{";
const FRACTION_MIDDLE: &str = "}{";
const SQRT_OPEN: &str = "\\sqrt{";
const SQRT_INDEX_OPEN: &str = "\\sqrt[";
const SQRT_INDEX_MIDDLE: &str = "]{";
const SUP_OPEN: &str = "^{";
const SUB_OPEN: &str = "_{";
const SUPSUB_MIDDLE: &str = "}^{";
const LEFT_COMMAND: &str = "\\left";
const RIGHT_COMMAND: &str = "\\right";
const TEXT_OPEN: &str = "\\text{";
const BEGIN_OPEN: &str = "\\begin{";
const END_OPEN: &str = "\\end{";
const ENV_CLOSE_WITH_SPACE: &str = "} ";
const ENV_CLOSE: char = '}';
const MATRIX_COLUMN_SEPARATOR: &str = " & ";
const MATRIX_ROW_SEPARATOR: &str = " \\\\ ";

/// LaTeX output plus source mappings back to the editable math tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedLatex {
    latex: String,
    node_spans: Vec<RenderedNodeSpan>,
    cursor_positions: Vec<RenderedCursorPosition>,
}

impl MappedLatex {
    /// The serialized LaTeX string.
    #[must_use]
    pub fn latex(&self) -> &str {
        &self.latex
    }

    /// Consume this mapping and return the serialized LaTeX string.
    #[must_use]
    pub fn into_latex(self) -> String {
        self.latex
    }

    /// Spans for AST nodes in the serialized LaTeX string.
    #[must_use]
    pub fn node_spans(&self) -> &[RenderedNodeSpan] {
        &self.node_spans
    }

    /// Serialized byte positions for cursor insertion points.
    #[must_use]
    pub fn cursor_positions(&self) -> &[RenderedCursorPosition] {
        &self.cursor_positions
    }
}

/// A span in the serialized LaTeX string associated with a `MathNode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedNodeSpan {
    path: Vec<CursorStep>,
    span: Range<usize>,
}

impl RenderedNodeSpan {
    /// Cursor-style path to the node represented by this span.
    #[must_use]
    pub fn path(&self) -> &[CursorStep] {
        &self.path
    }

    /// Byte span in the serialized LaTeX string.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
}

/// A serialized LaTeX byte position for a cursor insertion point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedCursorPosition {
    path: Vec<CursorStep>,
    byte_index: usize,
}

impl RenderedCursorPosition {
    /// Cursor path represented by this serialized position.
    #[must_use]
    pub fn path(&self) -> &[CursorStep] {
        &self.path
    }

    /// Byte index in the serialized LaTeX string.
    #[must_use]
    pub fn byte_index(&self) -> usize {
        self.byte_index
    }
}

/// Serialize a `MathNode` to a LaTeX string.
#[must_use]
pub fn to_latex(node: &MathNode) -> String {
    to_latex_with_mapping(node).into_latex()
}

/// Serialize a `MathNode` to LaTeX and include source mappings.
#[must_use]
pub fn to_latex_with_mapping(node: &MathNode) -> MappedLatex {
    let mut writer = LatexMappingWriter::default();
    writer.write_node(node, &[]);
    writer.finish()
}

#[derive(Default)]
struct LatexMappingWriter {
    latex: String,
    node_spans: Vec<RenderedNodeSpan>,
    cursor_positions: Vec<RenderedCursorPosition>,
}

impl LatexMappingWriter {
    fn finish(self) -> MappedLatex {
        MappedLatex {
            latex: self.latex,
            node_spans: self.node_spans,
            cursor_positions: self.cursor_positions,
        }
    }

    fn write_node(&mut self, node: &MathNode, path: &[CursorStep]) {
        let start = self.latex.len();

        match node {
            MathNode::Seq(children) => {
                self.record_cursor_position(path, 0);
                for (index, child) in children.iter().enumerate() {
                    let child_path = path_with_step(path, CursorStep::SeqPos(index));
                    self.write_node(child, &child_path);
                    self.record_cursor_position(path, index + 1);
                }
            }
            MathNode::Symbol(data) => {
                self.latex.push_str(&data.ctrl_seq);
            }
            MathNode::Fraction { num, den } => {
                self.latex.push_str(FRACTION_OPEN);
                self.write_node(num, &path_with_step(path, CursorStep::Numerator));
                self.latex.push_str(FRACTION_MIDDLE);
                self.write_node(den, &path_with_step(path, CursorStep::Denominator));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Sqrt {
                index: None,
                radicand,
            } => {
                self.latex.push_str(SQRT_OPEN);
                self.write_node(radicand, &path_with_step(path, CursorStep::Radicand));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Sqrt {
                index: Some(idx),
                radicand,
            } => {
                self.latex.push_str(SQRT_INDEX_OPEN);
                self.write_node(idx, &path_with_step(path, CursorStep::Index));
                self.latex.push_str(SQRT_INDEX_MIDDLE);
                self.write_node(radicand, &path_with_step(path, CursorStep::Radicand));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Sup { base, exp } => {
                self.write_braced_if_compound(base, &path_with_step(path, CursorStep::Base));
                self.latex.push_str(SUP_OPEN);
                self.write_node(exp, &path_with_step(path, CursorStep::Exponent));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Sub { base, script } => {
                self.write_braced_if_compound(base, &path_with_step(path, CursorStep::Base));
                self.latex.push_str(SUB_OPEN);
                self.write_node(script, &path_with_step(path, CursorStep::Subscript));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::SupSub { base, sup, sub } => {
                self.write_braced_if_compound(base, &path_with_step(path, CursorStep::Base));
                self.latex.push_str(SUB_OPEN);
                self.write_node(sub, &path_with_step(path, CursorStep::Subscript));
                self.latex.push_str(SUPSUB_MIDDLE);
                self.write_node(sup, &path_with_step(path, CursorStep::Exponent));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Parens { open, close, body } => {
                self.latex.push_str(LEFT_COMMAND);
                self.latex.push_str(bracket_latex(*open, true));
                self.write_node(body, &path_with_step(path, CursorStep::Inner));
                self.latex.push_str(RIGHT_COMMAND);
                self.latex.push_str(bracket_latex(*close, false));
            }
            MathNode::Style { kind, body } => {
                self.latex.push_str(style_command(*kind));
                self.latex.push(GROUP_OPEN);
                self.write_node(body, &path_with_step(path, CursorStep::Inner));
                self.latex.push(GROUP_CLOSE);
            }
            MathNode::Matrix { kind, cells } => {
                self.write_matrix(*kind, cells, path);
            }
            MathNode::Text(text) => {
                self.latex.push_str(TEXT_OPEN);
                self.latex.push_str(text);
                self.latex.push(GROUP_CLOSE);
            }
        }

        self.record_node_span(path, start);
    }

    fn write_braced_if_compound(&mut self, node: &MathNode, path: &[CursorStep]) {
        match node {
            MathNode::Seq(children) if children.is_empty() => {
                let start = self.latex.len();
                self.latex.push_str(EMPTY_GROUP_LATEX);
                let cursor_index = start + GROUP_OPEN.len_utf8();
                self.record_cursor_position_at(path, 0, cursor_index);
                self.record_node_span_at(path, cursor_index, cursor_index);
            }
            MathNode::Seq(children) if children.len() == 1 => {
                self.write_node(node, path);
            }
            MathNode::Seq(_) => {
                self.latex.push(GROUP_OPEN);
                self.write_node(node, path);
                self.latex.push(GROUP_CLOSE);
            }
            _ => {
                self.write_node(node, path);
            }
        }
    }

    fn write_matrix(&mut self, kind: MatrixKind, cells: &[Vec<MathNode>], path: &[CursorStep]) {
        let environment = kind.environment_name();
        self.latex.push_str(BEGIN_OPEN);
        self.latex.push_str(environment);
        self.latex.push_str(ENV_CLOSE_WITH_SPACE);

        for (row_index, row) in cells.iter().enumerate() {
            if row_index > 0 {
                self.latex.push_str(MATRIX_ROW_SEPARATOR);
            }

            for (col_index, cell) in row.iter().enumerate() {
                if col_index > 0 {
                    self.latex.push_str(MATRIX_COLUMN_SEPARATOR);
                }

                self.write_node(
                    cell,
                    &path_with_step(
                        path,
                        CursorStep::MatrixCell {
                            row: row_index,
                            col: col_index,
                        },
                    ),
                );
            }
        }

        self.latex.push_str(END_OPEN);
        self.latex.push_str(environment);
        self.latex.push(ENV_CLOSE);
    }

    fn record_node_span(&mut self, path: &[CursorStep], start: usize) {
        self.record_node_span_at(path, start, self.latex.len());
    }

    fn record_node_span_at(&mut self, path: &[CursorStep], start: usize, end: usize) {
        self.node_spans.push(RenderedNodeSpan {
            path: path.to_vec(),
            span: start..end,
        });
    }

    fn record_cursor_position(&mut self, seq_path: &[CursorStep], pos: usize) {
        self.record_cursor_position_at(seq_path, pos, self.latex.len());
    }

    fn record_cursor_position_at(
        &mut self,
        seq_path: &[CursorStep],
        pos: usize,
        byte_index: usize,
    ) {
        self.cursor_positions.push(RenderedCursorPosition {
            path: path_with_step(seq_path, CursorStep::SeqPos(pos)),
            byte_index,
        });
    }
}

fn path_with_step(path: &[CursorStep], step: CursorStep) -> Vec<CursorStep> {
    let mut extended = Vec::with_capacity(path.len() + 1);
    extended.extend_from_slice(path);
    extended.push(step);
    extended
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
    use super::{to_latex, to_latex_with_mapping, MappedLatex, RenderedNodeSpan};
    use crate::editor::cursor::CursorStep;
    use crate::editor::tree::{BracketKind, MathNode, MatrixKind};
    use crate::editor::tree::{SymbolData, SymbolKind};

    fn sym(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    fn digit(d: char) -> MathNode {
        MathNode::Symbol(SymbolData::digit(d))
    }

    fn span_for<'a>(mapped: &'a MappedLatex, path: &[CursorStep]) -> &'a RenderedNodeSpan {
        mapped
            .node_spans()
            .iter()
            .find(|span| span.path() == path)
            .unwrap()
    }

    fn cursor_index_for(mapped: &MappedLatex, path: &[CursorStep]) -> usize {
        mapped
            .cursor_positions()
            .iter()
            .find(|position| position.path() == path)
            .unwrap()
            .byte_index()
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
    fn matrix_environment() {
        let tree = MathNode::Matrix {
            kind: MatrixKind::Parenthesized,
            cells: vec![
                vec![MathNode::Seq(vec![sym("a")]), MathNode::Seq(vec![sym("b")])],
                vec![MathNode::Seq(vec![sym("c")]), MathNode::Seq(vec![sym("d")])],
            ],
        };

        assert_eq!(
            to_latex(&tree),
            "\\begin{pmatrix} a & b \\\\ c & d\\end{pmatrix}",
        );
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

    #[test]
    fn mapped_latex_matches_plain_serializer() {
        let tree = MathNode::Seq(vec![
            MathNode::Sup {
                base: Box::new(MathNode::Seq(vec![sym("x")])),
                exp: Box::new(MathNode::Seq(vec![digit('2')])),
            },
            MathNode::Symbol(SymbolData::binary_op("+", "+")),
            MathNode::Sqrt {
                index: Some(Box::new(MathNode::Seq(vec![digit('3')]))),
                radicand: Box::new(MathNode::Seq(vec![sym("y")])),
            },
        ]);

        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), to_latex(&tree));
        assert_eq!(mapped.latex(), "x^{2}+\\sqrt[3]{y}");
    }

    #[test]
    fn mapped_latex_records_symbol_spans() {
        let tree = MathNode::Seq(vec![sym("x"), sym("y")]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(span_for(&mapped, &[CursorStep::SeqPos(0)]).span(), 0..1);
        assert_eq!(span_for(&mapped, &[CursorStep::SeqPos(1)]).span(), 1..2);
    }

    #[test]
    fn mapped_latex_records_cursor_positions() {
        let tree = MathNode::Seq(vec![sym("x"), sym("y")]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(cursor_index_for(&mapped, &[CursorStep::SeqPos(0)]), 0);
        assert_eq!(cursor_index_for(&mapped, &[CursorStep::SeqPos(1)]), 1);
        assert_eq!(cursor_index_for(&mapped, &[CursorStep::SeqPos(2)]), 2);
    }

    #[test]
    fn mapped_latex_records_fraction_slots() {
        let tree = MathNode::Seq(vec![MathNode::Fraction {
            num: Box::new(MathNode::Seq(vec![sym("a")])),
            den: Box::new(MathNode::Seq(vec![sym("b")])),
        }]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), "\\frac{a}{b}");
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Numerator,
                    CursorStep::SeqPos(0),
                ],
            )
            .span(),
            6..7,
        );
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Denominator,
                    CursorStep::SeqPos(0),
                ],
            )
            .span(),
            9..10,
        );
    }

    #[test]
    fn mapped_latex_records_script_slots() {
        let tree = MathNode::Seq(vec![MathNode::Sup {
            base: Box::new(MathNode::Seq(vec![sym("x")])),
            exp: Box::new(MathNode::Seq(vec![digit('2')])),
        }]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), "x^{2}");
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Base,
                    CursorStep::SeqPos(0),
                ],
            )
            .span(),
            0..1,
        );
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Exponent,
                    CursorStep::SeqPos(0),
                ],
            )
            .span(),
            3..4,
        );
    }

    #[test]
    fn mapped_latex_records_root_slots() {
        let tree = MathNode::Seq(vec![MathNode::Sqrt {
            index: Some(Box::new(MathNode::Seq(vec![digit('3')]))),
            radicand: Box::new(MathNode::Seq(vec![sym("x")])),
        }]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), "\\sqrt[3]{x}");
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Index,
                    CursorStep::SeqPos(0)
                ],
            )
            .span(),
            6..7,
        );
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Radicand,
                    CursorStep::SeqPos(0),
                ],
            )
            .span(),
            9..10,
        );
    }

    #[test]
    fn mapped_latex_records_paren_inner_slot() {
        let tree = MathNode::Seq(vec![MathNode::Parens {
            open: BracketKind::Round,
            close: BracketKind::Round,
            body: Box::new(MathNode::Seq(vec![sym("x")])),
        }]);
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), "\\left(x\\right)");
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::SeqPos(0),
                    CursorStep::Inner,
                    CursorStep::SeqPos(0)
                ],
            )
            .span(),
            6..7,
        );
    }

    #[test]
    fn mapped_latex_records_matrix_cell_slots() {
        let tree = MathNode::Matrix {
            kind: MatrixKind::Bracketed,
            cells: vec![vec![MathNode::Seq(vec![sym("x")])]],
        };
        let mapped = to_latex_with_mapping(&tree);

        assert_eq!(mapped.latex(), "\\begin{bmatrix} x\\end{bmatrix}");
        assert_eq!(
            span_for(
                &mapped,
                &[
                    CursorStep::MatrixCell { row: 0, col: 0 },
                    CursorStep::SeqPos(0)
                ],
            )
            .span(),
            16..17,
        );
        assert_eq!(
            cursor_index_for(
                &mapped,
                &[
                    CursorStep::MatrixCell { row: 0, col: 0 },
                    CursorStep::SeqPos(1)
                ],
            ),
            17,
        );
    }
}
