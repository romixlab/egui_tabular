//! Row spans (VIEW-16): cells the model reports as covering several rows of their column
//! ([`TableModel::row_span`]). Read-only, one unit for drawing, hover, selection and keyboard
//! moves, shown while the view shows the natural row order.
//!
//! The body skips the covered cells while it lays out rows and collects one [`SpanCell`] per span
//! in view: at its first row, or at the first row in view when the span starts above it, so no
//! upward scan is needed. The spans are drawn after all rows, when the rows' heights in this frame
//! are known, on top of the row fills.

use super::layout::RowLayout;
use super::state::{SelectedRange, State};
use super::{Grid, Slot, TableEvent, cell_child_ui, finish_cell, level_color};
use crate::cell_ui::{CellSlot, CellUi, RowCells};
use egui::epaint::{PathShape, PathStroke};
use egui::{CornerRadius, Painter, PointerButton, Pos2, Shape, Stroke, Ui};
use tabular_core::{CellCoord, ColumnUid, RowUid, TableModel};

/// Display rows `first..=last` of the span covering the cell at display row `row_idx` of `col`,
/// if it covers more than one row. `natural`: the view shows the natural row order, the only
/// order spans are defined in.
pub(super) fn span_at<M: TableModel>(
    model: &M,
    rows: &RowLayout,
    natural: bool,
    row_idx: usize,
    col: ColumnUid,
) -> Option<(usize, usize)> {
    if !natural {
        return None;
    }
    let row_uid = *rows.rows().get(row_idx)?;
    let span = model.row_span(CellCoord {
        row_uid,
        col_uid: col,
    })?;
    if span.rows < 2 {
        return None;
    }
    let first = rows.index_of(span.first)?;
    let last = (first + span.rows - 1).min(rows.len() - 1);
    (first..=last).contains(&row_idx).then_some((first, last))
}

impl State {
    /// [`span_at`] for the shown column at `col_idx`.
    pub(super) fn span_at<M: TableModel>(
        &self,
        model: &M,
        columns: &[ColumnUid],
        row_idx: usize,
        col_idx: usize,
    ) -> Option<(usize, usize)> {
        let col = *columns.get(col_idx)?;
        span_at(model, &self.rows, self.row_natural.is_none(), row_idx, col)
    }

    /// The selected cell or span at display row `row_idx` of column `col_idx`.
    pub(super) fn unit_at<M: TableModel>(
        &self,
        model: &M,
        columns: &[ColumnUid],
        row_idx: usize,
        col_idx: usize,
    ) -> SelectedRange {
        let mut range = SelectedRange::single_cell(row_idx, col_idx);
        if let Some((first, last)) = self.span_at(model, columns, row_idx, col_idx) {
            range.stretch_to(first, col_idx);
            range.stretch_to(last, col_idx);
        }
        range
    }

    /// Whether the selection is one cell or exactly one span: what the arrow keys move as a unit.
    pub(super) fn selection_is_unit<M: TableModel>(
        &self,
        model: &M,
        columns: &[ColumnUid],
    ) -> bool {
        self.selected_range.is_some_and(|r| {
            r.width() == 1
                && (r.height() == 1
                    || self.span_at(model, columns, r.row_start(), r.col_start())
                        == Some((r.row_start(), r.row_end())))
        })
    }

    /// Grow a cell range selection until no span sticks out of it: a span is selected whole or
    /// not at all. Only the top and bottom rows can cut a span, so this asks about the cells of
    /// those two rows. Whole-row selections stay as they are.
    pub(super) fn grow_selection_to_spans<M: TableModel>(&mut self, model: &M) {
        let Some(mut range) = self.selected_range else {
            return;
        };
        let columns = &self.columns_ordered;
        if self.row_natural.is_some() || range.width() >= columns.len() {
            return;
        }
        loop {
            let before = range;
            for col_idx in range.col_start()..=range.col_end() {
                for row_idx in [range.row_start(), range.row_end()] {
                    if let Some((first, last)) = self.span_at(model, columns, row_idx, col_idx) {
                        range.stretch_to(first, col_idx);
                        range.stretch_to(last, col_idx);
                    }
                }
            }
            if range == before {
                break;
            }
        }
        if range != self.selected_range.unwrap() {
            self.selected_range = Some(range);
        }
    }
}

/// A span in view, collected while the body lays out rows.
pub(super) struct SpanCell {
    pub(super) slot: Slot,
    pub(super) col_idx: usize,
    pub(super) col_uid: ColumnUid,
    /// Display rows of the span.
    pub(super) first_idx: usize,
    pub(super) last_idx: usize,
    pub(super) first_row: RowUid,
    /// The span's first row in view, and where it was laid out.
    pub(super) owner_idx: usize,
    pub(super) owner_y: f32,
    pub(super) skipped: bool,
}

/// What the spans of one frame share with the rest of the body.
pub(super) struct SpanFrame<'a> {
    pub(super) grid: &'a Grid<'a>,
    pub(super) painter: &'a Painter,
    pub(super) hover_pos: Option<Pos2>,
    pub(super) shift: bool,
    /// Spans further than this from the viewport are cut there: they are clipped anyway.
    pub(super) reach: f32,
    /// The spans collected by the body.
    pub(super) spans: Vec<SpanCell>,
}

/// Draw the spans collected by the body, after all rows (see the module docs).
pub(super) fn show_span_cells<M: TableModel, C: CellUi<M>>(
    s: &mut State,
    model: &mut M,
    cell_ui: &mut C,
    ui: &mut Ui,
    frame: SpanFrame,
    outline: &mut Vec<Shape>,
    commit_edit: &mut Option<CellCoord>,
) {
    let SpanFrame {
        grid,
        painter,
        hover_pos,
        shift,
        reach,
        spans,
    } = frame;
    let visual = ui.visuals().clone();
    for span in spans {
        let SpanCell {
            slot,
            col_idx,
            col_uid,
            first_idx,
            last_idx,
            first_row,
            owner_idx,
            owner_y,
            skipped,
        } = span;
        let top = owner_y - s.rows.height_of(first_idx, owner_idx, reach);
        let bottom = owner_y + s.rows.height_of(owner_idx, last_idx + 1, reach);
        let rect = slot.rect(egui::Rangef::new(top, bottom));
        let bg = painter.add(Shape::Noop);

        let mut cell = cell_child_ui(
            ui,
            grid,
            rect,
            slot.auto,
            true,
            grid.id.with(("span", first_row, slot.key)),
            egui::Sense::click(),
        );
        if skipped {
            cell.style_mut().visuals.override_text_color = Some(visual.weak_text_color());
        }
        let mut slots = [CellSlot {
            ui: cell,
            shown: true,
            level: None,
            tooltips: vec![],
        }];
        cell_ui.show_row(
            model,
            first_row,
            &mut RowCells::new(first_row, &[col_uid], &mut slots),
        );
        let [cell] = slots;
        s.column_widths
            .observe(slot.key, cell.ui.min_rect().width() + 2.0 * grid.pad.x);
        let content_rect = cell.ui.max_rect();
        let resp = finish_cell(cell.ui, rect);

        let coord = CellCoord {
            row_uid: first_row,
            col_uid,
        };
        let selected = s.selected_range.filter(|r| {
            r.contains_col(col_idx) && r.row_start() <= last_idx && r.row_end() >= first_idx
        });
        let meta = model.metadata(coord);
        let level = cell.level.or_else(|| meta.as_ref().and_then(|m| m.level));
        // Opaque, so that the stripes and hover fills of the covered rows don't show through.
        let base = if hover_pos.is_some_and(|p| rect.contains(p)) {
            visual.widgets.hovered.bg_fill
        } else {
            visual.panel_fill
        };
        let tint = match level {
            Some(level) => Some(
                level_color(level, &visual).gamma_multiply(if selected.is_some() {
                    0.4
                } else {
                    0.2
                }),
            ),
            None => selected.map(|_| visual.warn_fg_color.gamma_multiply(0.2)),
        };
        painter.set(
            bg,
            Shape::Vec(
                std::iter::once(Shape::rect_filled(rect, CornerRadius::ZERO, base))
                    .chain(tint.map(|c| Shape::rect_filled(rect, CornerRadius::ZERO, c)))
                    .collect(),
            ),
        );
        if let Some(corner) = meta.as_ref().and_then(|m| m.corner) {
            let r = rect.right_top();
            painter.add(PathShape {
                points: vec![Pos2::new(r.x - 10.0, r.y), r, Pos2::new(r.x, r.y + 10.0)],
                closed: true,
                fill: level_color(corner, &visual),
                stroke: PathStroke::NONE,
            });
        }
        if skipped {
            let stroke = Stroke::new(1.0, visual.weak_text_color());
            let r = content_rect.intersect(rect);
            painter.line_segment([r.min, r.max], stroke);
            painter.line_segment([r.left_bottom(), r.right_top()], stroke);
        }
        if let Some(range) = selected {
            let st = Stroke {
                width: 1.,
                color: visual.warn_fg_color.gamma_multiply(0.5),
            };
            let r = rect.shrink(st.width / 2.0);
            if range.row_start() >= first_idx {
                outline.push(Shape::line_segment([r.left_top(), r.right_top()], st));
            }
            if range.row_end() <= last_idx {
                outline.push(Shape::line_segment([r.left_bottom(), r.right_bottom()], st));
            }
            if col_idx == range.col_start() {
                outline.push(Shape::line_segment([r.left_top(), r.left_bottom()], st));
            }
            if col_idx == range.col_end() {
                outline.push(Shape::line_segment([r.right_top(), r.right_bottom()], st));
            }
        }

        if resp.double_clicked() {
            s.events.push(TableEvent::RowDoubleClicked {
                row: first_row,
                col: Some(col_uid),
            });
        }
        if resp.clicked_by(PointerButton::Primary) {
            s.events.push(TableEvent::RowClicked {
                row: first_row,
                col: Some(col_uid),
            });
            if let Some(coord) = s.selected_range.and_then(|r| r.editing()) {
                *commit_edit = Some(coord);
            }
            match &mut s.selected_range {
                Some(r) if shift => {
                    r.stretch_to(first_idx, col_idx);
                    r.stretch_to(last_idx, col_idx);
                }
                _ => {
                    let mut r = SelectedRange::single_cell(first_idx, col_idx);
                    r.stretch_to(last_idx, col_idx);
                    s.selected_range = Some(r);
                }
            }
        }
        let tooltips: Vec<&str> = meta
            .iter()
            .flat_map(|m| m.tooltips.iter().map(|t| t.as_str()))
            .chain(cell.tooltips.iter().map(String::as_str))
            .collect();
        if !tooltips.is_empty() {
            resp.on_hover_ui(|ui| {
                ui.vertical(|ui| {
                    for msg in tooltips {
                        ui.label(msg);
                    }
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabular_core::{Capabilities, ColumnInfo, Revision, RowSpan};

    /// 20 rows, columns 0, 1 and 2; column 1 has spans over rows 2..=5 and 8..=9.
    struct Spans(ColumnInfo);

    impl TableModel for Spans {
        fn revision(&self) -> Revision {
            Revision::default()
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::NONE
        }
        fn columns(&self) -> impl Iterator<Item = ColumnUid> {
            [ColumnUid(0), ColumnUid(1), ColumnUid(2)].into_iter()
        }
        fn column(&self, _col: ColumnUid) -> Option<&ColumnInfo> {
            Some(&self.0)
        }
        fn rows(&self) -> impl Iterator<Item = RowUid> {
            (0..20).map(RowUid)
        }
        fn row_span(&self, coord: CellCoord) -> Option<RowSpan> {
            let row = coord.row_uid.0;
            let (first, rows) = match row {
                _ if coord.col_uid != ColumnUid(1) => return None,
                2..=5 => (2, 4),
                8..=9 => (8, 2),
                _ => (row, 1),
            };
            Some(RowSpan {
                first: RowUid(first),
                rows,
            })
        }
    }

    fn state() -> (State, Spans) {
        let mut s = State::default();
        s.rows.sync((0..20).map(RowUid).collect());
        s.columns_ordered = vec![ColumnUid(0), ColumnUid(1), ColumnUid(2)];
        (s, Spans(ColumnInfo::new("x", None)))
    }

    #[test]
    fn span_at_answers_for_every_covered_row() {
        let (s, m) = state();
        let cols = s.columns_ordered.clone();
        assert_eq!(s.span_at(&m, &cols, 2, 1), Some((2, 5)));
        assert_eq!(s.span_at(&m, &cols, 5, 1), Some((2, 5)));
        assert_eq!(
            s.span_at(&m, &cols, 6, 1),
            None,
            "a span of one row is a plain cell"
        );
        assert_eq!(s.span_at(&m, &cols, 3, 0), None);
        assert_eq!(s.span_at(&m, &cols, 99, 1), None);
    }

    #[test]
    fn spans_are_off_in_a_sorted_or_filtered_order() {
        let (mut s, m) = state();
        s.row_natural = Some(Default::default());
        let cols = s.columns_ordered.clone();
        assert_eq!(s.span_at(&m, &cols, 3, 1), None);
    }

    #[test]
    fn selection_grows_to_whole_spans() {
        let (mut s, m) = state();
        let mut r = SelectedRange::single_cell(1, 0);
        r.stretch_to(3, 1);
        s.selected_range = Some(r);
        s.grow_selection_to_spans(&m);
        let r = s.selected_range.unwrap();
        assert_eq!((r.row_start(), r.row_end()), (1, 5));

        // Two spans chained through the range's edges.
        let mut r = SelectedRange::single_cell(5, 1);
        r.stretch_to(8, 1);
        s.selected_range = Some(r);
        s.grow_selection_to_spans(&m);
        let r = s.selected_range.unwrap();
        assert_eq!((r.row_start(), r.row_end()), (2, 9));

        // Whole rows stay whole rows.
        s.selected_range = Some(SelectedRange::single_row(3, 3));
        s.grow_selection_to_spans(&m);
        let r = s.selected_range.unwrap();
        assert_eq!((r.row_start(), r.row_end()), (3, 3));
    }

    #[test]
    fn a_span_is_a_unit() {
        let (mut s, m) = state();
        let cols = s.columns_ordered.clone();
        let unit = s.unit_at(&m, &cols, 4, 1);
        assert_eq!((unit.row_start(), unit.row_end()), (2, 5));
        s.selected_range = Some(unit);
        assert!(s.selection_is_unit(&m, &cols));
        let mut r = unit;
        r.stretch_to(6, 1);
        s.selected_range = Some(r);
        assert!(!s.selection_is_unit(&m, &cols));
    }
}
