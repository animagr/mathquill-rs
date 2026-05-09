//! Auto-commands and auto-operators: detect typed sequences and replace them.
//!
//! When a user types `s`, `i`, `n` in sequence, the three `Symbol` nodes are
//! replaced with a single operator `\sin`. Similarly, typing `a`, `l`, `p`, `h`, `a`
//! produces `\alpha`.

use super::tree::{BracketKind, MathNode, SymbolData, SymbolKind};

/// An auto-command definition: a typed sequence that becomes a single node.
struct AutoCmd {
    trigger: &'static str,
    build: fn() -> MathNode,
}

/// Try to match the longest trigger in a table against the trailing symbols in `seq[..pos]`.
fn check_table(table: &[AutoCmd], seq: &[MathNode], pos: usize) -> Option<(usize, MathNode)> {
    let mut best: Option<(usize, &AutoCmd)> = None;
    for cmd in table {
        let len = cmd.trigger.len();
        if pos < len {
            continue;
        }
        let start = pos - len;
        if matches_trigger(&seq[start..pos], cmd.trigger)
            && best.as_ref().map_or(true, |(best_len, _)| len > *best_len)
        {
            best = Some((len, cmd));
        }
    }
    best.map(|(_, cmd)| (pos - cmd.trigger.len(), (cmd.build)()))
}

/// Check all tables and return the globally longest match.
/// Suppresses short matches if the current contiguous variable run is a prefix
/// of a longer trigger (to avoid `mp` firing inside a partially-typed `implies`).
#[must_use]
pub fn check_all(seq: &[MathNode], pos: usize) -> Option<(usize, MathNode)> {
    let candidates = [
        check_table(AUTO_OPERATORS, seq, pos),
        check_table(AUTO_SYMBOLS, seq, pos),
        check_table(AUTO_STRUCTURES, seq, pos),
    ];
    let best = candidates
        .into_iter()
        .flatten()
        .min_by_key(|(start, _)| *start)?;

    let run = contiguous_variable_run(seq, pos);
    if could_be_prefix_of_longer_trigger(run) && (pos - best.0) < run.len() {
        return None;
    }

    Some(best)
}

/// Extract the contiguous run of variable letters ending at `pos`.
fn contiguous_variable_run(seq: &[MathNode], pos: usize) -> &[MathNode] {
    let mut start = pos;
    while start > 0 {
        if matches!(&seq[start - 1], MathNode::Symbol(data) if data.kind == SymbolKind::Variable) {
            start -= 1;
        } else {
            break;
        }
    }
    &seq[start..pos]
}

/// Check if the given run of variable nodes could be the prefix of any trigger
/// that is longer than the run itself.
fn could_be_prefix_of_longer_trigger(run: &[MathNode]) -> bool {
    let run_str: String = run.iter().filter_map(|n| {
        if let MathNode::Symbol(data) = n {
            data.ch.chars().next()
        } else {
            None
        }
    }).collect();

    if run_str.is_empty() {
        return false;
    }

    for table in [AUTO_OPERATORS, AUTO_SYMBOLS, AUTO_STRUCTURES] {
        for cmd in table {
            if cmd.trigger.len() > run_str.len() && cmd.trigger.starts_with(&run_str) {
                return true;
            }
        }
    }
    false
}

/// Check whether the last N symbols in `seq[..pos]` form an auto-operator name.
/// Returns `Some((start_index, replacement_node))` if a match is found.
#[must_use]
pub fn check_auto_operator(seq: &[MathNode], pos: usize) -> Option<(usize, MathNode)> {
    check_table(AUTO_OPERATORS, seq, pos)
}

/// Check whether the last N symbols form a structural command (like `sqrt`, `sum`, `int`, `prod`).
/// Returns `Some((start_index, replacement_node))` if a match is found.
#[must_use]
pub fn check_auto_structure(seq: &[MathNode], pos: usize) -> Option<(usize, MathNode)> {
    check_table(AUTO_STRUCTURES, seq, pos)
}

/// Check whether the last N symbols form a Greek letter name.
/// Returns `Some((start_index, replacement_node))` if a match is found.
#[must_use]
pub fn check_auto_symbol(seq: &[MathNode], pos: usize) -> Option<(usize, MathNode)> {
    check_table(AUTO_SYMBOLS, seq, pos)
}

fn matches_trigger(nodes: &[MathNode], trigger: &str) -> bool {
    if nodes.len() != trigger.len() {
        return false;
    }
    for (node, expected_ch) in nodes.iter().zip(trigger.chars()) {
        match node {
            MathNode::Symbol(data) if data.kind == SymbolKind::Variable => {
                if data.ch.len() != 1 || !data.ch.starts_with(expected_ch) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn operator(name: &str) -> MathNode {
    MathNode::Symbol(SymbolData {
        ch: name.to_string(),
        ctrl_seq: format!("\\{name} "),
        kind: SymbolKind::Operator,
    })
}

fn greek(display: &str, cmd: &str) -> MathNode {
    MathNode::Symbol(SymbolData {
        ch: display.to_string(),
        ctrl_seq: format!("\\{cmd} "),
        kind: SymbolKind::Variable,
    })
}

const AUTO_OPERATORS: &[AutoCmd] = &[
    AutoCmd { trigger: "sin", build: || operator("sin") },
    AutoCmd { trigger: "cos", build: || operator("cos") },
    AutoCmd { trigger: "tan", build: || operator("tan") },
    AutoCmd { trigger: "sec", build: || operator("sec") },
    AutoCmd { trigger: "csc", build: || operator("csc") },
    AutoCmd { trigger: "cot", build: || operator("cot") },
    AutoCmd { trigger: "arcsin", build: || operator("arcsin") },
    AutoCmd { trigger: "arccos", build: || operator("arccos") },
    AutoCmd { trigger: "arctan", build: || operator("arctan") },
    AutoCmd { trigger: "sinh", build: || operator("sinh") },
    AutoCmd { trigger: "cosh", build: || operator("cosh") },
    AutoCmd { trigger: "tanh", build: || operator("tanh") },
    AutoCmd { trigger: "log", build: || operator("log") },
    AutoCmd { trigger: "ln", build: || operator("ln") },
    AutoCmd { trigger: "exp", build: || operator("exp") },
    AutoCmd { trigger: "lim", build: || operator("lim") },
    AutoCmd { trigger: "min", build: || operator("min") },
    AutoCmd { trigger: "max", build: || operator("max") },
    AutoCmd { trigger: "inf", build: || operator("inf") },
    AutoCmd { trigger: "sup", build: || operator("sup") },
    AutoCmd { trigger: "det", build: || operator("det") },
    AutoCmd { trigger: "dim", build: || operator("dim") },
    AutoCmd { trigger: "deg", build: || operator("deg") },
    AutoCmd { trigger: "gcd", build: || operator("gcd") },
    AutoCmd { trigger: "ker", build: || operator("ker") },
    AutoCmd { trigger: "mod", build: || operator("mod") },
    AutoCmd { trigger: "arg", build: || operator("arg") },
];

const AUTO_SYMBOLS: &[AutoCmd] = &[
    // Lowercase Greek
    AutoCmd { trigger: "alpha", build: || greek("\u{03B1}", "alpha") },
    AutoCmd { trigger: "beta", build: || greek("\u{03B2}", "beta") },
    AutoCmd { trigger: "gamma", build: || greek("\u{03B3}", "gamma") },
    AutoCmd { trigger: "delta", build: || greek("\u{03B4}", "delta") },
    AutoCmd { trigger: "epsilon", build: || greek("\u{03F5}", "epsilon") },
    AutoCmd { trigger: "zeta", build: || greek("\u{03B6}", "zeta") },
    AutoCmd { trigger: "eta", build: || greek("\u{03B7}", "eta") },
    AutoCmd { trigger: "theta", build: || greek("\u{03B8}", "theta") },
    AutoCmd { trigger: "iota", build: || greek("\u{03B9}", "iota") },
    AutoCmd { trigger: "kappa", build: || greek("\u{03BA}", "kappa") },
    AutoCmd { trigger: "lambda", build: || greek("\u{03BB}", "lambda") },
    AutoCmd { trigger: "mu", build: || greek("\u{03BC}", "mu") },
    AutoCmd { trigger: "nu", build: || greek("\u{03BD}", "nu") },
    AutoCmd { trigger: "xi", build: || greek("\u{03BE}", "xi") },
    AutoCmd { trigger: "pi", build: || greek("\u{03C0}", "pi") },
    AutoCmd { trigger: "rho", build: || greek("\u{03C1}", "rho") },
    AutoCmd { trigger: "sigma", build: || greek("\u{03C3}", "sigma") },
    AutoCmd { trigger: "tau", build: || greek("\u{03C4}", "tau") },
    AutoCmd { trigger: "upsilon", build: || greek("\u{03C5}", "upsilon") },
    AutoCmd { trigger: "phi", build: || greek("\u{03D5}", "phi") },
    AutoCmd { trigger: "chi", build: || greek("\u{03C7}", "chi") },
    AutoCmd { trigger: "psi", build: || greek("\u{03C8}", "psi") },
    AutoCmd { trigger: "omega", build: || greek("\u{03C9}", "omega") },
    // Uppercase Greek
    AutoCmd { trigger: "Gamma", build: || greek("\u{0393}", "Gamma") },
    AutoCmd { trigger: "Delta", build: || greek("\u{0394}", "Delta") },
    AutoCmd { trigger: "Theta", build: || greek("\u{0398}", "Theta") },
    AutoCmd { trigger: "Lambda", build: || greek("\u{039B}", "Lambda") },
    AutoCmd { trigger: "Xi", build: || greek("\u{039E}", "Xi") },
    AutoCmd { trigger: "Pi", build: || greek("\u{03A0}", "Pi") },
    AutoCmd { trigger: "Sigma", build: || greek("\u{03A3}", "Sigma") },
    AutoCmd { trigger: "Phi", build: || greek("\u{03A6}", "Phi") },
    AutoCmd { trigger: "Psi", build: || greek("\u{03A8}", "Psi") },
    AutoCmd { trigger: "Omega", build: || greek("\u{03A9}", "Omega") },
    // Common symbols
    AutoCmd { trigger: "infty", build: || greek("\u{221E}", "infty") },
    AutoCmd { trigger: "forall", build: || greek("\u{2200}", "forall") },
    AutoCmd { trigger: "exists", build: || greek("\u{2203}", "exists") },
    AutoCmd { trigger: "partial", build: || greek("\u{2202}", "partial") },
    AutoCmd { trigger: "nabla", build: || greek("\u{2207}", "nabla") },
    AutoCmd { trigger: "pm", build: || greek("\u{00B1}", "pm") },
    AutoCmd { trigger: "mp", build: || greek("\u{2213}", "mp") },
    AutoCmd { trigger: "times", build: || greek("\u{00D7}", "times") },
    AutoCmd { trigger: "div", build: || greek("\u{00F7}", "div") },
    AutoCmd { trigger: "neq", build: || greek("\u{2260}", "neq") },
    AutoCmd { trigger: "leq", build: || greek("\u{2264}", "leq") },
    AutoCmd { trigger: "geq", build: || greek("\u{2265}", "geq") },
    AutoCmd { trigger: "approx", build: || greek("\u{2248}", "approx") },
    // Arrows and logical
    AutoCmd { trigger: "to", build: || greek("\u{2192}", "to") },
    AutoCmd { trigger: "gets", build: || greek("\u{2190}", "gets") },
    AutoCmd { trigger: "implies", build: || greek("\u{21D2}", "implies") },
    AutoCmd { trigger: "iff", build: || greek("\u{21D4}", "iff") },
    AutoCmd { trigger: "mapsto", build: || greek("\u{21A6}", "mapsto") },
    // Set theory
    AutoCmd { trigger: "subset", build: || greek("\u{2282}", "subset") },
    AutoCmd { trigger: "supset", build: || greek("\u{2283}", "supset") },
    AutoCmd { trigger: "cup", build: || greek("\u{222A}", "cup") },
    AutoCmd { trigger: "cap", build: || greek("\u{2229}", "cap") },
    AutoCmd { trigger: "emptyset", build: || greek("\u{2205}", "emptyset") },
    // Misc
    AutoCmd { trigger: "cdot", build: || greek("\u{22C5}", "cdot") },
    AutoCmd { trigger: "ldots", build: || greek("\u{2026}", "ldots") },
    AutoCmd { trigger: "cdots", build: || greek("\u{22EF}", "cdots") },
    AutoCmd { trigger: "propto", build: || greek("\u{221D}", "propto") },
    AutoCmd { trigger: "perp", build: || greek("\u{22A5}", "perp") },
    AutoCmd { trigger: "parallel", build: || greek("\u{2225}", "parallel") },
];

fn large_op(name: &str) -> MathNode {
    MathNode::Sub {
        base: Box::new(MathNode::Seq(vec![MathNode::Symbol(SymbolData {
            ch: name.to_string(),
            ctrl_seq: format!("\\{name} "),
            kind: SymbolKind::Operator,
        })])),
        script: Box::new(MathNode::empty_seq()),
    }
}

const AUTO_STRUCTURES: &[AutoCmd] = &[
    AutoCmd {
        trigger: "sqrt",
        build: || MathNode::Sqrt {
            index: None,
            radicand: Box::new(MathNode::empty_seq()),
        },
    },
    AutoCmd {
        trigger: "abs",
        build: || MathNode::Parens {
            open: BracketKind::Pipe,
            close: BracketKind::Pipe,
            body: Box::new(MathNode::empty_seq()),
        },
    },
    AutoCmd {
        trigger: "sum",
        build: || large_op("sum"),
    },
    AutoCmd {
        trigger: "int",
        build: || MathNode::Symbol(SymbolData {
            ch: "int".to_string(),
            ctrl_seq: "\\int ".to_string(),
            kind: SymbolKind::Operator,
        }),
    },
    AutoCmd {
        trigger: "prod",
        build: || large_op("prod"),
    },
    AutoCmd {
        trigger: "norm",
        build: || MathNode::Parens {
            open: BracketKind::DoublePipe,
            close: BracketKind::DoublePipe,
            body: Box::new(MathNode::empty_seq()),
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn var(ch: &str) -> MathNode {
        MathNode::Symbol(SymbolData::variable(ch))
    }

    #[test]
    fn detect_sin() {
        let seq = vec![var("s"), var("i"), var("n")];
        let result = check_auto_operator(&seq, 3);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        if let MathNode::Symbol(data) = &node {
            assert_eq!(data.ctrl_seq, "\\sin ");
        } else {
            panic!("expected Symbol");
        }
    }

    #[test]
    fn detect_sin_after_other_content() {
        let seq = vec![var("x"), var("s"), var("i"), var("n")];
        let result = check_auto_operator(&seq, 4);
        assert!(result.is_some());
        let (start, _) = result.unwrap();
        assert_eq!(start, 1);
    }

    #[test]
    fn no_match_partial() {
        let seq = vec![var("s"), var("i")];
        assert!(check_auto_operator(&seq, 2).is_none());
    }

    #[test]
    fn detect_alpha() {
        let seq = vec![var("a"), var("l"), var("p"), var("h"), var("a")];
        let result = check_auto_symbol(&seq, 5);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        if let MathNode::Symbol(data) = &node {
            assert_eq!(data.ctrl_seq, "\\alpha ");
            assert_eq!(data.ch, "\u{03B1}");
        } else {
            panic!("expected Symbol");
        }
    }

    #[test]
    fn detect_pi() {
        let seq = vec![var("p"), var("i")];
        let result = check_auto_symbol(&seq, 2);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        if let MathNode::Symbol(data) = &node {
            assert_eq!(data.ctrl_seq, "\\pi ");
        } else {
            panic!("expected Symbol");
        }
    }

    #[test]
    fn detect_sqrt_structure() {
        let seq = vec![var("s"), var("q"), var("r"), var("t")];
        let result = check_auto_structure(&seq, 4);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        assert!(matches!(node, MathNode::Sqrt { .. }));
    }

    #[test]
    fn detect_sum_structure() {
        let seq = vec![var("s"), var("u"), var("m")];
        let result = check_auto_structure(&seq, 3);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        assert!(matches!(node, MathNode::Sub { .. }));
    }

    #[test]
    fn detect_abs_structure() {
        let seq = vec![var("a"), var("b"), var("s")];
        let result = check_auto_structure(&seq, 3);
        assert!(result.is_some());
        let (start, node) = result.unwrap();
        assert_eq!(start, 0);
        assert!(matches!(node, MathNode::Parens { .. }));
    }

    #[test]
    fn no_match_non_variables() {
        let seq = vec![
            MathNode::Symbol(SymbolData::digit('1')),
            var("i"),
            var("n"),
        ];
        assert!(check_auto_operator(&seq, 3).is_none());
    }
}
