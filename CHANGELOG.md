# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
