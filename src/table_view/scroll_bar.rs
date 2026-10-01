//! Vertical scroll bar for the anchor-based layout.
//!
//! The total pixel height of the table is unknown, so the bar works in rows: the thumb length is
//! the fraction of rows in view and its position is the fractional index of the top row.

use super::layout::RowLayout;
use egui::{CornerRadius, Id, Rect, Sense, Ui, pos2};

/// Width reserved for the bar, including margins.
pub(super) fn width(ui: &Ui) -> f32 {
    let s = &ui.spacing().scroll;
    s.bar_inner_margin + s.bar_width + s.bar_outer_margin
}

/// Handles dragging and clicking, then paints the bar. `rect` is the area reserved with [`width`].
pub(super) fn show(ui: &mut Ui, id: Id, rect: Rect, rows: &mut RowLayout) {
    let len = rows.len() as f32;
    let visible = rows.visible_count().clamp(1.0, len.max(1.0));
    let max_position = (len - visible).max(0.0);
    if max_position <= 0.0 {
        return;
    }

    let s = ui.spacing().scroll;
    let track = Rect::from_x_y_ranges(
        rect.left() + s.bar_inner_margin..=rect.right() - s.bar_outer_margin,
        rect.y_range(),
    );
    let thumb_len = (track.height() * visible / len)
        .max(s.handle_min_length)
        .min(track.height());
    let travel = (track.height() - thumb_len).max(1.0);
    let thumb_rect = |position: f32| {
        let top = track.top() + position.clamp(0.0, max_position) / max_position * travel;
        Rect::from_x_y_ranges(track.x_range(), top..=top + thumb_len)
    };

    let response = ui.interact(rect, id, Sense::click_and_drag());
    // Pointer offset from the thumb top while dragging.
    let grab_id = id.with("grab");
    if let Some(pointer) = response.interact_pointer_pos() {
        if response.drag_started() || response.clicked() {
            let thumb = thumb_rect(rows.position());
            let grab = if thumb.y_range().contains(pointer.y) {
                pointer.y - thumb.top()
            } else {
                thumb_len / 2.0
            };
            ui.data_mut(|d| d.insert_temp(grab_id, grab));
        }
        let grab = ui
            .data(|d| d.get_temp::<f32>(grab_id))
            .unwrap_or(thumb_len / 2.0);
        let position = (pointer.y - grab - track.top()) / travel * max_position;
        rows.scroll_to_position(position.clamp(0.0, max_position));
    }

    let visuals = ui.style().interact(&response);
    let radius = CornerRadius::same((s.bar_width / 2.0) as u8);
    let painter = ui.painter();
    painter.rect_filled(track, radius, ui.visuals().extreme_bg_color);
    let thumb = thumb_rect(rows.position());
    painter.rect_filled(
        Rect::from_min_max(pos2(track.left(), thumb.top()), thumb.max),
        radius,
        visuals.bg_fill,
    );
}
