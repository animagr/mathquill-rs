# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-05-12

### Changed

- Cursor overlay positioning now accounts for the full cursor path through the math tree, so it moves across the rendered input instead of staying pinned to the left edge.
- Added focused tests for cursor overlay offset behavior.

## [0.1.1] - 2026-05-09

### Added

- Selection model: Shift+Left/Right to select, Ctrl+A to select all, backspace/delete/typing replaces selection
- Wrap selection in structures: select text then press `/`, `^`, `_`, `(`, `[`, `{`, `|` to wrap in fraction, sup/sub, or delimiters
- Clipboard support: Ctrl+C to copy, Ctrl+X to cut, Ctrl+V to paste (as LaTeX characters)
- Auto-operators: typing `sin`, `cos`, `tan`, `log`, `ln`, `lim`, `exp`, `min`, `max`, `det`, `gcd`, etc. auto-converts to `\sin`, `\cos`, etc.
- Auto-symbols: Greek letters (`alpha`, `beta`, `pi`, `theta`, `sigma`, etc.), relations (`leq`, `geq`, `neq`, `approx`), arrows (`to`, `implies`, `iff`, `mapsto`, `gets`), set theory (`subset`, `supset`, `cup`, `cap`, `emptyset`), misc (`infty`, `nabla`, `pm`, `times`, `div`, `cdot`, `ldots`, `cdots`, `propto`, `perp`, `parallel`)
- Auto-structures: typing `sqrt` creates `\sqrt{}`, `abs` creates `|...|`, `norm` creates `||...||`, `sum`/`prod` creates large operators with subscript, `int` creates `\int`
- Smart auto-command matching: longest match wins across all tables; prefix suppression prevents short triggers (e.g., `mp`) from firing inside partially-typed longer triggers (e.g., `implies`)
- Tab / Shift+Tab navigation between fields in compound nodes (numerator↔denominator, base↔exponent↔subscript, index↔radicand)
- Home/End keys to jump to start/end of current sequence
- Curly brace `{`, pipe `|` delimiters, plus `!`, `%`, `:`, `;`, `~` (as `\sim`) character inputs
- Nth-root insertion (`\sqrt[n]{}`) via `Editor::insert_nth_root` and toolbar button
- `\text{}` block insertion via `Editor::insert_text_block`
- Delete-forward now unwraps compound nodes (like backspace), spilling contents inline
- Backspace unwraps `SupSub` and `Sqrt` nodes; exits left when at position 0 inside compound nodes
- Undo (`Ctrl+Z`) and redo (`Ctrl+Shift+Z` / `Ctrl+Y`) keyboard shortcuts
- Toolbar in demo app with buttons for fraction, superscript, subscript, sqrt, nth-root, brackets, and abs
- Help panels with keyboard shortcuts and auto-command reference
- 50 new unit tests (86 total)

## [0.1.0] - 2026-05-09

### Added

- Math AST (`MathNode` enum) with `Seq`, `Symbol`, `Fraction`, `Sqrt`, `Sup`, `Sub`, `SupSub`, `Parens`, `Style`, `Text` variants
- Path-based cursor (`Vec<CursorStep>`) with navigation into/out of compound structures
- Up/down movement between fraction numerator/denominator and superscript/subscript
- Input handling: `/` for fraction, `^`/`_` for scripts, `(`/`[` for parens, digits, letters, operators
- LiveFraction leftward scan (wraps content left of cursor into numerator, stopping at operators)
- Sup-to-SupSub upgrade when adding a subscript to an existing superscript (and vice versa)
- Backspace with compound node unwrapping (deleting a fraction spills its numerator back inline)
- LaTeX serializer (`MathNode` -> LaTeX string) compatible with RaTeX's parser
- RaTeX rendering pipeline with cached PNG output (parse -> layout -> render via tiny-skia)
- Embedded KaTeX fonts via `ratex-font-loader`'s `embed-fonts` feature
- egui widget with click-to-focus, keyboard input, cursor blink animation, and texture display
- Undo/redo snapshot stack (200-level depth)
- eframe demo application showing the widget with live LaTeX output
- 36 unit tests across all modules

[0.1.2]: https://github.com/animagr/mathquill-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/animagr/mathquill-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/animagr/mathquill-rs/releases/tag/v0.1.0
