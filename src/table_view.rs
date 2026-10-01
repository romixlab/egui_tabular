pub mod config;
mod interaction;
mod layout;
mod scroll_bar;
mod state;
mod tool_column;

use crate::frontend::TableFrontend;
use crate::table_view::layout::{ColumnKey, MAX_AUTO_COLUMN_WIDTH};
use crate::table_view::state::SelectedRange;
pub use config::TableViewConfig;
use egui::emath::GuiRounding;
use egui::epaint::{PathShape, PathStroke};
use egui::scroll_area::ScrollSource;
use egui::{
    Align, CornerRadius, CursorIcon, Id, Key, Label, Layout, PointerButton, PopupAnchor, Pos2,
    Rangef, Rect, Response, RichText, ScrollArea, Sense, Shape, Stroke, TextWrapMode, Tooltip, Ui,
    UiBuilder, UiKind, UiStackInfo, Vec2,
};
use std::collections::HashMap;
use tabular_core::backend::{BackendColumn, OneShotFlags, TableBackend, VisualRowIdx};
use tabular_core::{CellCoord, ColumnUid};
use tap::Tap;

/// Body height used when the parent `Ui` has unbounded height, e.g. inside a vertical `ScrollArea`.
const DEFAULT_MAX_HEIGHT: f32 = 800.0;
const MIN_HEADER_HEIGHT: f32 = 20.0;
const RESIZE_HANDLE_HALF_WIDTH: f32 = 3.0;

/// Horizontal placement of a column in the current frame.
#[derive(Copy, Clone)]
struct Slot {
    key: ColumnKey,
    x: f32,
    /// Full width, including padding.
    width: f32,
    auto: bool,
}

/// Column geometry shared by all cells of the current frame.
#[derive(Copy, Clone)]
struct Grid<'a> {
    slots: &'a [Slot],
    /// Space between a cell's edge and its contents.
    pad: Vec2,
    clip: Rect,
    id: Id,
}

impl Grid<'_> {
    fn x_range(&self) -> Rangef {
        match (self.slots.first(), self.slots.last()) {
            (Some(first), Some(last)) => Rangef::new(first.x, last.x + last.width),
            _ => Rangef::new(self.clip.left(), self.clip.left()),
        }
    }
}

impl Slot {
    fn rect(&self, y: Rangef) -> Rect {
        Rect::from_x_y_ranges(self.x..=self.x + self.width, y)
    }
}

/// Child `Ui` for one cell, laid out top-down inside `rect` minus the grid padding. It is clipped
/// to the column, and also to `rect`'s height if `clip_height` is set. With `measure` set the contents may
/// use up to [`MAX_AUTO_COLUMN_WIDTH`], so that an auto-sized column can grow to fit them.
fn cell_ui(
    parent: &mut Ui,
    grid: &Grid,
    rect: Rect,
    measure: bool,
    clip_height: bool,
    id_salt: Id,
    sense: Sense,
) -> Ui {
    let Grid { pad, clip, .. } = *grid;
    let content_width = if measure {
        MAX_AUTO_COLUMN_WIDTH
    } else {
        (rect.width() - 2.0 * pad.x).max(0.0)
    };
    let max_rect = Rect::from_min_size(
        rect.min + pad,
        Vec2::new(content_width, (rect.height() - 2.0 * pad.y).max(0.0)),
    );
    let mut ui = parent.new_child(
        UiBuilder::new()
            .id_salt(id_salt)
            .ui_stack_info(UiStackInfo::new(UiKind::TableCell))
            .max_rect(max_rect)
            .layout(Layout::top_down(Align::Min))
            .sense(sense),
    );
    let y = if clip_height {
        rect.y_range()
    } else {
        clip.y_range()
    };
    ui.set_clip_rect(clip.intersect(Rect::from_x_y_ranges(rect.x_range(), y)));
    if measure || clip_height {
        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
    }
    ui
}

/// Makes the whole cell `rect` interactive and returns the cell's response.
fn finish_cell(mut ui: Ui, rect: Rect) -> Response {
    ui.expand_to_include_rect(rect);
    ui.response()
}

pub struct TableView {
    state: state::State,
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

impl TableView {
    pub fn new() -> Self {
        TableView {
            state: state::State::default(),
        }
    }

    pub fn show<T: TableFrontend + TableBackend>(
        &mut self,
        table: &mut T,
        config: &mut TableViewConfig,
        max_height: Option<f32>,
        ui: &mut Ui,
        id: Id,
    ) -> Response {
        let mut is_no_columns = self.state.columns_ordered.is_empty();
        let prev_selected_range = self.state.selected_range;
        let pointer_over_table = ui.rect_contains_pointer(ui.max_rect());
        let is_read_only = table.persistent_flags().is_read_only;
        if pointer_over_table && !is_read_only && !self.state.is_editing() {
            self.handle_paste(is_no_columns, table, ui);
        }

        self.check_col_set_updated(table, &mut is_no_columns);
        self.sync_rows(table);

        if is_no_columns {
            table.one_shot_flags_archive();
            *table.one_shot_flags_internal_mut() = OneShotFlags::zero();
            if ui.button("Create column").clicked() {
                table.create_column();
            }
            return ui.label("No columns, but can paste tabular data from clipboard");
        }

        if pointer_over_table && !self.state.is_editing() {
            self.handle_key_input(table, ui);
        } else if pointer_over_table && self.state.is_editing() {
            self.handle_key_input_when_editing(table, ui);
        }
        self.handle_paste_continue(table, id, ui);

        let columns = core::mem::take(&mut self.state.columns_ordered);
        let show_tool_column = true;
        let keys: Vec<ColumnKey> = show_tool_column
            .then_some(ColumnKey::Tool)
            .into_iter()
            .chain(columns.iter().map(|uid| ColumnKey::Data(*uid)))
            .collect();

        let pad = ui.spacing().item_spacing * 0.5;
        self.state.rows.set_metrics(
            config.minimum_row_height + 2.0 * pad.y,
            !config.use_heterogeneous_row_heights,
        );

        // Fill the available height unless capped by max_height.
        let available = ui.available_rect_before_wrap();
        let mut height = max_height.map_or(available.height(), |m| m.min(available.height()));
        if !height.is_finite() {
            height = DEFAULT_MAX_HEIGHT;
        }
        let header_height = self.state.header_height;
        let body_height = (height - header_height)
            .max(3.0 * (config.minimum_row_height + 2.0 * pad.y))
            .round_ui();

        // Auto-sizing changes ease in, user resizing is immediate.
        let ctx = ui.ctx().clone();
        let widths: Vec<(ColumnKey, f32, bool)> = keys
            .iter()
            .map(|key| {
                let w = self.state.column_widths.get(*key);
                let time = self.state.column_widths.animation_time(*key);
                let shown = ctx
                    .animate_value_with_time(id.with(("column_width", *key)), w.width, time)
                    .round_ui();
                (*key, shown, w.auto)
            })
            .collect();
        let columns_width: f32 = widths.iter().map(|(_, w, _)| w).sum();

        self.handle_mouse_wheel(ui, body_height);
        self.state.rows.normalize(body_height);
        let bar_width = if self.state.rows.overflows() {
            scroll_bar::width(ui)
        } else {
            0.0
        };
        // Right after the last column, or at the right edge if the columns are wider.
        let bar_left = (available.left() + columns_width)
            .min(available.right() - bar_width)
            .round_ui();
        let bar_rect = Rect::from_x_y_ranges(
            bar_left..=bar_left + bar_width,
            available.top() + header_height..=available.top() + header_height + body_height,
        );
        if bar_width > 0.0 {
            scroll_bar::show(ui, id.with("v_scroll"), bar_rect, &mut self.state.rows);
            self.state.rows.normalize(body_height);
        }

        let outer_clip = ui.clip_rect();
        let content_rect = Rect::from_min_size(
            available.min,
            Vec2::new(
                if bar_width > 0.0 {
                    bar_left - available.left()
                } else {
                    available.width()
                }
                .max(0.0),
                header_height + body_height,
            ),
        );
        let mut swap_columns = None;
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(content_rect), |ui| {
                ScrollArea::horizontal()
                    .id_salt(id.with("h_scroll"))
                    .scroll_source(ScrollSource::MOUSE_WHEEL)
                    .show(ui, |ui| {
                        let top = ui.cursor().top().round_ui();
                        let left = ui.cursor().left().round_ui();
                        let mut x = left;
                        let slots: Vec<Slot> = widths
                            .iter()
                            .map(|&(key, width, auto)| {
                                let slot = Slot {
                                    key,
                                    x,
                                    width,
                                    auto,
                                };
                                x += width;
                                slot
                            })
                            .collect();
                        // Horizontal clipping comes from the scroll area, vertical from the parent.
                        let clip =
                            Rect::from_x_y_ranges(ui.clip_rect().x_range(), outer_clip.y_range());

                        let grid = Grid {
                            slots: &slots,
                            pad,
                            clip,
                            id,
                        };
                        let body_top =
                            self.show_header(table, config, ui, &grid, top, &mut swap_columns);
                        // "Clear" in the header menu may have removed rows (VIEW-1), and a sync
                        // can move the anchor (stick to bottom), so clamp it again before layout.
                        self.sync_rows(table);
                        self.state.rows.normalize(body_height);
                        let bottom = self.show_body(
                            table,
                            config,
                            ui,
                            &grid,
                            Rangef::new(body_top, body_top + body_height),
                            &columns,
                        );
                        ui.allocate_rect(
                            Rect::from_x_y_ranges(left..=x, top..=bottom),
                            Sense::hover(),
                        );
                        self.column_resize_handles(
                            ui,
                            &grid,
                            Rangef::new(top, body_top),
                            Rangef::new(top, bottom),
                        );
                    });
            })
            .response;
        if self.state.column_widths.end_frame() {
            ctx.request_repaint();
        }
        self.state.table_rect = Some(if bar_width > 0.0 {
            response.rect.union(bar_rect)
        } else {
            response.rect
        });

        self.state.columns_ordered = columns.tap_mut(|columns| {
            if let Some((c1, c2)) = swap_columns {
                Self::swap_columns(columns, c1, c2, &mut self.state.selected_range);
            }
        });

        self.check_col_set_updated(table, &mut is_no_columns);

        if table.row_count() == 0 {
            let create_row = ui.button("Add row");
            if create_row.clicked()
                && let Some(row) = table.create_row([])
            {
                self.state.rows.reveal_row(row);
            }
            create_row.on_hover_text(
                "When table is not empty, right click tool column cell to create more rows",
            );
        }
        self.sync_rows(table); // if modified rows during this render cycle

        if self.state.selected_range != prev_selected_range {
            let rows_selected = if let Some(r) = self.state.selected_range {
                let mut rows_selected = vec![];
                for row_idx in r.row_start()..=r.row_end() {
                    if let Some(row_uid) = table.row_uid(VisualRowIdx(row_idx)) {
                        rows_selected.push(row_uid);
                    }
                }
                rows_selected
            } else {
                vec![]
            };
            table.one_shot_flags_internal_mut().rows_selected = Some(rows_selected);
        }

        table.one_shot_flags_archive();
        *table.one_shot_flags_internal_mut() = OneShotFlags::zero();
        response
    }

    /// Scroll rows with the mouse wheel. The delta is consumed only if the table actually
    /// scrolled, so a parent scroll area takes over at the ends.
    fn handle_mouse_wheel(&mut self, ui: &mut Ui, body_height: f32) {
        let Some(rect) = self.state.table_rect else {
            return;
        };
        if !ui.rect_contains_pointer(rect) {
            return;
        }
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy == 0.0 {
            return;
        }
        let rows = &mut self.state.rows;
        let before = rows.anchor();
        rows.scroll_by(-dy);
        rows.normalize(body_height);
        if rows.anchor() != before {
            ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
        }
    }

    /// Rebuild the display order when the backend reports a row set change, or when the row count
    /// changed without it being reported.
    fn sync_rows(&mut self, table: &impl TableBackend) {
        let count = table.row_count();
        if table.one_shot_flags_internal().row_set_updated || self.state.rows.len() != count {
            let rows = (0..count)
                .filter_map(|i| table.row_uid(VisualRowIdx(i)))
                .collect();
            self.state.rows.sync(rows);
        }
    }

    /// Lays out the header row at `top`. Returns the y coordinate where the body starts.
    fn show_header<T: TableFrontend + TableBackend>(
        &mut self,
        table: &mut T,
        config: &mut TableViewConfig,
        ui: &mut Ui,
        grid: &Grid,
        top: f32,
        swap_columns: &mut Option<(ColumnUid, ColumnUid)>,
    ) -> f32 {
        let Grid {
            slots,
            pad,
            clip,
            id,
        } = *grid;
        let ctx = ui.ctx().clone();
        let ui_layer_id = ui.layer_id();
        let visual = ui.visuals().clone();
        let painter = ui.painter().with_clip_rect(clip);
        let estimate = Rangef::new(top, top + self.state.header_height);

        let mut cells = Vec::with_capacity(slots.len());
        for slot in slots {
            let mut cell = cell_ui(
                ui,
                grid,
                slot.rect(estimate),
                slot.auto,
                false,
                id.with(("header", slot.key)),
                Sense::click_and_drag(),
            );
            match slot.key {
                ColumnKey::Tool => Self::draw_table_icon(&mut cell),
                ColumnKey::Data(column_uid) => {
                    if let Some(backend_column) = self.state.columns.get(&column_uid) {
                        let ui = &mut cell;
                        table.custom_column_ui(column_uid, ui, id);
                        let changed = Self::column_mapping_ui(
                            table.column_mapping_choices(),
                            column_uid,
                            &mut config.column_mapped_to,
                            ui,
                            id,
                        );
                        if changed {
                            table.one_shot_flags_internal_mut().column_mapping_changed =
                                Some(column_uid);
                        }
                        let col_name = if backend_column.name.is_empty() {
                            "No name"
                        } else {
                            backend_column.name.as_str()
                        };
                        let col_name = Label::new(RichText::new(col_name).strong().monospace())
                            .selectable(false)
                            .wrap_mode(TextWrapMode::Extend);
                        ui.add(col_name)
                            .on_hover_cursor(CursorIcon::Grab)
                            .on_hover_ui(|ui| {
                                Self::column_name_hover_ui(backend_column, ui);
                            });
                        if !backend_column.ty.is_empty() {
                            ui.add(
                                Label::new(backend_column.ty.as_str())
                                    .wrap_mode(TextWrapMode::Extend),
                            );
                        }
                    }
                }
            }
            cells.push(cell);
        }

        let content_height = cells
            .iter()
            .map(|c| c.min_rect().height())
            .fold(0.0, f32::max);
        let height = (content_height + 2.0 * pad.y)
            .max(MIN_HEADER_HEIGHT)
            .round_ui();
        if (height - self.state.header_height).abs() > 0.5 {
            self.state.header_height = height;
            ctx.request_repaint();
        }
        let y_range = Rangef::new(top, top + height);

        for (slot, cell) in slots.iter().zip(cells) {
            self.state
                .column_widths
                .observe(slot.key, cell.min_rect().width() + 2.0 * pad.x);
            let resp = finish_cell(cell, slot.rect(y_range));
            let column_uid = match slot.key {
                ColumnKey::Tool => {
                    resp.context_menu(|ui| {
                        if let Some(row) = tool_column::tool_column_header_menu_ui(ui, table) {
                            self.state.rows.reveal_row(row);
                        }
                    });
                    resp.on_hover_text("Tool column, right click for actions");
                    continue;
                }
                ColumnKey::Data(column_uid) => column_uid,
            };
            let Some(backend_column) = self.state.columns.get(&column_uid) else {
                continue;
            };

            // Set drag payload for column reordering.
            resp.dnd_set_drag_payload(column_uid);

            if resp.dragged() {
                Tooltip::always_open(
                    ctx.clone(),
                    ui_layer_id,
                    "_egui_tabular_column_move".into(),
                    PopupAnchor::Pointer,
                )
                .gap(12.0)
                .show(|ui| {
                    ui.label(backend_column.name.as_str());
                });
            }

            if resp.dnd_hover_payload::<ColumnUid>().is_some() {
                let mut rect = resp.rect;
                rect.set_height(rect.height() * 0.66);
                painter.rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    visual.selection.bg_fill.gamma_multiply(0.5),
                );
            }

            if let Some(payload) = resp.dnd_release_payload::<ColumnUid>() {
                *swap_columns = Some((column_uid, *payload));
            }

            Self::column_context_menu(backend_column, column_uid, resp, table);
        }

        painter.hline(
            grid.x_range(),
            top + height,
            visual.widgets.noninteractive.bg_stroke,
        );
        top + height
    }

    /// Lays out rows downwards from the scroll anchor until the body is full. Returns the bottom
    /// of the body: the bottom of the last row if the table fits, otherwise `body.max`.
    fn show_body<T: TableFrontend + TableBackend>(
        &mut self,
        table: &mut T,
        config: &TableViewConfig,
        ui: &mut Ui,
        grid: &Grid,
        body: Rangef,
        columns: &[ColumnUid],
    ) -> f32 {
        let (top, height) = (body.min, body.span());
        let clip = grid
            .clip
            .intersect(Rect::from_x_y_ranges(grid.clip.x_range(), body));
        let grid = &Grid { clip, ..*grid };
        let Grid { slots, pad, id, .. } = *grid;
        let ctx = ui.ctx().clone();
        let style = ui.style().clone();
        let visual = &style.visuals;
        let painter = ui.painter().with_clip_rect(clip);
        let uniform = !config.use_heterogeneous_row_heights;
        let min_row_height = config.minimum_row_height + 2.0 * pad.y;
        let first_data_slot = slots
            .iter()
            .position(|s| matches!(s.key, ColumnKey::Data(_)))
            .unwrap_or(slots.len());
        let x_range = grid.x_range();
        let hover_pos = ctx
            .pointer_hover_pos()
            .filter(|_| ui.rect_contains_pointer(clip));
        let is_read_only = table.persistent_flags().is_read_only;
        let s = &mut self.state;
        let mut commit_edit = None;
        let mut repaint = false;

        let (start, offset) = s.rows.anchor();
        let mut y = (top - offset).round_ui();
        let mut row_idx = start;
        while row_idx < s.rows.len() && y < top + height {
            let row_uid = s.rows.rows()[row_idx];
            let estimate = Rangef::new(y, y + s.rows.height(row_idx));
            let is_editing_cell_on_this_row = s
                .selected_range
                .map(|r| r.is_editing() && r.contains_row(row_idx))
                .unwrap_or(false);
            let row_bg = painter.add(Shape::Noop);

            let mut cells = Vec::with_capacity(slots.len());
            for (slot_idx, slot) in slots.iter().enumerate() {
                let cell_bg = painter.add(Shape::Noop);
                let col_idx = slot_idx.wrapping_sub(first_data_slot);
                let is_editing_current_cell = matches!(slot.key, ColumnKey::Data(_))
                    && s.selected_range
                        .map(|r| {
                            r.is_editing() && r == SelectedRange::single_cell(row_idx, col_idx)
                        })
                        .unwrap_or(false);
                let mut cell = cell_ui(
                    ui,
                    grid,
                    slot.rect(estimate),
                    // An editor sizes itself to the available width, don't let it grow the column.
                    slot.auto && !is_editing_current_cell,
                    uniform,
                    id.with(("cell", row_uid, slot.key)),
                    Sense::click(),
                );
                if is_editing_cell_on_this_row {
                    cell.style_mut().visuals.override_text_color =
                        Some(visual.selection.stroke.color);
                }
                match slot.key {
                    ColumnKey::Tool => {
                        cell.add(Label::new(format!("{row_idx}")).selectable(false));
                    }
                    ColumnKey::Data(col_uid) => {
                        let coord = CellCoord { row_uid, col_uid };
                        cell.style_mut()
                            .visuals
                            .widgets
                            .noninteractive
                            .fg_stroke
                            .color = visual.strong_text_color();
                        if is_editing_current_cell {
                            let _resp = table.show_cell_editor(coord, &mut cell, id);
                            if cell.input(|i| i.key_pressed(Key::Enter)) {
                                commit_edit = Some(coord);
                            }
                            if cell.input(|i| i.key_pressed(Key::Escape))
                                && let Some(r) = &mut s.selected_range
                            {
                                r.set_editing(None);
                            }
                        } else {
                            table.show_cell_view(coord, &mut cell, id);
                        }
                    }
                }
                cells.push((slot_idx, slot, cell, cell_bg, is_editing_current_cell));
            }

            let row_height = if uniform {
                estimate.span()
            } else {
                let content = cells
                    .iter()
                    .map(|(_, _, c, _, _)| c.min_rect().height())
                    .fold(0.0, f32::max);
                (content + 2.0 * pad.y).max(min_row_height).round_ui()
            };
            repaint |= s.rows.set_measured(row_idx, row_height);
            let y_range = Rangef::new(y, y + row_height);
            let row_rect = Rect::from_x_y_ranges(x_range, y_range);

            let row_fill = if is_editing_cell_on_this_row {
                Some(visual.selection.bg_fill)
            } else if hover_pos.is_some_and(|p| row_rect.contains(p)) {
                Some(visual.widgets.hovered.bg_fill)
            } else if row_idx % 2 == 1 {
                Some(visual.faint_bg_color)
            } else {
                None
            };
            if let Some(fill) = row_fill {
                painter.set(
                    row_bg,
                    Shape::rect_filled(row_rect, CornerRadius::ZERO, fill),
                );
            }

            for (slot_idx, slot, cell, cell_bg, is_editing_current_cell) in cells {
                if !is_editing_current_cell {
                    s.column_widths
                        .observe(slot.key, cell.min_rect().width() + 2.0 * pad.x);
                }
                let cell_rect = slot.rect(y_range);
                let resp = finish_cell(cell, cell_rect);
                let col_idx = slot_idx.wrapping_sub(first_data_slot);
                let col_uid = match slot.key {
                    ColumnKey::Tool => {
                        resp.context_menu(|ui| {
                            if let Some(row) =
                                tool_column::tool_column_row_menu_ui(ui, table, row_uid)
                            {
                                s.rows.reveal_row(row);
                            }
                        });
                        if resp.clicked() {
                            if let Some(r) = &mut s.selected_range {
                                if ctx.input(|i| i.modifiers.shift) {
                                    r.stretch_multi_row(row_idx, columns.len());
                                } else {
                                    *r = SelectedRange::single_row(row_idx, columns.len());
                                }
                            } else {
                                s.selected_range =
                                    Some(SelectedRange::single_row(row_idx, columns.len()));
                            }
                        }
                        continue;
                    }
                    ColumnKey::Data(col_uid) => col_uid,
                };

                let coord = CellCoord { row_uid, col_uid };
                let current_cell = SelectedRange::single_cell(row_idx, col_idx);
                let (
                    is_first_row_in_selection,
                    is_last_row_in_selection,
                    is_current_cell_in_selection,
                ) = s
                    .selected_range
                    .map(|r| {
                        (
                            r.row_start() == row_idx,
                            r.row_end() == row_idx,
                            r.contains(row_idx, col_idx),
                        )
                    })
                    .unwrap_or((false, false, false));

                let in_selection = is_current_cell_in_selection && !is_editing_cell_on_this_row;
                let color = if let Some(backend_color) = table.cell_color(coord) {
                    Some(backend_color.gamma_multiply(if in_selection { 0.4 } else { 0.2 }))
                } else if in_selection {
                    // Light orange background inside selection
                    Some(visual.warn_fg_color.gamma_multiply(0.2))
                } else {
                    None
                };
                if let Some(color) = color {
                    painter.set(
                        cell_bg,
                        Shape::rect_filled(cell_rect, CornerRadius::ZERO, color),
                    );
                }

                if let Some(corner) = table.cell_corner(coord) {
                    let r = cell_rect.right_top();
                    painter.add(PathShape {
                        points: vec![Pos2::new(r.x - 10.0, r.y), r, Pos2::new(r.x, r.y + 10.0)],
                        closed: true,
                        fill: corner,
                        stroke: PathStroke::NONE,
                    });
                }

                // Lines on the first and last row of selection
                let st = Stroke {
                    width: 1.,
                    color: visual.warn_fg_color.gamma_multiply(0.5),
                };
                if is_first_row_in_selection && is_current_cell_in_selection {
                    painter.hline(cell_rect.x_range(), cell_rect.top(), st);
                }
                if is_last_row_in_selection && is_current_cell_in_selection {
                    painter.hline(cell_rect.x_range(), cell_rect.bottom(), st);
                }

                if resp.clicked_by(PointerButton::Primary) {
                    if let Some(r) = &mut s.selected_range {
                        if ctx.input(|i| i.modifiers.shift) {
                            r.stretch_to(row_idx, col_idx);
                        } else if *r == current_cell {
                            if !is_read_only {
                                r.set_editing(Some(coord));
                            }
                        } else {
                            if let Some(coord) = r.editing() {
                                commit_edit = Some(coord);
                            }
                            s.selected_range = Some(current_cell);
                        }
                    } else {
                        s.selected_range = Some(current_cell);
                    }
                }
                let tooltips = table.cell_tooltips(coord);
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

            y += row_height;
            row_idx += 1;
        }

        s.rows.finish_layout(row_idx, y - top, height);
        if repaint {
            ctx.request_repaint();
        }

        if let Some(coord) = commit_edit {
            table.commit_cell_edit(coord);
            if let Some(r) = &mut s.selected_range {
                r.set_editing(None);
            }
        }

        if s.rows.overflows() {
            top + height
        } else {
            y.min(top + height)
        }
    }

    /// Drag a column's right edge to resize it, double-click it to return to auto width.
    fn column_resize_handles(&mut self, ui: &mut Ui, grid: &Grid, header: Rangef, full: Rangef) {
        let Grid {
            slots, clip, id, ..
        } = *grid;
        let painter = ui.painter().with_clip_rect(clip);
        let widgets = ui.visuals().widgets.clone();
        for slot in slots {
            let x = slot.x + slot.width;
            let rect = Rect::from_x_y_ranges(
                x - RESIZE_HANDLE_HALF_WIDTH..=x + RESIZE_HANDLE_HALF_WIDTH,
                full,
            );
            let resp = ui.interact(rect, id.with(("resize", slot.key)), Sense::click_and_drag());
            if resp.dragged() {
                self.state
                    .column_widths
                    .resize(slot.key, resp.drag_delta().x);
            }
            if resp.double_clicked() {
                self.state.column_widths.reset(slot.key);
            }
            let (stroke, y) = if resp.dragged() {
                (widgets.active.bg_stroke, full)
            } else if resp.hovered() {
                (widgets.hovered.bg_stroke, full)
            } else {
                (widgets.noninteractive.bg_stroke, header)
            };
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
            }
            painter.vline(x, y, stroke);
        }
    }

    fn draw_table_icon(ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 14.0), Sense::hover());
        let stroke = Stroke::new(1.0_f32, ui.visuals().text_color());
        let p = ui.painter();
        p.rect_stroke(
            rect,
            CornerRadius::same(3),
            stroke,
            egui::StrokeKind::Middle,
        );
        p.line_segment([rect.center_top(), rect.center_bottom()], stroke);
        p.line_segment([rect.left_center(), rect.right_center()], stroke);
    }

    fn check_col_set_updated(&mut self, table: &mut impl TableBackend, is_no_columns: &mut bool) {
        if table.one_shot_flags_internal().columns_reset {
            // log::trace!("Updating col info");
            self.state.columns_ordered = table.used_columns().collect();
            self.state.columns_ordered.sort();
            *is_no_columns = self.state.columns_ordered.is_empty();
        }
        if table.one_shot_flags_internal().columns_reset
            || table.one_shot_flags_internal().columns_changed
        {
            self.state.columns.clear();
            for col_uid in self.state.columns_ordered.iter() {
                if let Some(info) = table.column_info(*col_uid) {
                    self.state.columns.insert(*col_uid, info.clone());
                }
            }
        }
    }

    fn column_context_menu(
        col: &BackendColumn,
        col_uid: ColumnUid,
        resp: Response,
        data: &mut impl TableBackend,
    ) {
        resp.context_menu(|ui| {
            if col.is_sortable {
                if ui.button("Sort ascending").clicked() {
                    ui.close_kind(UiKind::Menu);
                }
                if ui.button("Sort descending").clicked() {
                    ui.close_kind(UiKind::Menu);
                }
                if ui.button("Add column").clicked() {
                    data.create_column();
                    ui.close_kind(UiKind::Menu);
                }
            }
            if ui.button("Hide").clicked() {
                ui.close_kind(UiKind::Menu);
            }
            if data.persistent_flags().are_cols_skippable {
                let mut skipped = data.is_col_skipped(col_uid);
                if ui.checkbox(&mut skipped, "Skip").changed() {
                    data.skip_col(col_uid, skipped);
                    ui.close_kind(UiKind::Menu);
                }
            }
        });
    }

    fn column_name_hover_ui(col: &BackendColumn, ui: &mut Ui) {
        if col.is_required {
            ui.label("Required column, synonyms:");
            for synonym_name in &col.synonyms {
                ui.horizontal(|ui| {
                    ui.label(synonym_name);
                    ui.label("or");
                    ui.label(synonym_name.to_lowercase());
                });
            }
        } else {
            if col.is_used {
                ui.label("Additional column, used");
            } else {
                ui.label("Additional column, not used");
            }
        }
    }

    fn swap_columns(
        columns: &mut Vec<ColumnUid>,
        c1: ColumnUid,
        c2: ColumnUid,
        selected_range: &mut Option<SelectedRange>,
    ) {
        let c1_idx = columns
            .iter()
            .enumerate()
            .find(|(_, uid)| **uid == c1)
            .map(|(idx, _)| idx);
        let c2_idx = columns
            .iter()
            .enumerate()
            .find(|(_, uid)| **uid == c2)
            .map(|(idx, _)| idx);
        if let (Some(c1_idx), Some(c2_idx)) = (c1_idx, c2_idx) {
            columns.swap(c1_idx, c2_idx);
            if let Some(r) = selected_range {
                if r.is_single_cell() {
                    // Keep selection if only one cell was selected, but adjust accordingly
                    r.swap_col(c1_idx, c2_idx);
                } else if !(r.contains_col(c1_idx) && r.contains_col(c2_idx)) {
                    // Deselect if column was dragged outside current selection
                    *selected_range = None;
                }
            }
        }
    }

    fn column_mapping_ui(
        choices: &[String],
        col_uid: ColumnUid,
        column_mapped_to: &mut HashMap<ColumnUid, String>,
        ui: &mut Ui,
        id: Id,
    ) -> bool {
        if choices.is_empty() {
            return false;
        }
        let is_used_elsewhere = if let Some(selected) = column_mapped_to.get(&col_uid) {
            if selected.is_empty() {
                false
            } else {
                column_mapped_to
                    .iter()
                    .any(|(col, value)| *col != col_uid && value == selected)
            }
        } else {
            false
        };
        let selected = column_mapped_to.entry(col_uid).or_default();
        let selected_text = if selected.is_empty() {
            RichText::new("Skip")
        } else {
            if is_used_elsewhere {
                RichText::new(selected.as_str()).color(ui.visuals().warn_fg_color)
            } else {
                RichText::new(selected.as_str())
            }
        };
        let mut changed = false;
        let resp = egui::ComboBox::from_id_salt(id.with(col_uid.0))
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(selected, String::new(), "Skip")
                    .changed();
                for m in choices {
                    changed |= ui
                        .selectable_value(selected, m.clone(), m.as_str())
                        .changed();
                }
            })
            .response;
        if is_used_elsewhere {
            resp.on_hover_text("Cannot map more than one column to the same entity");
        }
        changed
    }
}
