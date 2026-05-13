//! Cursor overlay positioning for the egui math widget.

use egui::{Pos2, Rect};
use ratex_layout::layout_box::{BoxContent, LayoutBox};

use super::renderer::RenderedMath;
use crate::editor::{
    cursor::{Cursor, CursorStep},
    tree::MathNode,
    Editor,
};
use crate::latex::to_latex_with_mapping;

const FALLBACK_RENDER_PADDING: f32 = 10.0;
const MIN_VISUAL_WIDTH: f32 = 1.0;
const EMPTY_VISUAL_OFFSET: f32 = 0.0;
const EMPTY_LAYOUT_OFFSET: f64 = 0.0;
const RADICAL_INDEX_KERN: f64 = 5.0 / 18.0;
const STRUCTURE_EXTRA_WIDTH: f32 = 1.5;
const SCRIPT_WIDTH_SCALE: f32 = 0.65;
const PARENS_EXTRA_WIDTH: f32 = 2.0;
const FRACTION_SLOT_OFFSET: f32 = 0.75;
const SQRT_INDEX_OFFSET: f32 = 0.5;
const INNER_CONTENT_OFFSET: f32 = 1.0;
const CURSOR_PATH_SLOT_OFFSET: usize = 1;
const CURSOR_PATH_SLOT_STRIDE: usize = 2;
const PADDING_MULTIPLIER: f32 = 2.0;
const HIT_TEST_TIE_BREAK_EPSILON: f64 = 1.0e-9;
const HIT_TEST_VERTICAL_WEIGHT: f64 = 1.25;
const CURSOR_ANCHOR_MIN_ASCENT: f64 = 0.35;
const CURSOR_ANCHOR_MAX_ASCENT: f64 = 0.9;
const CURSOR_ANCHOR_MIN_DESCENT: f64 = 0.15;
const CURSOR_ANCHOR_MAX_DESCENT: f64 = 0.35;
const CURSOR_DISPLAY_MIN_ASCENT: f64 = 0.45;
const CURSOR_DISPLAY_MIN_DESCENT: f64 = 0.12;

/// Cursor position and vertical extent in egui logical points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CursorPosition {
    x: f32,
    top: f32,
    bottom: f32,
}

impl CursorPosition {
    /// Horizontal cursor position in the widget rectangle.
    #[must_use]
    pub(crate) fn x(self) -> f32 {
        self.x
    }

    /// Top of the cursor line in the widget rectangle.
    #[must_use]
    pub(crate) fn top(self) -> f32 {
        self.top
    }

    /// Bottom of the cursor line in the widget rectangle.
    #[must_use]
    pub(crate) fn bottom(self) -> f32 {
        self.bottom
    }
}

/// Calculate the cursor overlay position for the current editor state.
#[must_use]
pub(crate) fn cursor_position_for_rect(
    editor: &Editor,
    rendered: Option<&RenderedMath>,
    rect: Rect,
) -> CursorPosition {
    let content_rect = rendered.map_or_else(
        || fallback_content_rect(rect),
        |rendered| rendered_content_rect(rendered, rect),
    );

    if let Some(rendered) = rendered {
        let layout_offset = layout_cursor_offset(&editor.root, &editor.cursor, rendered);
        let anchor =
            layout_cursor_anchor(&editor.root, &editor.cursor, rendered, AnchorMode::Display);

        if let Some(offset) = layout_offset {
            let width = f64_to_f32(
                rendered
                    .display_list()
                    .width
                    .max(rendered.layout_box().width),
            );
            if width > 0.0 {
                let x = content_rect.left() + (f64_to_f32(offset) * content_rect.width() / width);
                let (top, bottom) = anchor.map_or((rect.top() + 4.0, rect.bottom() - 4.0), |a| {
                    layout_y_to_screen(a, rendered, content_rect, rect)
                });
                return CursorPosition {
                    x: x.clamp(content_rect.left(), content_rect.right()),
                    top,
                    bottom,
                };
            }
        }
    }

    fallback_cursor_position(&editor.root, &editor.cursor, content_rect)
}

fn layout_y_to_screen(
    anchor: CursorAnchor,
    rendered: &RenderedMath,
    content_rect: Rect,
    widget_rect: Rect,
) -> (f32, f32) {
    let layout_height = rendered
        .display_list()
        .total_height()
        .max(rendered.layout_box().height + rendered.layout_box().depth);

    if layout_height <= 0.0 || content_rect.height() <= 0.0 {
        return (widget_rect.top() + 4.0, widget_rect.bottom() - 4.0);
    }

    let scale = f32_to_f64(content_rect.height()) / layout_height;
    let top = f32_to_f64(content_rect.top()) + anchor.top * scale;
    let bottom = f32_to_f64(content_rect.top()) + anchor.bottom * scale;
    (
        f64_to_f32(top).clamp(widget_rect.top(), widget_rect.bottom()),
        f64_to_f32(bottom).clamp(widget_rect.top(), widget_rect.bottom()),
    )
}

/// Calculate cursor position for an arbitrary cursor path (used for selection highlights).
#[must_use]
pub(crate) fn cursor_position_for_path(
    root: &MathNode,
    cursor: &Cursor,
    rendered: Option<&RenderedMath>,
    rect: Rect,
) -> CursorPosition {
    let content_rect = rendered.map_or_else(
        || fallback_content_rect(rect),
        |rendered| rendered_content_rect(rendered, rect),
    );

    if let Some(rendered) = rendered {
        let layout_offset = layout_cursor_offset(root, cursor, rendered);
        let anchor = layout_cursor_anchor(root, cursor, rendered, AnchorMode::Display);

        if let Some(offset) = layout_offset {
            let width = f64_to_f32(
                rendered
                    .display_list()
                    .width
                    .max(rendered.layout_box().width),
            );
            if width > 0.0 {
                let x = content_rect.left() + (f64_to_f32(offset) * content_rect.width() / width);
                let (top, bottom) = anchor.map_or((rect.top() + 4.0, rect.bottom() - 4.0), |a| {
                    layout_y_to_screen(a, rendered, content_rect, rect)
                });
                return CursorPosition {
                    x: x.clamp(content_rect.left(), content_rect.right()),
                    top,
                    bottom,
                };
            }
        }
    }

    fallback_cursor_position(root, cursor, content_rect)
}

/// Find the closest editor cursor for a point inside the rendered widget.
#[must_use]
pub(crate) fn cursor_for_point(
    editor: &Editor,
    rendered: Option<&RenderedMath>,
    rect: Rect,
    point: Pos2,
) -> Option<Cursor> {
    let content_rect = rendered.map_or_else(
        || fallback_content_rect(rect),
        |rendered| rendered_content_rect(rendered, rect),
    );

    if let Some(rendered) = rendered {
        let layout_x = point_to_layout_x(rendered, content_rect, point.x)?;
        let layout_y = point_to_layout_y(rendered, content_rect, point.y)?;
        if let Some(cursor) = seek_recursive(&editor.root, rendered, layout_x, layout_y) {
            return Some(cursor);
        }
        if let Some(cursor) = closest_layout_cursor(&editor.root, rendered, layout_x, layout_y) {
            return Some(cursor);
        }
    }

    closest_visual_cursor(&editor.root, content_rect, point.x)
}

fn rendered_content_rect(rendered: &RenderedMath, rect: Rect) -> Rect {
    let metrics = rendered.metrics();
    let content_width_px = f64_to_f32(rendered.display_list().width)
        * metrics.font_size()
        * metrics.device_pixel_ratio();
    let padding_px = metrics.padding() * metrics.device_pixel_ratio();
    let total_width_px = content_width_px + (padding_px * PADDING_MULTIPLIER);

    if total_width_px <= 0.0 || rect.width() <= 0.0 {
        return fallback_content_rect(rect);
    }

    let left_padding = rect.width() * (padding_px / total_width_px);
    Rect::from_min_max(
        egui::pos2(rect.left() + left_padding, rect.top()),
        egui::pos2(rect.right() - left_padding, rect.bottom()),
    )
}

fn point_to_layout_x(rendered: &RenderedMath, content_rect: Rect, point_x: f32) -> Option<f64> {
    let layout_width = rendered
        .display_list()
        .width
        .max(rendered.layout_box().width);

    if layout_width <= 0.0 || content_rect.width() <= 0.0 {
        return None;
    }

    let ratio = ((point_x - content_rect.left()) / content_rect.width()).clamp(0.0, 1.0);
    Some(f32_to_f64(ratio) * layout_width)
}

fn point_to_layout_y(rendered: &RenderedMath, content_rect: Rect, point_y: f32) -> Option<f64> {
    let layout_height = rendered
        .display_list()
        .total_height()
        .max(rendered.layout_box().height + rendered.layout_box().depth);

    if layout_height <= 0.0 || content_rect.height() <= 0.0 {
        return None;
    }

    let ratio = ((point_y - content_rect.top()) / content_rect.height()).clamp(0.0, 1.0);
    Some(f32_to_f64(ratio) * layout_height)
}

fn closest_layout_cursor(
    root: &MathNode,
    rendered: &RenderedMath,
    layout_x: f64,
    layout_y: f64,
) -> Option<Cursor> {
    let mapped = to_latex_with_mapping(root);
    let mut best: Option<CursorHit> = None;

    for position in mapped.cursor_positions() {
        let cursor = Cursor::from_path(position.path().to_vec());
        let Some(anchor) = layout_cursor_anchor(root, &cursor, rendered, AnchorMode::HitTest)
        else {
            continue;
        };
        let distance = anchor.distance_to(layout_x, layout_y);
        let depth = cursor.path().len();
        let hit = CursorHit {
            cursor,
            distance,
            depth,
        };

        let should_replace = match &best {
            Some(best_hit) => hit.is_better_than(best_hit),
            None => true,
        };

        if should_replace {
            best = Some(hit);
        }
    }

    best.map(|hit| hit.cursor)
}

/// Recursive seek: walks the layout tree top-down, using compound node
/// bounding boxes to decide whether to recurse into child slots.
fn seek_recursive(
    root: &MathNode,
    rendered: &RenderedMath,
    layout_x: f64,
    layout_y: f64,
) -> Option<Cursor> {
    let context = LayoutContext {
        x: EMPTY_LAYOUT_OFFSET,
        baseline_y: rendered.display_list().height,
        scale: 1.0,
    };
    let path = seek_in_seq(
        root,
        rendered.layout_box(),
        layout_x,
        layout_y,
        context,
        Vec::new(),
    )?;
    Some(Cursor::from_path(path))
}

fn seek_in_seq(
    node: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    mut prefix: Vec<CursorStep>,
) -> Option<Vec<CursorStep>> {
    let children = node.as_seq()?;
    if children.is_empty() {
        prefix.push(CursorStep::SeqPos(0));
        return Some(prefix);
    }

    let logical = logical_child_boxes(children, layout_box)?;

    for (i, child) in children.iter().enumerate() {
        let cl = logical.get(i)?;
        let abs_left = context.x + (cl.x * context.scale);
        let abs_right = abs_left + (cl.layout_box.width * context.scale);

        if x < abs_left {
            prefix.push(CursorStep::SeqPos(i));
            return Some(prefix);
        }

        if x > abs_right {
            if i == children.len() - 1 {
                prefix.push(CursorStep::SeqPos(children.len()));
                return Some(prefix);
            }
            continue;
        }

        let child_ctx = LayoutContext {
            x: abs_left,
            baseline_y: context.baseline_y,
            scale: context.scale,
        };

        if let Some(path) = seek_into_compound(child, cl.layout_box, x, y, child_ctx, &prefix, i) {
            return Some(path);
        }

        let mid = (abs_left + abs_right) / 2.0;
        if x < mid {
            prefix.push(CursorStep::SeqPos(i));
        } else {
            prefix.push(CursorStep::SeqPos(i + 1));
        }
        return Some(prefix);
    }

    prefix.push(CursorStep::SeqPos(children.len()));
    Some(prefix)
}

fn seek_into_compound(
    node: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    match node {
        MathNode::Fraction { num, den } => {
            seek_into_fraction(num, den, layout_box, x, y, context, prefix, seq_index)
        }
        MathNode::Sup { .. } | MathNode::Sub { .. } | MathNode::SupSub { .. } => {
            seek_into_scripts(node, layout_box, x, y, context, prefix, seq_index)
        }
        MathNode::Sqrt { .. } => {
            seek_into_radical(node, layout_box, x, y, context, prefix, seq_index)
        }
        MathNode::Parens { body, .. } => {
            seek_into_leftright(body, layout_box, x, y, context, prefix, seq_index)
        }
        MathNode::Style { body, .. } => {
            let mut path = prefix.to_vec();
            path.push(CursorStep::SeqPos(seq_index));
            path.push(CursorStep::Inner);
            seek_in_seq(body, layout_box, x, y, context, path)
        }
        MathNode::Matrix { cells, .. } => {
            seek_into_matrix(cells, layout_box, x, y, context, prefix, seq_index)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn seek_into_fraction(
    num: &MathNode,
    den: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    let fraction = locate_fraction_box(layout_box)?;
    let BoxContent::Fraction {
        numer,
        denom,
        numer_shift,
        denom_shift,
        numer_scale,
        denom_scale,
        ..
    } = &fraction.layout_box.content
    else {
        return None;
    };

    let frac_ctx = LayoutContext {
        x: context.x + (fraction.x * context.scale),
        baseline_y: context.baseline_y,
        scale: context.scale,
    };

    let (slot, child, child_box, shift, scale) = if y < frac_ctx.baseline_y {
        (
            CursorStep::Numerator,
            num,
            &**numer,
            *numer_shift,
            *numer_scale,
        )
    } else {
        (
            CursorStep::Denominator,
            den,
            &**denom,
            *denom_shift,
            *denom_scale,
        )
    };

    let child_x = frac_ctx.x
        + (centered_child_x(fraction.layout_box.width, child_box.width, scale) * frac_ctx.scale);
    let child_baseline = if matches!(slot, CursorStep::Numerator) {
        frac_ctx.baseline_y - (shift * frac_ctx.scale)
    } else {
        frac_ctx.baseline_y + (shift * frac_ctx.scale)
    };

    let child_ctx = LayoutContext {
        x: child_x,
        baseline_y: child_baseline,
        scale: frac_ctx.scale * scale,
    };

    let mut path = prefix.to_vec();
    path.push(CursorStep::SeqPos(seq_index));
    path.push(slot);
    seek_in_seq(child, child_box, x, y, child_ctx, path)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn seek_into_scripts(
    node: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    let BoxContent::SupSub {
        base: base_box,
        sup: exponent_layout,
        sub: subscript_layout,
        sup_shift,
        sub_shift,
        sup_scale,
        sub_scale,
        center_scripts,
        italic_correction,
        sub_h_kern,
    } = &layout_box.content
    else {
        return None;
    };

    let (base_node, exponent_node, subscript_node) = match node {
        MathNode::Sup { base, exp } => (&**base, Some(&**exp), None),
        MathNode::Sub { base, script } => (&**base, None, Some(&**script)),
        MathNode::SupSub { base, sup, sub } => (&**base, Some(&**sup), Some(&**sub)),
        _ => return None,
    };

    let base_abs_x = if *center_scripts {
        context.x + (centered_child_x(layout_box.width, base_box.width, 1.0) * context.scale)
    } else {
        context.x
    };
    let base_abs_right = base_abs_x + (base_box.width * context.scale);

    if x < base_abs_right {
        let base_ctx = LayoutContext {
            x: base_abs_x,
            baseline_y: context.baseline_y,
            scale: context.scale,
        };
        let mut path = prefix.to_vec();
        path.push(CursorStep::SeqPos(seq_index));
        path.push(CursorStep::Base);
        return seek_in_seq(base_node, base_box, x, y, base_ctx, path);
    }

    let enter_sup = exponent_node.is_some() && (subscript_node.is_none() || y < context.baseline_y);

    if enter_sup {
        let exp_lb = exponent_layout.as_ref()?;
        let exp_node = exponent_node?;
        let exp_abs_x = if *center_scripts {
            context.x
                + (centered_child_x(layout_box.width, exp_lb.width, *sup_scale) * context.scale)
        } else {
            base_abs_x + ((base_box.width + italic_correction) * context.scale)
        };
        let exp_ctx = LayoutContext {
            x: exp_abs_x,
            baseline_y: context.baseline_y - (sup_shift * context.scale),
            scale: context.scale * sup_scale,
        };
        let mut path = prefix.to_vec();
        path.push(CursorStep::SeqPos(seq_index));
        path.push(CursorStep::Exponent);
        return seek_in_seq(exp_node, exp_lb, x, y, exp_ctx, path);
    }

    if let Some(script_node) = subscript_node {
        let script_lb = subscript_layout.as_ref()?;
        let script_abs_x = if *center_scripts {
            context.x
                + (centered_child_x(layout_box.width, script_lb.width, *sub_scale) * context.scale)
        } else {
            base_abs_x + ((base_box.width + sub_h_kern) * context.scale)
        };
        let script_ctx = LayoutContext {
            x: script_abs_x,
            baseline_y: context.baseline_y + (sub_shift * context.scale),
            scale: context.scale * sub_scale,
        };
        let mut path = prefix.to_vec();
        path.push(CursorStep::SeqPos(seq_index));
        path.push(CursorStep::Subscript);
        return seek_in_seq(script_node, script_lb, x, y, script_ctx, path);
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn seek_into_radical(
    node: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    let BoxContent::Radical {
        body,
        index: index_box,
        index_offset,
        index_scale,
        ..
    } = &layout_box.content
    else {
        return None;
    };

    let MathNode::Sqrt {
        index: idx_node,
        radicand,
    } = node
    else {
        return None;
    };

    let radical_width = layout_box.width - index_offset - body.width;
    let radicand_abs_x = context.x + ((index_offset + radical_width) * context.scale);

    if let (Some(idx), Some(idx_box)) = (idx_node.as_deref(), index_box.as_ref()) {
        let idx_abs_x = context.x + ((index_offset + RADICAL_INDEX_KERN) * context.scale);
        if x < radicand_abs_x && x >= idx_abs_x {
            let to_shift = 0.6 * (layout_box.height - layout_box.depth);
            let idx_ctx = LayoutContext {
                x: idx_abs_x,
                baseline_y: context.baseline_y - (to_shift * context.scale),
                scale: context.scale * index_scale,
            };
            let mut path = prefix.to_vec();
            path.push(CursorStep::SeqPos(seq_index));
            path.push(CursorStep::Index);
            return seek_in_seq(idx, idx_box, x, y, idx_ctx, path);
        }
    }

    let radicand_ctx = LayoutContext {
        x: radicand_abs_x,
        baseline_y: context.baseline_y,
        scale: context.scale,
    };
    let mut path = prefix.to_vec();
    path.push(CursorStep::SeqPos(seq_index));
    path.push(CursorStep::Radicand);
    seek_in_seq(radicand, body, x, y, radicand_ctx, path)
}

#[allow(clippy::too_many_arguments)]
fn seek_into_leftright(
    body: &MathNode,
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    let BoxContent::LeftRight { left, inner, .. } = &layout_box.content else {
        return None;
    };

    let inner_abs_left = context.x + (left.width * context.scale);
    let inner_abs_right = context.x + ((left.width + inner.width) * context.scale);

    if x < inner_abs_left || x > inner_abs_right {
        return None;
    }

    let inner_ctx = LayoutContext {
        x: inner_abs_left,
        baseline_y: context.baseline_y,
        scale: context.scale,
    };

    let mut path = prefix.to_vec();
    path.push(CursorStep::SeqPos(seq_index));
    path.push(CursorStep::Inner);
    seek_in_seq(body, inner, x, y, inner_ctx, path)
}

#[allow(clippy::too_many_arguments)]
fn seek_into_matrix(
    cells: &[Vec<MathNode>],
    layout_box: &LayoutBox,
    x: f64,
    y: f64,
    context: LayoutContext,
    prefix: &[CursorStep],
    seq_index: usize,
) -> Option<Vec<CursorStep>> {
    let num_rows = cells.len();
    let num_cols = cells.first().map_or(0, Vec::len);
    if num_rows == 0 || num_cols == 0 {
        return None;
    }

    let mut best_row = 0;
    let mut best_row_dist = f64::MAX;
    for row_idx in 0..num_rows {
        if let Some(cl) = matrix_cell_layout(layout_box, row_idx, 0, context) {
            let dist = (y - cl.context.baseline_y).abs();
            if dist < best_row_dist {
                best_row_dist = dist;
                best_row = row_idx;
            }
        }
    }

    let mut best_col = 0;
    let mut best_col_dist = f64::MAX;
    for col_idx in 0..num_cols {
        if let Some(cl) = matrix_cell_layout(layout_box, best_row, col_idx, context) {
            let mid_x = cl.context.x + (cl.layout_box.width * cl.context.scale / 2.0);
            let dist = (x - mid_x).abs();
            if dist < best_col_dist {
                best_col_dist = dist;
                best_col = col_idx;
            }
        }
    }

    let cell = cells.get(best_row)?.get(best_col)?;
    let cl = matrix_cell_layout(layout_box, best_row, best_col, context)?;

    let mut path = prefix.to_vec();
    path.push(CursorStep::SeqPos(seq_index));
    path.push(CursorStep::MatrixCell {
        row: best_row,
        col: best_col,
    });
    seek_in_seq(cell, cl.layout_box, x, y, cl.context, path)
}

fn closest_visual_cursor(root: &MathNode, content_rect: Rect, point_x: f32) -> Option<Cursor> {
    let children = root.as_seq()?;
    let total_width = visual_width(root).max(MIN_VISUAL_WIDTH);

    if content_rect.width() <= 0.0 {
        return Some(Cursor::new());
    }

    let ratio = ((point_x - content_rect.left()) / content_rect.width()).clamp(0.0, 1.0);
    let target_offset = ratio * total_width;
    let mut current_offset = EMPTY_VISUAL_OFFSET;

    for (index, child) in children.iter().enumerate() {
        let child_width = visual_width(child);
        let midpoint = current_offset + (child_width / 2.0);
        if target_offset <= midpoint {
            return Some(Cursor::at_root_pos(index));
        }
        current_offset += child_width;
    }

    Some(Cursor::at_root_pos(children.len()))
}

fn fallback_content_rect(rect: Rect) -> Rect {
    let content_left = rect.left() + FALLBACK_RENDER_PADDING;
    let content_right = rect.right() - FALLBACK_RENDER_PADDING;

    if content_right <= content_left {
        return rect;
    }

    Rect::from_min_max(
        egui::pos2(content_left, rect.top()),
        egui::pos2(content_right, rect.bottom()),
    )
}

fn fallback_cursor_position(
    root: &MathNode,
    cursor: &Cursor,
    content_rect: Rect,
) -> CursorPosition {
    let total_width = visual_width(root).max(MIN_VISUAL_WIDTH);
    let cursor_offset =
        cursor_visual_offset(root, cursor).unwrap_or_else(|| usize_to_f32(cursor.seq_pos()));
    let ratio = (cursor_offset / total_width).clamp(0.0, 1.0);

    CursorPosition {
        x: content_rect.left() + (content_rect.width() * ratio),
        top: content_rect.top(),
        bottom: content_rect.bottom(),
    }
}

fn layout_cursor_offset(root: &MathNode, cursor: &Cursor, rendered: &RenderedMath) -> Option<f64> {
    let mapped = to_latex_with_mapping(root);
    let cursor_is_mapped = mapped
        .cursor_positions()
        .iter()
        .any(|position| position.path() == cursor.path());

    if !cursor_is_mapped {
        return None;
    }

    offset_for_path(root, rendered.layout_box(), cursor.path())
}

fn layout_cursor_anchor(
    root: &MathNode,
    cursor: &Cursor,
    rendered: &RenderedMath,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let mapped = to_latex_with_mapping(root);
    let cursor_is_mapped = mapped
        .cursor_positions()
        .iter()
        .any(|position| position.path() == cursor.path());

    if !cursor_is_mapped {
        return None;
    }

    let context = LayoutContext {
        x: EMPTY_LAYOUT_OFFSET,
        baseline_y: rendered.display_list().height,
        scale: 1.0,
    };

    anchor_for_path(root, rendered.layout_box(), cursor.path(), context, mode)
}

fn anchor_for_path(
    node: &MathNode,
    layout_box: &LayoutBox,
    path: &[CursorStep],
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let (seq_pos, remaining) = path.split_first()?;
    let CursorStep::SeqPos(pos) = *seq_pos else {
        return None;
    };

    let children = node.as_seq()?;
    let local_x = sequence_cursor_offset(children, layout_box, pos)?;

    if remaining.is_empty() {
        let (height, depth) = neighbor_height_depth(children, layout_box, pos);
        return Some(cursor_anchor_with_dims(
            local_x, height, depth, context, mode,
        ));
    }

    let child = children.get(pos)?;
    let child_box = sequence_child_box(children, layout_box, pos)?;
    let child_context = LayoutContext {
        x: context.x + (child_box.x * context.scale),
        baseline_y: context.baseline_y,
        scale: context.scale,
    };
    let (slot, child_path) = remaining.split_first()?;

    slot_cursor_anchor(
        child,
        child_box.layout_box,
        *slot,
        child_path,
        child_context,
        mode,
    )
}

fn neighbor_height_depth(children: &[MathNode], layout_box: &LayoutBox, pos: usize) -> (f64, f64) {
    if children.is_empty() {
        return (layout_box.height, layout_box.depth);
    }

    let Some(logical) = logical_child_boxes(children, layout_box) else {
        return (layout_box.height, layout_box.depth);
    };

    let left = if pos > 0 { logical.get(pos - 1) } else { None };
    let right = logical.get(pos);

    match (left, right) {
        (Some(l), Some(r)) => (
            l.layout_box.height.max(r.layout_box.height),
            l.layout_box.depth.max(r.layout_box.depth),
        ),
        (Some(n), None) | (None, Some(n)) => (n.layout_box.height, n.layout_box.depth),
        (None, None) => (layout_box.height, layout_box.depth),
    }
}

fn offset_for_path(node: &MathNode, layout_box: &LayoutBox, path: &[CursorStep]) -> Option<f64> {
    let (seq_pos, remaining) = path.split_first()?;
    let CursorStep::SeqPos(pos) = *seq_pos else {
        return None;
    };

    let children = node.as_seq()?;
    let sequence_offset = sequence_cursor_offset(children, layout_box, pos)?;

    if remaining.is_empty() {
        return Some(sequence_offset);
    }

    let child = children.get(pos)?;
    let child_box = sequence_child_box(children, layout_box, pos)?;
    let (slot, child_path) = remaining.split_first()?;
    let slot_offset = slot_cursor_offset(child, child_box.layout_box, *slot, child_path)?;

    Some(child_box.x + slot_offset)
}

fn slot_cursor_anchor(
    node: &MathNode,
    layout_box: &LayoutBox,
    slot: CursorStep,
    path: &[CursorStep],
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    match (node, slot) {
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => fraction_slot_anchor(
            layout_box,
            num,
            path,
            FractionSlot::Numerator,
            context,
            mode,
        ),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => fraction_slot_anchor(
            layout_box,
            den,
            path,
            FractionSlot::Denominator,
            context,
            mode,
        ),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => radical_slot_anchor(
            layout_box,
            radicand,
            path,
            RadicalSlot::Radicand,
            context,
            mode,
        ),
        (
            MathNode::Sqrt {
                index: Some(index), ..
            },
            CursorStep::Index,
        ) => radical_slot_anchor(layout_box, index, path, RadicalSlot::Index, context, mode),
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Base,
        ) => supsub_slot_anchor(layout_box, base, path, SupSubSlot::Base, context, mode),
        (MathNode::Sup { exp, .. } | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => {
            supsub_slot_anchor(layout_box, exp, path, SupSubSlot::Sup, context, mode)
        }
        (
            MathNode::Sub { script, .. } | MathNode::SupSub { sub: script, .. },
            CursorStep::Subscript,
        ) => supsub_slot_anchor(layout_box, script, path, SupSubSlot::Sub, context, mode),
        (MathNode::Parens { body, .. }, CursorStep::Inner) => {
            leftright_slot_anchor(layout_box, body, path, context, mode)
        }
        (MathNode::Style { body, .. }, CursorStep::Inner) => {
            anchor_for_path(body, layout_box, path, context, mode)
        }
        (MathNode::Matrix { cells, .. }, CursorStep::MatrixCell { row, col }) => cells
            .get(row)
            .and_then(|matrix_row| matrix_row.get(col))
            .and_then(|cell| matrix_slot_anchor(layout_box, cell, path, row, col, context, mode)),
        _ => None,
    }
}

fn sequence_cursor_offset(
    children: &[MathNode],
    layout_box: &LayoutBox,
    pos: usize,
) -> Option<f64> {
    if children.is_empty() {
        return (pos == 0).then_some(EMPTY_LAYOUT_OFFSET);
    }

    let logical_boxes = logical_child_boxes(children, layout_box)?;

    if pos == children.len() {
        return logical_boxes
            .last()
            .map(|child| child.x + child.layout_box.width);
    }

    logical_boxes.get(pos).map(|child| child.x)
}

fn sequence_child_box<'a>(
    children: &[MathNode],
    layout_box: &'a LayoutBox,
    pos: usize,
) -> Option<LayoutChild<'a>> {
    logical_child_boxes(children, layout_box)?.get(pos).copied()
}

fn logical_child_boxes<'a>(
    children: &[MathNode],
    layout_box: &'a LayoutBox,
) -> Option<Vec<LayoutChild<'a>>> {
    if children.is_empty() {
        return Some(Vec::new());
    }

    if children.len() == 1 && !matches!(layout_box.content, BoxContent::HBox(_)) {
        return Some(vec![LayoutChild {
            x: EMPTY_LAYOUT_OFFSET,
            layout_box,
        }]);
    }

    let BoxContent::HBox(layout_children) = &layout_box.content else {
        return None;
    };

    let mut logical_children = Vec::with_capacity(children.len());
    let mut x = EMPTY_LAYOUT_OFFSET;

    for child in layout_children {
        if !is_spacing_box(child) {
            logical_children.push(LayoutChild {
                x,
                layout_box: child,
            });
        }
        x += child.width;
    }

    (logical_children.len() == children.len()).then_some(logical_children)
}

fn slot_cursor_offset(
    node: &MathNode,
    layout_box: &LayoutBox,
    slot: CursorStep,
    path: &[CursorStep],
) -> Option<f64> {
    match (node, slot) {
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => {
            fraction_slot_offset(layout_box, num, path, FractionSlot::Numerator)
        }
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => {
            fraction_slot_offset(layout_box, den, path, FractionSlot::Denominator)
        }
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => {
            radical_slot_offset(layout_box, radicand, path, RadicalSlot::Radicand)
        }
        (
            MathNode::Sqrt {
                index: Some(index), ..
            },
            CursorStep::Index,
        ) => radical_slot_offset(layout_box, index, path, RadicalSlot::Index),
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Base,
        ) => supsub_slot_offset(layout_box, base, path, SupSubSlot::Base),
        (MathNode::Sup { exp, .. } | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => {
            supsub_slot_offset(layout_box, exp, path, SupSubSlot::Sup)
        }
        (
            MathNode::Sub { script, .. } | MathNode::SupSub { sub: script, .. },
            CursorStep::Subscript,
        ) => supsub_slot_offset(layout_box, script, path, SupSubSlot::Sub),
        (MathNode::Parens { body, .. }, CursorStep::Inner) => {
            leftright_slot_offset(layout_box, body, path)
        }
        (MathNode::Style { body, .. }, CursorStep::Inner) => {
            offset_for_path(body, layout_box, path)
        }
        (MathNode::Matrix { cells, .. }, CursorStep::MatrixCell { row, col }) => cells
            .get(row)
            .and_then(|matrix_row| matrix_row.get(col))
            .and_then(|cell| matrix_slot_offset(layout_box, cell, path, row, col)),
        _ => None,
    }
}

fn fraction_slot_offset(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: FractionSlot,
) -> Option<f64> {
    let fraction = locate_fraction_box(layout_box)?;
    let BoxContent::Fraction {
        numer,
        denom,
        numer_scale,
        denom_scale,
        ..
    } = &fraction.layout_box.content
    else {
        return None;
    };

    match slot {
        FractionSlot::Numerator => {
            let x =
                fraction.x + centered_child_x(fraction.layout_box.width, numer.width, *numer_scale);
            Some(x + (offset_for_path(node, numer, path)? * numer_scale))
        }
        FractionSlot::Denominator => {
            let x =
                fraction.x + centered_child_x(fraction.layout_box.width, denom.width, *denom_scale);
            Some(x + (offset_for_path(node, denom, path)? * denom_scale))
        }
    }
}

fn fraction_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: FractionSlot,
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let fraction = locate_fraction_box(layout_box)?;
    let BoxContent::Fraction {
        numer,
        denom,
        numer_shift,
        denom_shift,
        numer_scale,
        denom_scale,
        ..
    } = &fraction.layout_box.content
    else {
        return None;
    };

    let fraction_context = LayoutContext {
        x: context.x + (fraction.x * context.scale),
        baseline_y: context.baseline_y,
        scale: context.scale,
    };

    match slot {
        FractionSlot::Numerator => {
            let child_context = LayoutContext {
                x: fraction_context.x
                    + (centered_child_x(fraction.layout_box.width, numer.width, *numer_scale)
                        * fraction_context.scale),
                baseline_y: fraction_context.baseline_y - (numer_shift * fraction_context.scale),
                scale: fraction_context.scale * numer_scale,
            };
            anchor_for_path(node, numer, path, child_context, mode)
        }
        FractionSlot::Denominator => {
            let child_context = LayoutContext {
                x: fraction_context.x
                    + (centered_child_x(fraction.layout_box.width, denom.width, *denom_scale)
                        * fraction_context.scale),
                baseline_y: fraction_context.baseline_y + (denom_shift * fraction_context.scale),
                scale: fraction_context.scale * denom_scale,
            };
            anchor_for_path(node, denom, path, child_context, mode)
        }
    }
}

fn supsub_slot_offset(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: SupSubSlot,
) -> Option<f64> {
    let BoxContent::SupSub {
        base,
        sup,
        sub,
        sup_scale,
        sub_scale,
        center_scripts,
        italic_correction,
        sub_h_kern,
        ..
    } = &layout_box.content
    else {
        return None;
    };

    let base_x = if *center_scripts {
        (layout_box.width - base.width) / 2.0
    } else {
        EMPTY_LAYOUT_OFFSET
    };

    match slot {
        SupSubSlot::Base => Some(base_x + offset_for_path(node, base, path)?),
        SupSubSlot::Sup => {
            let sup_box = sup.as_ref()?;
            let child_x = if *center_scripts {
                centered_child_x(layout_box.width, sup_box.width, *sup_scale)
            } else {
                base_x + base.width + italic_correction
            };
            Some(child_x + (offset_for_path(node, sup_box, path)? * sup_scale))
        }
        SupSubSlot::Sub => {
            let sub_box = sub.as_ref()?;
            let child_x = if *center_scripts {
                centered_child_x(layout_box.width, sub_box.width, *sub_scale)
            } else {
                base_x + base.width + sub_h_kern
            };
            Some(child_x + (offset_for_path(node, sub_box, path)? * sub_scale))
        }
    }
}

fn supsub_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: SupSubSlot,
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let BoxContent::SupSub {
        base,
        sup,
        sub,
        sup_shift,
        sub_shift,
        sup_scale,
        sub_scale,
        center_scripts,
        italic_correction,
        sub_h_kern,
    } = &layout_box.content
    else {
        return None;
    };

    let base_x = if *center_scripts {
        centered_child_x(layout_box.width, base.width, 1.0)
    } else {
        EMPTY_LAYOUT_OFFSET
    };

    match slot {
        SupSubSlot::Base => {
            let child_context = LayoutContext {
                x: context.x + (base_x * context.scale),
                baseline_y: context.baseline_y,
                scale: context.scale,
            };
            anchor_for_path(node, base, path, child_context, mode)
        }
        SupSubSlot::Sup => {
            let sup_box = sup.as_ref()?;
            let child_x = if *center_scripts {
                centered_child_x(layout_box.width, sup_box.width, *sup_scale)
            } else {
                base_x + base.width + italic_correction
            };
            let child_context = LayoutContext {
                x: context.x + (child_x * context.scale),
                baseline_y: context.baseline_y - (sup_shift * context.scale),
                scale: context.scale * sup_scale,
            };
            anchor_for_path(node, sup_box, path, child_context, mode)
        }
        SupSubSlot::Sub => {
            let sub_box = sub.as_ref()?;
            let child_x = if *center_scripts {
                centered_child_x(layout_box.width, sub_box.width, *sub_scale)
            } else {
                base_x + base.width + sub_h_kern
            };
            let child_context = LayoutContext {
                x: context.x + (child_x * context.scale),
                baseline_y: context.baseline_y + (sub_shift * context.scale),
                scale: context.scale * sub_scale,
            };
            anchor_for_path(node, sub_box, path, child_context, mode)
        }
    }
}

fn radical_slot_offset(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: RadicalSlot,
) -> Option<f64> {
    let BoxContent::Radical {
        body,
        index,
        index_offset,
        index_scale,
        ..
    } = &layout_box.content
    else {
        return None;
    };

    match slot {
        RadicalSlot::Radicand => {
            let radical_width = layout_box.width - index_offset - body.width;
            Some(index_offset + radical_width + offset_for_path(node, body, path)?)
        }
        RadicalSlot::Index => {
            let index_box = index.as_ref()?;
            Some(
                index_offset
                    + RADICAL_INDEX_KERN
                    + (offset_for_path(node, index_box, path)? * index_scale),
            )
        }
    }
}

fn radical_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    slot: RadicalSlot,
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let BoxContent::Radical {
        body,
        index,
        index_offset,
        index_scale,
        ..
    } = &layout_box.content
    else {
        return None;
    };

    match slot {
        RadicalSlot::Radicand => {
            let radical_width = layout_box.width - index_offset - body.width;
            let child_context = LayoutContext {
                x: context.x + ((index_offset + radical_width) * context.scale),
                baseline_y: context.baseline_y,
                scale: context.scale,
            };
            anchor_for_path(node, body, path, child_context, mode)
        }
        RadicalSlot::Index => {
            let index_box = index.as_ref()?;
            let to_shift = 0.6 * (layout_box.height - layout_box.depth);
            let child_context = LayoutContext {
                x: context.x + ((index_offset + RADICAL_INDEX_KERN) * context.scale),
                baseline_y: context.baseline_y - (to_shift * context.scale),
                scale: context.scale * index_scale,
            };
            anchor_for_path(node, index_box, path, child_context, mode)
        }
    }
}

fn leftright_slot_offset(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
) -> Option<f64> {
    let BoxContent::LeftRight { left, inner, .. } = &layout_box.content else {
        return None;
    };

    Some(left.width + offset_for_path(node, inner, path)?)
}

fn leftright_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let BoxContent::LeftRight { left, inner, .. } = &layout_box.content else {
        return None;
    };

    let child_context = LayoutContext {
        x: context.x + (left.width * context.scale),
        baseline_y: context.baseline_y,
        scale: context.scale,
    };

    anchor_for_path(node, inner, path, child_context, mode)
}

fn matrix_slot_offset(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    row: usize,
    col: usize,
) -> Option<f64> {
    let context = LayoutContext {
        x: EMPTY_LAYOUT_OFFSET,
        baseline_y: EMPTY_LAYOUT_OFFSET,
        scale: 1.0,
    };
    let cell_layout = matrix_cell_layout(layout_box, row, col, context)?;
    let local_anchor = anchor_for_path(
        node,
        cell_layout.layout_box,
        path,
        cell_layout.context,
        AnchorMode::HitTest,
    )?;

    Some(local_anchor.x)
}

fn matrix_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    row: usize,
    col: usize,
    context: LayoutContext,
    mode: AnchorMode,
) -> Option<CursorAnchor> {
    let cell_layout = matrix_cell_layout(layout_box, row, col, context)?;
    anchor_for_path(
        node,
        cell_layout.layout_box,
        path,
        cell_layout.context,
        mode,
    )
}

fn matrix_cell_layout(
    layout_box: &LayoutBox,
    row: usize,
    col: usize,
    context: LayoutContext,
) -> Option<MatrixCellLayout<'_>> {
    match &layout_box.content {
        BoxContent::Array {
            cells,
            col_widths,
            col_aligns,
            row_heights,
            row_depths,
            col_gap,
            offset,
            content_x_offset,
            ..
        } => {
            let matrix_row = cells.get(row)?;
            let cell = matrix_row.get(col)?;
            let column_width = *col_widths.get(col)?;
            let row_height = *row_heights.get(row)?;

            let mut local_x = *content_x_offset;
            for width in col_widths.iter().take(col) {
                local_x += width + col_gap;
            }

            let align = col_aligns.get(col).copied().unwrap_or(b'c');
            local_x += match align {
                b'l' => EMPTY_LAYOUT_OFFSET,
                b'r' => column_width - cell.width,
                _ => (column_width - cell.width) / 2.0,
            };

            let mut local_y = -*offset;
            for (height, depth) in row_heights.iter().zip(row_depths).take(row) {
                local_y += height + depth;
            }
            local_y += row_height;

            let cell_context = LayoutContext {
                x: context.x + (local_x * context.scale),
                baseline_y: context.baseline_y + (local_y * context.scale),
                scale: context.scale,
            };

            Some(MatrixCellLayout {
                context: cell_context,
                layout_box: cell,
            })
        }
        BoxContent::LeftRight { left, inner, .. } => {
            let inner_context = LayoutContext {
                x: context.x + (left.width * context.scale),
                baseline_y: context.baseline_y,
                scale: context.scale,
            };
            matrix_cell_layout(inner, row, col, inner_context)
        }
        _ => None,
    }
}

fn locate_fraction_box(layout_box: &LayoutBox) -> Option<LayoutChild<'_>> {
    if matches!(layout_box.content, BoxContent::Fraction { .. }) {
        return Some(LayoutChild {
            x: EMPTY_LAYOUT_OFFSET,
            layout_box,
        });
    }

    let BoxContent::HBox(children) = &layout_box.content else {
        return None;
    };

    let mut x = EMPTY_LAYOUT_OFFSET;
    for child in children {
        if matches!(child.content, BoxContent::Fraction { .. }) {
            return Some(LayoutChild {
                x,
                layout_box: child,
            });
        }
        x += child.width;
    }

    None
}

fn centered_child_x(parent_width: f64, child_width: f64, child_scale: f64) -> f64 {
    (parent_width - (child_width * child_scale)) / 2.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorMode {
    HitTest,
    Display,
}

fn cursor_anchor_with_dims(
    local_x: f64,
    height: f64,
    depth: f64,
    context: LayoutContext,
    mode: AnchorMode,
) -> CursorAnchor {
    let (ascent, descent) = match mode {
        AnchorMode::HitTest => (
            height.clamp(CURSOR_ANCHOR_MIN_ASCENT, CURSOR_ANCHOR_MAX_ASCENT),
            depth.clamp(CURSOR_ANCHOR_MIN_DESCENT, CURSOR_ANCHOR_MAX_DESCENT),
        ),
        AnchorMode::Display => (
            height.max(CURSOR_DISPLAY_MIN_ASCENT),
            depth.max(CURSOR_DISPLAY_MIN_DESCENT),
        ),
    };

    CursorAnchor {
        x: context.x + (local_x * context.scale),
        top: context.baseline_y - (ascent * context.scale),
        bottom: context.baseline_y + (descent * context.scale),
    }
}

fn is_spacing_box(layout_box: &LayoutBox) -> bool {
    matches!(layout_box.content, BoxContent::Kern)
}

fn cursor_visual_offset(root: &MathNode, cursor: &Cursor) -> Option<f32> {
    let mut node = root;
    let mut offset = EMPTY_VISUAL_OFFSET;
    let path = cursor.path();
    let mut i = 0;

    while i < path.len() {
        let CursorStep::SeqPos(pos) = path[i] else {
            return None;
        };
        let children = node.as_seq()?;
        offset += children.iter().take(pos).map(visual_width).sum::<f32>();

        if i == path.len() - 1 {
            return Some(offset);
        }

        let child = children.get(pos)?;
        let slot = path.get(i + CURSOR_PATH_SLOT_OFFSET).copied()?;
        offset += slot_visual_offset(child, slot);
        node = child_for_slot(child, slot)?;
        i += CURSOR_PATH_SLOT_STRIDE;
    }

    None
}

fn visual_width(node: &MathNode) -> f32 {
    match node {
        MathNode::Seq(children) => children.iter().map(visual_width).sum(),
        MathNode::Symbol(data) => usize_to_f32(data.ch.chars().count().max(1)),
        MathNode::Text(text) => usize_to_f32(text.chars().count().max(1)),
        MathNode::Fraction { num, den } => {
            visual_width(num).max(visual_width(den)) + STRUCTURE_EXTRA_WIDTH
        }
        MathNode::Sqrt { index, radicand } => {
            index.as_deref().map_or(EMPTY_VISUAL_OFFSET, visual_width)
                + visual_width(radicand)
                + STRUCTURE_EXTRA_WIDTH
        }
        MathNode::Sup { base, exp } => {
            visual_width(base) + (visual_width(exp) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::Sub { base, script } => {
            visual_width(base) + (visual_width(script) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::SupSub { base, sup, sub } => {
            visual_width(base) + (visual_width(sup).max(visual_width(sub)) * SCRIPT_WIDTH_SCALE)
        }
        MathNode::Parens { body, .. } => visual_width(body) + PARENS_EXTRA_WIDTH,
        MathNode::Style { body, .. } => visual_width(body),
        MathNode::Matrix { cells, .. } => {
            cells
                .iter()
                .map(|row| row.iter().map(visual_width).sum::<f32>())
                .fold(EMPTY_VISUAL_OFFSET, f32::max)
                + STRUCTURE_EXTRA_WIDTH
        }
    }
}

fn slot_visual_offset(node: &MathNode, slot: CursorStep) -> f32 {
    match (node, slot) {
        (MathNode::Fraction { .. }, CursorStep::Numerator | CursorStep::Denominator) => {
            FRACTION_SLOT_OFFSET
        }
        (MathNode::Sqrt { index, .. }, CursorStep::Radicand) => {
            index.as_deref().map_or(EMPTY_VISUAL_OFFSET, visual_width) + STRUCTURE_EXTRA_WIDTH
        }
        (MathNode::Sqrt { .. }, CursorStep::Index) => SQRT_INDEX_OFFSET,
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Exponent | CursorStep::Subscript,
        ) => visual_width(base),
        (MathNode::Parens { .. }, CursorStep::Inner) => INNER_CONTENT_OFFSET,
        (MathNode::Matrix { cells, .. }, CursorStep::MatrixCell { row, col }) => {
            cells.get(row).map_or(EMPTY_VISUAL_OFFSET, |matrix_row| {
                matrix_row.iter().take(col).map(visual_width).sum()
            })
        }
        _ => EMPTY_VISUAL_OFFSET,
    }
}

fn child_for_slot(node: &MathNode, slot: CursorStep) -> Option<&MathNode> {
    match (node, slot) {
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => Some(num),
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => Some(den),
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => Some(radicand),
        (
            MathNode::Sqrt {
                index: Some(index), ..
            },
            CursorStep::Index,
        ) => Some(index),
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Base,
        ) => Some(base),
        (MathNode::Sup { exp, .. } | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => {
            Some(exp)
        }
        (
            MathNode::Sub { script, .. } | MathNode::SupSub { sub: script, .. },
            CursorStep::Subscript,
        ) => Some(script),
        (MathNode::Parens { body, .. } | MathNode::Style { body, .. }, CursorStep::Inner) => {
            Some(body)
        }
        (MathNode::Matrix { cells, .. }, CursorStep::MatrixCell { row, col }) => {
            cells.get(row).and_then(|matrix_row| matrix_row.get(col))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct LayoutChild<'a> {
    x: f64,
    layout_box: &'a LayoutBox,
}

#[derive(Debug, Clone, Copy)]
struct MatrixCellLayout<'a> {
    context: LayoutContext,
    layout_box: &'a LayoutBox,
}

#[derive(Debug, Clone, Copy)]
struct LayoutContext {
    x: f64,
    baseline_y: f64,
    scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct CursorAnchor {
    x: f64,
    top: f64,
    bottom: f64,
}

impl CursorAnchor {
    fn distance_to(self, layout_x: f64, layout_y: f64) -> f64 {
        let dx = (self.x - layout_x).abs();
        let dy = if layout_y < self.top {
            self.top - layout_y
        } else if layout_y > self.bottom {
            layout_y - self.bottom
        } else {
            0.0
        };

        dx + (dy * HIT_TEST_VERTICAL_WEIGHT)
    }
}

#[derive(Debug, Clone)]
struct CursorHit {
    cursor: Cursor,
    distance: f64,
    depth: usize,
}

impl CursorHit {
    fn is_better_than(&self, other: &Self) -> bool {
        self.distance + HIT_TEST_TIE_BREAK_EPSILON < other.distance
            || ((self.distance - other.distance).abs() <= HIT_TEST_TIE_BREAK_EPSILON
                && self.depth > other.depth)
    }
}

#[derive(Debug, Clone, Copy)]
enum FractionSlot {
    Numerator,
    Denominator,
}

#[derive(Debug, Clone, Copy)]
enum SupSubSlot {
    Base,
    Sup,
    Sub,
}

#[derive(Debug, Clone, Copy)]
enum RadicalSlot {
    Index,
    Radicand,
}

#[allow(clippy::cast_precision_loss)]
fn usize_to_f32(value: usize) -> f32 {
    value as f32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn f64_to_f32(value: f64) -> f32 {
    value as f32
}

fn f32_to_f64(value: f32) -> f64 {
    f64::from(value)
}

#[cfg(test)]
mod tests {
    use egui::Rect;

    use super::{
        cursor_for_point, cursor_position_for_rect, cursor_visual_offset, fallback_content_rect,
        layout_cursor_offset, rendered_content_rect, seek_recursive, visual_width,
        EMPTY_LAYOUT_OFFSET, EMPTY_VISUAL_OFFSET,
    };
    use crate::editor::cursor::{Cursor, CursorStep};
    use crate::editor::Editor;
    use crate::ui::renderer::RenderCache;

    #[test]
    fn cursor_offset_moves_across_root_sequence() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');

        let offset = cursor_visual_offset(&editor.root, &editor.cursor).unwrap();

        assert!(offset > EMPTY_VISUAL_OFFSET);
        assert!((offset - visual_width(&editor.root)).abs() < f32::EPSILON);
    }

    #[test]
    fn cursor_offset_accounts_for_parent_nodes() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('+');
        editor.type_char('b');
        editor.type_char('/');
        editor.type_char('c');

        let offset = cursor_visual_offset(&editor.root, &editor.cursor).unwrap();

        assert!(offset > EMPTY_VISUAL_OFFSET);
    }

    #[test]
    fn rendered_content_rect_uses_render_padding() {
        let mut cache = RenderCache::new();
        let rendered = cache.render("abc").unwrap();
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 80.0));

        let content_rect = rendered_content_rect(rendered, rect);

        assert!(content_rect.left() > rect.left());
        assert!(content_rect.right() < rect.right());
    }

    #[test]
    fn cursor_position_uses_fallback_without_rendered_math() {
        let mut editor = Editor::new();
        editor.type_char('x');
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 80.0));

        let position = cursor_position_for_rect(&editor, None, rect);
        let fallback = fallback_content_rect(rect);

        assert!((position.x() - fallback.right()).abs() < f32::EPSILON);
    }

    #[test]
    fn layout_cursor_offset_uses_rendered_sequence_width() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let offset = layout_cursor_offset(&editor.root, &editor.cursor, rendered).unwrap();

        assert!(offset > EMPTY_LAYOUT_OFFSET);
        assert!((offset - rendered.layout_box().width).abs() < f64::EPSILON);
    }

    #[test]
    fn layout_cursor_offset_enters_fraction_denominator() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        editor.type_char('b');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let offset = layout_cursor_offset(&editor.root, &editor.cursor, rendered).unwrap();

        assert!(offset > EMPTY_LAYOUT_OFFSET);
        assert!(offset < rendered.layout_box().width);
    }

    #[test]
    fn layout_cursor_offset_enters_superscript() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('^');
        editor.type_char('2');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let offset = layout_cursor_offset(&editor.root, &editor.cursor, rendered).unwrap();

        assert!(offset > EMPTY_LAYOUT_OFFSET);
        assert!(offset <= rendered.layout_box().width);
    }

    #[test]
    fn layout_cursor_offset_enters_nth_root_index() {
        let mut editor = Editor::new();
        editor.insert_nth_root();
        editor.type_char('3');
        editor.cursor = Cursor::from_path(vec![
            CursorStep::SeqPos(0),
            CursorStep::Radicand,
            CursorStep::SeqPos(0),
        ]);
        editor.type_char('x');
        editor.cursor = Cursor::from_path(vec![
            CursorStep::SeqPos(0),
            CursorStep::Index,
            CursorStep::SeqPos(1),
        ]);
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let offset = layout_cursor_offset(&editor.root, &editor.cursor, rendered).unwrap();

        assert!(offset > EMPTY_LAYOUT_OFFSET);
        assert!(offset < rendered.layout_box().width);
    }

    #[test]
    fn cursor_for_point_uses_layout_positions_at_root() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 80.0));

        let cursor =
            cursor_for_point(&editor, Some(rendered), rect, egui::pos2(200.0, 40.0)).unwrap();

        assert_eq!(cursor.path(), &[CursorStep::SeqPos(3)]);
    }

    #[test]
    fn cursor_for_point_uses_fallback_without_rendered_math() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 80.0));

        let cursor = cursor_for_point(&editor, None, rect, egui::pos2(0.0, 40.0)).unwrap();

        assert_eq!(cursor.path(), &[CursorStep::SeqPos(0)]);
    }

    #[test]
    fn cursor_for_point_uses_fraction_vertical_regions() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        editor.type_char('b');
        let numerator_path = vec![
            CursorStep::SeqPos(0),
            CursorStep::Numerator,
            CursorStep::SeqPos(1),
        ];
        let denominator_path = vec![
            CursorStep::SeqPos(0),
            CursorStep::Denominator,
            CursorStep::SeqPos(1),
        ];
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 80.0));

        editor.cursor = Cursor::from_path(numerator_path.clone());
        let numerator_x = cursor_position_for_rect(&editor, Some(rendered), rect).x();
        let numerator_cursor = cursor_for_point(
            &editor,
            Some(rendered),
            rect,
            egui::pos2(numerator_x, rect.top() + 1.0),
        )
        .unwrap();

        editor.cursor = Cursor::from_path(denominator_path.clone());
        let point_x = cursor_position_for_rect(&editor, Some(rendered), rect).x();
        let denominator_cursor = cursor_for_point(
            &editor,
            Some(rendered),
            rect,
            egui::pos2(point_x, rect.bottom() - 1.0),
        )
        .unwrap();

        assert_eq!(numerator_cursor.path(), numerator_path.as_slice());
        assert_eq!(denominator_cursor.path(), denominator_path.as_slice());
    }

    #[test]
    fn seek_recursive_places_cursor_at_end_for_far_right() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        editor.type_char('c');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let cursor = seek_recursive(&editor.root, rendered, 9999.0, 10.0).unwrap();
        assert_eq!(cursor.path(), &[CursorStep::SeqPos(3)]);
    }

    #[test]
    fn seek_recursive_places_cursor_at_start_for_far_left() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let cursor = seek_recursive(&editor.root, rendered, -1.0, 10.0).unwrap();
        assert_eq!(cursor.path(), &[CursorStep::SeqPos(0)]);
    }

    #[test]
    fn seek_recursive_enters_fraction_numerator() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        editor.type_char('b');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let mid_x = rendered.layout_box().width / 2.0;
        let cursor = seek_recursive(&editor.root, rendered, mid_x, 0.0).unwrap();
        assert!(
            cursor.path().contains(&CursorStep::Numerator),
            "Expected numerator, got {:?}",
            cursor.path()
        );
    }

    #[test]
    fn seek_recursive_enters_fraction_denominator() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('/');
        editor.type_char('b');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let mid_x = rendered.layout_box().width / 2.0;
        let bottom_y = rendered.display_list().height + rendered.layout_box().depth;
        let cursor = seek_recursive(&editor.root, rendered, mid_x, bottom_y).unwrap();
        assert!(
            cursor.path().contains(&CursorStep::Denominator),
            "Expected denominator, got {:?}",
            cursor.path()
        );
    }

    #[test]
    fn seek_recursive_enters_superscript() {
        let mut editor = Editor::new();
        editor.type_char('x');
        editor.type_char('^');
        editor.type_char('2');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let right_x = rendered.layout_box().width * 0.9;
        let cursor = seek_recursive(&editor.root, rendered, right_x, 0.0).unwrap();
        assert!(
            cursor.path().contains(&CursorStep::Exponent),
            "Expected exponent, got {:?}",
            cursor.path()
        );
    }

    #[test]
    fn seek_recursive_enters_symbol_midpoint() {
        let mut editor = Editor::new();
        editor.type_char('a');
        editor.type_char('b');
        let mut cache = RenderCache::new();
        let rendered = cache.render(&editor.to_latex()).unwrap();

        let quarter_x = rendered.layout_box().width * 0.25;
        let cursor = seek_recursive(&editor.root, rendered, quarter_x, 10.0).unwrap();
        let pos = cursor.seq_pos();
        assert!(
            pos <= 1,
            "Click at 25% width should be near start, got pos {pos}"
        );
    }
}
