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
| LaTeX Serializer |   src/latex.rs   Walks the tree, emits LaTeX string
+--------+---------+
         |
         v
+------------------+
|      RaTeX       |   ratex-parser -> ratex-layout -> ratex-render
|   LaTeX -> PNG   |   Produces PNG bytes via tiny-skia
+--------+---------+
         |
         v
+------------------+
|   egui Widget    |   src/ui/        Uploads PNG as texture, handles focus/input
+------------------+
```

On every keystroke that mutates the AST:

```
mutate tree -> serialize to LaTeX -> RaTeX parse+layout+render -> upload PNG texture -> display
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

The `RenderCache` in `src/ui/renderer.rs` caches the last rendered LaTeX string and PNG bytes, only re-rendering when the AST changes.

## Current implementation status

### Complete

- **Math AST** (`src/editor/tree.rs`) -- `MathNode` enum with `Seq`, `Symbol`, `Fraction`, `Sqrt`, `Sup`, `Sub`, `SupSub`, `Parens`, `Style`, `Text`
- **Path-based cursor** (`src/editor/cursor.rs`) -- Navigation into/out of compound structures, up/down movement between fraction numerator/denominator and sup/sub
- **Input handling** (`src/editor/input.rs`) -- Character dispatch (`/` for fraction, `^`/`_` for scripts, `(` `[` for parens), backspace with compound node unwrapping
- **Structured commands** (`src/editor/commands.rs`) -- LiveFraction leftward scan, Sup/Sub creation with Sup-to-SupSub upgrade
- **LaTeX serializer** (`src/latex.rs`) -- Full round-trip from AST to LaTeX string
- **RaTeX rendering** (`src/ui/renderer.rs`) -- Cached render pipeline with PNG output
- **egui widget** (`src/ui/math_widget.rs`) -- Focus, keyboard input, cursor blink, texture display
- **Undo/redo** (`src/editor/undo.rs`) -- Snapshot stack with push/pop/undo/redo
- **36 unit tests** across all modules

### Not yet implemented

- **Selection** -- Selecting ranges of nodes, cut/copy/paste
- **Click-to-place cursor** -- Mapping mouse coordinates to AST node positions (requires DisplayList-to-AST mapping)
- **Cursor overlay rendering** -- Drawing the cursor at the correct position within the rendered math (current placeholder uses a fixed-width estimate)
- **Auto-operators** -- Typing `sin` automatically converting to `\sin`
- **Greek letter shortcuts** -- Typing `alpha` converting to `\alpha`
- **Matrices and environments** -- `\begin{pmatrix}...\end{pmatrix}`
- **Toolbar / button input** -- GUI buttons for inserting structures
- **Accessibility** -- Screen reader support

## Building

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

To run the demo app:

```bash
cargo run
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
    tree.rs           MathNode enum, SymbolData, BracketKind, StyleKind
    cursor.rs         Path-based cursor with CursorStep enum
    input.rs          Editor struct, character/key dispatch
    commands.rs       Fraction/script insertion logic
    undo.rs           Undo/redo snapshot stack
  ui/
    mod.rs
    math_widget.rs    egui widget (focus, input, render, display)
    renderer.rs       RaTeX render cache, PNG-to-texture conversion
```

## License

See repository root.
