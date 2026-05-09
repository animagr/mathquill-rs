//! The editor model: math AST, cursor, input handling, and undo.
//!
//! This module is UI-agnostic — it knows nothing about rendering or egui.

pub mod auto_cmds;
pub mod commands;
pub mod cursor;
pub mod input;
pub mod selection;
pub mod tree;
pub mod undo;

pub use input::Editor;
