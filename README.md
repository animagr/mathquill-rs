# mathquill-rs

A native Rust WYSIWYG math editor inspired by [MathQuill](https://github.com/mathquill/mathquill), using [RaTeX](https://github.com/erweixin/RaTeX) for rendering and [egui](https://github.com/emilk/egui) for the desktop UI.

Users type math formulas with structured cursor navigation -- moving into and out of fractions, superscripts, subscripts, square roots, and parentheses -- just like MathQuill, but with no DOM, no JavaScript, and no browser dependency.

## Architecture

```
Keyboard Input
      |
      v
+------------------+
|   Editor Model   |   src/editor/    MathNode AST + path-based Cursor
+--------+---------+
         |
         v
+------------------+
| LaTeX Serializer |   src/latex.rs   Walks the tree, emits LaTeX + cursor mappings
+--------+---------+
         |
         v
+------------------+
|      RaTeX       |   ratex-parser -> ratex-layout -> ratex-render
| LaTeX -> Layout  |   Produces layout metadata and PNG bytes via tiny-skia
+--------+---------+
         |
         v
+------------------+
|   egui Widget    |   src/ui/        Uploads PNG as texture, handles focus/input
+------------------+
```

On every keystroke that mutates the AST:

```
mutate tree -> serialize to mapped LaTeX -> RaTeX parse+layout+render -> upload PNG texture -> display
```

RaTeX's `embed-fonts` feature compiles KaTeX font data into the binary, so no runtime font directory is needed.

## How it differs from MathQuill

| Aspect | MathQuill (JS) | mathquill-rs (Rust) |
|--------|---------------|---------------------|
| Tree structure | Doubly-linked nodes with `[L]`/`[R]`/`parent` pointers | Recursive `MathNode` enum with `Seq(Vec<MathNode>)` containers |
| Cursor | Pointer-based `(parent, [L], [R])` | Path-based `Vec<CursorStep>` from root to current position |
| Rendering | DOM manipulation + KaTeX CSS | RaTeX pipeline (parse -> layout -> PNG via tiny-skia) |
| UI framework | Browser DOM | egui (immediate mode, cross-platform) |
| Undo/redo | Not built-in | Snapshot-based stack with 200-level depth |

The editor model is UI-agnostic -- it knows nothing about rendering or egui.

## RaTeX integration

[RaTeX](https://github.com/erweixin/RaTeX) is a Rust reimplementation of KaTeX's rendering pipeline. This project uses six RaTeX crates:

- **ratex-parser** -- Parses LaTeX strings into an AST
- **ratex-layout** -- Lays out the AST into positioned boxes
- **ratex-render** -- Renders the layout to PNG via tiny-skia
- **ratex-types** -- Shared types (`DisplayList`, `DisplayItem`)
- **ratex-font** -- Font metrics and glyph data
- **ratex-font-loader** -- Font loading (with `embed-fonts` for compile-time embedding)

The `RenderCache` in `src/ui/renderer.rs` caches the last rendered LaTeX string together with PNG bytes, RaTeX layout boxes, display-list metadata, and render metrics. The UI uses that metadata for cursor overlay placement and click hit testing.

## Current implementation status

### Complete

- **Math AST** (`src/editor/tree.rs`) -- `MathNode` enum with `Seq`, `Symbol`, `Fraction`, `Sqrt`, `Sup`, `Sub`, `SupSub`, `Parens`, `Style`, `Matrix`, `Text`
- **Path-based cursor** (`src/editor/cursor.rs`) -- Navigation into/out of compound structures, up/down movement between fraction numerator/denominator and sup/sub, and tabbing through matrix cells
- **Input handling** (`src/editor/input.rs`) -- Character dispatch (`/` for fraction, `^`/`_` for scripts, `(` `[` `{` `|` for delimiters), backspace and delete-forward with compound node unwrapping
- **Structured commands** (`src/editor/commands.rs`) -- LiveFraction leftward scan, Sup/Sub creation with Sup-to-SupSub upgrade
- **Selection** (`src/editor/selection.rs`) -- Shift+arrow selection, Ctrl+A select all, backspace/delete/typing replaces selection, wrap selection in structures (`/`, `^`, `_`, `(`, etc.)
- **Clipboard** -- Ctrl+C copy, Ctrl+X cut, Ctrl+V paste (replays characters through the editor)
- **Auto-operators** (`src/editor/auto_cmds.rs`) -- Typing `sin`, `cos`, `tan`, `log`, `ln`, `lim`, `exp`, `min`, `max`, `det`, `gcd`, etc. auto-converts to `\sin`, `\cos`, etc.
- **Auto-symbols** -- Typing `alpha`, `beta`, `pi`, `theta`, `sigma`, `infty`, `nabla`, `leq`, `geq`, `neq`, `approx`, `pm`, `times`, `div`, `to`, `implies`, `iff`, `subset`, `cup`, `cdot`, etc. auto-converts to the corresponding LaTeX command
- **Auto-structures** -- Typing `sqrt` creates `\sqrt{}` with cursor inside, `abs` creates `|...|`, `norm` creates `||...||`, `sum`/`prod` creates large operators with subscript, `int` creates `\int`
- **LaTeX serializer** (`src/latex.rs`) -- Full round-trip from AST to LaTeX string, plus mapped node spans and cursor insertion positions
- **RaTeX rendering** (`src/ui/renderer.rs`) -- Cached render pipeline with PNG output, layout boxes, display-list metadata, and render metrics
- **Cursor overlay** (`src/ui/cursor_overlay.rs`) -- Layout-backed cursor positioning for root sequences, fractions, scripts, radicals, parens, and matrix cells, with a visual-width fallback
- **Click-to-place cursor** (`src/ui/math_widget.rs`) -- Clicks focus the editor and place the cursor at the nearest rendered insertion point; hit testing uses render-space x/y distances for supported compound nodes
- **egui widget** (`src/ui/math_widget.rs`) -- Focus, keyboard input, cursor blink, texture display, and cursor placement
- **Matrices and environments** -- Core AST, cursor navigation, insertion API, LaTeX serialization, and mapping support for `matrix`, `pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix`, and `smallmatrix`
- **Tab / Shift+Tab navigation** -- Tab moves forward between fields in compound nodes (numerator→denominator, base→exponent→subscript, index→radicand); Shift+Tab moves backward
- **Home/End keys** -- Jump to start/end of current sequence
- **Undo/redo** (`src/editor/undo.rs`) -- Snapshot stack with 200-level depth, Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y
- **Toolbar** (`src/app.rs`) -- Demo app buttons for fraction, superscript, subscript, sqrt, nth-root, parentheses, brackets, abs
- **116 unit tests** across all modules

### Not yet implemented

- **Full MathQuill-style seek behavior** -- Click hit testing is layout-backed and y-aware for supported structures, but exact recursive before/after behavior around compound bounds and drag selection are still pending
- **Matrix UI controls** -- Core matrix AST/serialization/navigation exists, but toolbar or keyboard insertion controls are not yet wired into the demo UI
- **Display modes** -- Inline vs. display math sizing
- **Accessibility** -- Screen reader support

## Prerequisites

### Rust toolchain

Install Rust via [rustup](https://rustup.rs/):

- **Windows**: Download and run [rustup-init.exe](https://win.rustup.rs/x86_64). This installs `rustc`, `cargo`, and `clippy`. You will also need the [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (select "Desktop development with C++" workload).
- **macOS**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Linux**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

Verify with `rustc --version` (requires Rust 1.75+).

### RaTeX

This project depends on [RaTeX](https://github.com/erweixin/RaTeX) via local path references (`../RaTeX/`). Clone both repos as siblings:

```bash
git clone https://github.com/erweixin/RaTeX.git
git clone https://github.com/animagr/mathquill-rs.git
```

Your directory layout should be:

```
parent/
  RaTeX/
  mathquill-rs/
```

## Building

```bash
cd mathquill-rs
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

To run the demo app:

```bash
cargo run
```

The demo window has a toolbar, a math editor (click to focus and place the cursor), and a live LaTeX output display. Type math naturally -- `/` for fractions, `^`/`_` for scripts, letter sequences like `sin`, `alpha`, `sqrt` auto-convert. Use Tab to move between fields, arrow keys to navigate, and Escape to unfocus. Collapsible panels at the bottom list all keyboard shortcuts and auto-commands.

### Testing the editor model

Run all 116 tests:

```bash
cargo test
```

Run tests for a specific module:

```bash
cargo test editor::input       # Input handling, auto-commands, selection
cargo test editor::cursor      # Cursor navigation, tab, home/end
cargo test editor::auto_cmds   # Auto-operator/symbol/structure detection
cargo test editor::commands    # Fraction/script insertion logic
cargo test editor::selection   # Selection range and node extraction
cargo test latex               # LaTeX serialization
cargo test ui::renderer        # RaTeX render pipeline
cargo test ui::cursor_overlay  # Cursor overlay and click hit testing
```

Run a single test by name:

```bash
cargo test editor::input::tests::wrap_selection_in_fraction
```

## Project structure

```
src/
  lib.rs              Crate root
  main.rs             Demo app entry point (eframe)
  app.rs              Demo application with MathWidget
  latex.rs            MathNode -> LaTeX serializer
  editor/
    mod.rs
    tree.rs           MathNode enum, SymbolData, BracketKind, StyleKind, MatrixKind
    cursor.rs         Path-based cursor with CursorStep enum
    input.rs          Editor struct, character/key dispatch
    commands.rs       Fraction/script insertion logic
    selection.rs      Selection model (anchor + range within a Seq)
    auto_cmds.rs      Auto-operators and auto-symbols detection
    undo.rs           Undo/redo snapshot stack
  ui/
    mod.rs
    math_widget.rs    egui widget (focus, input, render, display)
    cursor_overlay.rs Cursor positioning and click hit testing
    renderer.rs       RaTeX render cache, layout metadata, PNG-to-texture conversion
```

## License

See repository root.
