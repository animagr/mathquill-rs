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

/// Cursor position in egui logical points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CursorPosition {
    x: f32,
}

impl CursorPosition {
    /// Horizontal cursor position in the widget rectangle.
    #[must_use]
    pub(crate) fn x(self) -> f32 {
        self.x
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

    if let Some((rendered, layout_offset)) = rendered.and_then(|rendered| {
        layout_cursor_offset(&editor.root, &editor.cursor, rendered)
            .map(|offset| (rendered, offset))
    }) {
        let width = f64_to_f32(
            rendered
                .display_list()
                .width
                .max(rendered.layout_box().width),
        );
        if width > 0.0 {
            let x =
                content_rect.left() + (f64_to_f32(layout_offset) * content_rect.width() / width);
            return CursorPosition {
                x: x.clamp(content_rect.left(), content_rect.right()),
            };
        }
    }

    fallback_cursor_position(&editor.root, &editor.cursor, content_rect)
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
        let Some(anchor) = layout_cursor_anchor(root, &cursor, rendered) else {
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

    anchor_for_path(root, rendered.layout_box(), cursor.path(), context)
}

fn anchor_for_path(
    node: &MathNode,
    layout_box: &LayoutBox,
    path: &[CursorStep],
    context: LayoutContext,
) -> Option<CursorAnchor> {
    let (seq_pos, remaining) = path.split_first()?;
    let CursorStep::SeqPos(pos) = *seq_pos else {
        return None;
    };

    let children = node.as_seq()?;
    let local_x = sequence_cursor_offset(children, layout_box, pos)?;
    let anchor = cursor_anchor(local_x, layout_box, context);

    if remaining.is_empty() {
        return Some(anchor);
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
    )
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
) -> Option<CursorAnchor> {
    match (node, slot) {
        (MathNode::Fraction { num, .. }, CursorStep::Numerator) => {
            fraction_slot_anchor(layout_box, num, path, FractionSlot::Numerator, context)
        }
        (MathNode::Fraction { den, .. }, CursorStep::Denominator) => {
            fraction_slot_anchor(layout_box, den, path, FractionSlot::Denominator, context)
        }
        (MathNode::Sqrt { radicand, .. }, CursorStep::Radicand) => {
            radical_slot_anchor(layout_box, radicand, path, RadicalSlot::Radicand, context)
        }
        (
            MathNode::Sqrt {
                index: Some(index), ..
            },
            CursorStep::Index,
        ) => radical_slot_anchor(layout_box, index, path, RadicalSlot::Index, context),
        (
            MathNode::Sup { base, .. } | MathNode::Sub { base, .. } | MathNode::SupSub { base, .. },
            CursorStep::Base,
        ) => supsub_slot_anchor(layout_box, base, path, SupSubSlot::Base, context),
        (MathNode::Sup { exp, .. } | MathNode::SupSub { sup: exp, .. }, CursorStep::Exponent) => {
            supsub_slot_anchor(layout_box, exp, path, SupSubSlot::Sup, context)
        }
        (
            MathNode::Sub { script, .. } | MathNode::SupSub { sub: script, .. },
            CursorStep::Subscript,
        ) => supsub_slot_anchor(layout_box, script, path, SupSubSlot::Sub, context),
        (MathNode::Parens { body, .. }, CursorStep::Inner) => {
            leftright_slot_anchor(layout_box, body, path, context)
        }
        (MathNode::Style { body, .. }, CursorStep::Inner) => {
            anchor_for_path(body, layout_box, path, context)
        }
        (MathNode::Matrix { cells, .. }, CursorStep::MatrixCell { row, col }) => cells
            .get(row)
            .and_then(|matrix_row| matrix_row.get(col))
            .and_then(|cell| matrix_slot_anchor(layout_box, cell, path, row, col, context)),
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
            anchor_for_path(node, numer, path, child_context)
        }
        FractionSlot::Denominator => {
            let child_context = LayoutContext {
                x: fraction_context.x
                    + (centered_child_x(fraction.layout_box.width, denom.width, *denom_scale)
                        * fraction_context.scale),
                baseline_y: fraction_context.baseline_y + (denom_shift * fraction_context.scale),
                scale: fraction_context.scale * denom_scale,
            };
            anchor_for_path(node, denom, path, child_context)
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
            anchor_for_path(node, base, path, child_context)
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
            anchor_for_path(node, sup_box, path, child_context)
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
            anchor_for_path(node, sub_box, path, child_context)
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
            anchor_for_path(node, body, path, child_context)
        }
        RadicalSlot::Index => {
            let index_box = index.as_ref()?;
            let to_shift = 0.6 * (layout_box.height - layout_box.depth);
            let child_context = LayoutContext {
                x: context.x + ((index_offset + RADICAL_INDEX_KERN) * context.scale),
                baseline_y: context.baseline_y - (to_shift * context.scale),
                scale: context.scale * index_scale,
            };
            anchor_for_path(node, index_box, path, child_context)
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
) -> Option<CursorAnchor> {
    let BoxContent::LeftRight { left, inner, .. } = &layout_box.content else {
        return None;
    };

    let child_context = LayoutContext {
        x: context.x + (left.width * context.scale),
        baseline_y: context.baseline_y,
        scale: context.scale,
    };

    anchor_for_path(node, inner, path, child_context)
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
    let local_anchor = anchor_for_path(node, cell_layout.layout_box, path, cell_layout.context)?;

    Some(local_anchor.x)
}

fn matrix_slot_anchor(
    layout_box: &LayoutBox,
    node: &MathNode,
    path: &[CursorStep],
    row: usize,
    col: usize,
    context: LayoutContext,
) -> Option<CursorAnchor> {
    let cell_layout = matrix_cell_layout(layout_box, row, col, context)?;
    anchor_for_path(node, cell_layout.layout_box, path, cell_layout.context)
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

fn cursor_anchor(local_x: f64, layout_box: &LayoutBox, context: LayoutContext) -> CursorAnchor {
    let ascent = layout_box
        .height
        .clamp(CURSOR_ANCHOR_MIN_ASCENT, CURSOR_ANCHOR_MAX_ASCENT);
    let descent = layout_box
        .depth
        .clamp(CURSOR_ANCHOR_MIN_DESCENT, CURSOR_ANCHOR_MAX_DESCENT);

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
        layout_cursor_offset, rendered_content_rect, visual_width, EMPTY_LAYOUT_OFFSET,
        EMPTY_VISUAL_OFFSET,
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
}
