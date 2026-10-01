pub mod config;
mod interaction;
mod layout;
mod scroll_bar;
mod state;
mod tool_column;

use crate::cell_ui::{CellSlot, CellUi, RowCells};
use crate::commands::TableCommand;
use crate::table_view::layout::{ColumnKey, MAX_AUTO_COLUMN_WIDTH};
use crate::table_view::state::{EditBuffer, SelectedRange};
pub use config::TableViewConfig;
use egui::emath::GuiRounding;
use egui::epaint::{PathShape, PathStroke};
use egui::scroll_area::ScrollSource;
use egui::{
    Align, Color32, CornerRadius, CursorIcon, Id, Key, Label, Layout, PointerButton, PopupAnchor,
    Pos2, Rangef, Rect, Response, RichText, ScrollArea, Sense, Shape, Stroke, TextWrapMode,
    Tooltip, Ui, UiBuilder, UiKind, UiStackInfo, Vec2, Visuals,
};
use std::collections::{HashMap, HashSet};
use tabular_core::{
    Capabilities, CellCoord, CellLevel, ColumnInfo, ColumnUid, ModelError, RowPosition, RowUid,
    TableModel,
};
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
fn cell_child_ui(
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
    // A selectable label senses clicks and drags on top of the cell, so clicking on text would
    // not select the cell or drag a header (EDIT-10).
    ui.style_mut().interaction.selectable_labels = false;
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

/// View settings chosen by the app. User preferences live in [`TableViewConfig`].
pub struct TableViewOptions {
    /// Salts every id of the view, so that several tables can share a `Ui`.
    pub id_salt: Id,
    /// Height limit; by default the table fills the available height.
    pub max_height: Option<f32>,
    /// Show the row number column with the row and table menus.
    pub tool_column: bool,
    /// Disallow changes to the data, on top of what the model allows. Skipping stays allowed.
    pub read_only: bool,
    /// Choices of the combo box above each column. No combo boxes if empty.
    pub column_mapping_choices: Vec<String>,
}

impl Default for TableViewOptions {
    fn default() -> Self {
        TableViewOptions {
            id_salt: Id::new("egui_tabular"),
            max_height: None,
            tool_column: true,
            read_only: false,
            column_mapping_choices: vec![],
        }
    }
}

/// What happened during [`TableView::show`].
pub struct TableViewOutput {
    pub response: Response,
    pub events: Vec<TableEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TableEvent {
    /// Rows touched by the selection, in view order. Empty when the selection was cleared.
    SelectionChanged {
        rows: Vec<RowUid>,
    },
    /// An edited value was written to the model.
    CellCommitted(CellCoord),
    /// An edit was left without writing the value.
    EditCancelled(CellCoord),
    /// The model refused a change.
    CommandFailed {
        command: Box<TableCommand>,
        error: ModelError,
    },
    ColumnMappingChanged(ColumnUid),
    ColumnsReordered,
    RowsCreated(Vec<RowUid>),
    /// Feedback for the user, e.g. for a toast or a status line.
    Message {
        level: CellLevel,
        text: String,
    },
}

/// Shows a [`TableModel`]. Owns all presentation state: column order and widths, row order and
/// scroll position, selection and the value being edited.
pub struct TableView {
    options: TableViewOptions,
    state: state::State,
}

impl Default for TableView {
    fn default() -> Self {
        Self::new(TableViewOptions::default())
    }
}

impl TableView {
    pub fn new(options: TableViewOptions) -> Self {
        TableView {
            options,
            state: state::State::default(),
        }
    }

    pub fn options(&self) -> &TableViewOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut TableViewOptions {
        &mut self.options
    }

    /// Show `model`, with cells drawn and edited by `cell_ui` (e.g. [`VariantCellUi`](crate::VariantCellUi)).
    ///
    /// Changes the user makes are applied to `model` during this call, but never while the table
    /// is being drawn.
    pub fn show<M: TableModel, C: CellUi<M>>(
        &mut self,
        ui: &mut Ui,
        model: &mut M,
        cell_ui: &mut C,
        config: &mut TableViewConfig,
    ) -> TableViewOutput {
        let id = ui.make_persistent_id(self.options.id_salt);
        let caps = self.capabilities(model);
        let prev_selected_range = self.state.selected_range;
        self.sync_model(model);
        let response = self.show_table(ui, id, caps, model, cell_ui, config);
        self.apply_commands(model);
        self.state.end_stale_edit();

        if self.state.selected_range != prev_selected_range {
            let rows = self
                .state
                .selected_range
                .map(|r| {
                    (r.row_start()..=r.row_end())
                        .filter_map(|idx| self.state.rows.rows().get(idx).copied())
                        .collect()
                })
                .unwrap_or_default();
            self.state
                .events
                .push(TableEvent::SelectionChanged { rows });
        }
        TableViewOutput {
            response,
            events: std::mem::take(&mut self.state.events),
        }
    }

    fn capabilities(&self, model: &impl TableModel) -> Capabilities {
        let caps = model.capabilities();
        if self.options.read_only {
            caps.read_only()
        } else {
            caps
        }
    }

    fn show_table<M: TableModel, C: CellUi<M>>(
        &mut self,
        ui: &mut Ui,
        id: Id,
        caps: Capabilities,
        model: &mut M,
        cell_ui: &mut C,
        config: &mut TableViewConfig,
    ) -> Response {
        let pointer_over_table = ui.rect_contains_pointer(ui.max_rect());
        if pointer_over_table && caps.edit_cells && !self.state.is_editing() {
            let is_no_columns = self.state.columns_ordered.is_empty();
            self.handle_paste(is_no_columns, caps, ui);
            self.apply_commands(model);
        }

        if self.state.columns_ordered.is_empty() {
            if caps.create_columns && ui.button("Create column").clicked() {
                self.state.commands.push(TableCommand::CreateColumn);
            }
            return ui.label("No columns, but can paste tabular data from clipboard");
        }

        if pointer_over_table && !self.state.is_editing() {
            self.handle_key_input(model, cell_ui, caps, ui);
        } else if pointer_over_table && self.state.is_editing() {
            self.handle_key_input_when_editing(ui);
        }
        self.handle_paste_continue(id, ui);
        // Apply what the input queued, so that the table shows the result in this frame.
        self.apply_commands(model);

        let columns = core::mem::take(&mut self.state.columns_ordered);
        let keys: Vec<ColumnKey> = self
            .options
            .tool_column
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
        let mut height = self
            .options
            .max_height
            .map_or(available.height(), |m| m.min(available.height()));
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
                        let body_top = self.show_header(
                            model,
                            cell_ui,
                            config,
                            caps,
                            ui,
                            &grid,
                            top,
                            &columns,
                            &mut swap_columns,
                        );
                        let bottom = self.show_body(
                            model,
                            cell_ui,
                            config,
                            caps,
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
                self.state.events.push(TableEvent::ColumnsReordered);
            }
        });

        if self.state.rows.len() == 0 && caps.create_rows {
            let create_row = ui.button("Add row");
            if create_row.clicked() {
                self.state.commands.push(TableCommand::CreateRows {
                    at: RowPosition::Append,
                    count: 1,
                });
            }
            create_row.on_hover_text(
                "When table is not empty, right click tool column cell to create more rows",
            );
        }
        response
    }

    /// Apply the queued commands, then sync with the changed model.
    fn apply_commands(&mut self, model: &mut impl TableModel) {
        for command in std::mem::take(&mut self.state.commands) {
            match command.apply(model) {
                Ok(applied) => {
                    if let TableCommand::Set { coord, .. } = &command {
                        self.state.events.push(TableEvent::CellCommitted(*coord));
                    }
                    if let Some(row) = applied.created_rows.last()
                        && matches!(command, TableCommand::CreateRows { .. })
                    {
                        self.state.rows.reveal_row(*row);
                    }
                    if !applied.created_rows.is_empty() {
                        self.state
                            .events
                            .push(TableEvent::RowsCreated(applied.created_rows));
                    }
                }
                Err(error) => self.state.events.push(TableEvent::CommandFailed {
                    command: Box::new(command),
                    error,
                }),
            }
        }
        self.sync_model(model);
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

    /// Pick up column and row changes from the model's revision counters. Columns the user moved
    /// keep their place; new columns are appended. Rows are also resynced when the row count
    /// differs, in case the model forgot to bump `revision.rows`.
    fn sync_model(&mut self, model: &impl TableModel) {
        let rev = model.revision();
        let last = self.state.revision;
        if last.is_none_or(|l| l.columns != rev.columns || l.skips != rev.skips) {
            let natural: Vec<ColumnUid> = model.columns().collect();
            let present: HashSet<ColumnUid> = natural.iter().copied().collect();
            let mut ordered: Vec<ColumnUid> = self
                .state
                .columns_ordered
                .iter()
                .copied()
                .filter(|col| present.contains(col))
                .collect();
            let known: HashSet<ColumnUid> = ordered.iter().copied().collect();
            ordered.extend(natural.iter().filter(|col| !known.contains(col)));
            self.state.columns_ordered = ordered;
            self.state.columns = natural
                .iter()
                .filter_map(|col| model.column(*col).map(|info| (*col, info.clone())))
                .collect();
        }
        if last.is_none_or(|l| l.rows != rev.rows) || self.state.rows.len() != model.row_count() {
            self.state.rows.sync(model.rows().collect());
        }
        self.state.revision = Some(rev);
    }

    /// Lays out the header row at `top`. Returns the y coordinate where the body starts.
    #[allow(clippy::too_many_arguments)]
    fn show_header<M: TableModel, C: CellUi<M>>(
        &mut self,
        model: &mut M,
        cell_ui: &mut C,
        config: &mut TableViewConfig,
        caps: Capabilities,
        ui: &mut Ui,
        grid: &Grid,
        top: f32,
        columns: &[ColumnUid],
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
            let mut cell = cell_child_ui(
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
                    if let Some(column) = self.state.columns.get(&column_uid) {
                        let ui = &mut cell;
                        cell_ui.header_ui(model, column_uid, ui);
                        let changed = Self::column_mapping_ui(
                            &self.options.column_mapping_choices,
                            column_uid,
                            &mut config.column_mapped_to,
                            ui,
                            id,
                        );
                        if changed {
                            self.state
                                .events
                                .push(TableEvent::ColumnMappingChanged(column_uid));
                        }
                        let col_name = if column.name.is_empty() {
                            "No name"
                        } else {
                            column.name.as_str()
                        };
                        let col_name = Label::new(RichText::new(col_name).strong().monospace())
                            .selectable(false)
                            .wrap_mode(TextWrapMode::Extend);
                        ui.add(col_name)
                            .on_hover_cursor(CursorIcon::Grab)
                            .on_hover_ui(|ui| {
                                Self::column_name_hover_ui(column, ui);
                            });
                        if let Some(ty) = column.type_text() {
                            ui.add(Label::new(ty.as_ref()).wrap_mode(TextWrapMode::Extend));
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
                    let mut export = false;
                    resp.context_menu(|ui| {
                        export = tool_column::tool_column_header_menu_ui(
                            ui,
                            caps,
                            &mut self.state.commands,
                        );
                    });
                    if export {
                        crate::util::export_csv(model, cell_ui, columns);
                    }
                    resp.on_hover_text("Tool column, right click for actions");
                    continue;
                }
                ColumnKey::Data(column_uid) => column_uid,
            };
            let Some(column) = self.state.columns.get(&column_uid) else {
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
                    ui.label(column.name.as_str());
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

            Self::column_context_menu(column, column_uid, resp, caps, &mut self.state.commands);
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
    #[allow(clippy::too_many_arguments)]
    fn show_body<M: TableModel, C: CellUi<M>>(
        &mut self,
        model: &mut M,
        cell_ui: &mut C,
        config: &TableViewConfig,
        caps: Capabilities,
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
            let row_skipped = model.is_row_skipped(row_uid);
            let row_bg = painter.add(Shape::Noop);

            // Create every cell first: the tool cell, the editor, then the row builder fills the rest.
            let mut tool_cell = None;
            let mut data_cells = Vec::with_capacity(columns.len());
            let mut data_slots = Vec::with_capacity(columns.len());
            for (slot_idx, slot) in slots.iter().enumerate() {
                let cell_bg = painter.add(Shape::Noop);
                let col_idx = slot_idx.wrapping_sub(first_data_slot);
                let is_editing_current_cell = matches!(slot.key, ColumnKey::Data(_))
                    && s.selected_range
                        .map(|r| {
                            r.is_editing() && r == SelectedRange::single_cell(row_idx, col_idx)
                        })
                        .unwrap_or(false);
                let mut cell = cell_child_ui(
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
                let col_uid = match slot.key {
                    ColumnKey::Tool => {
                        cell.add(Label::new(format!("{row_idx}")).selectable(false));
                        tool_cell = Some((slot, cell, cell_bg));
                        continue;
                    }
                    ColumnKey::Data(col_uid) => col_uid,
                };
                let coord = CellCoord { row_uid, col_uid };
                let skipped = row_skipped || s.columns.get(&col_uid).is_some_and(|c| c.is_skipped);
                if skipped && !is_editing_cell_on_this_row {
                    cell.style_mut().visuals.override_text_color = Some(visual.weak_text_color());
                }
                let mut shown = true;
                if is_editing_current_cell {
                    if s.edit.as_ref().map(|e| e.coord) != Some(coord) {
                        s.edit = cell_ui.begin_edit(model, coord).map(|value| EditBuffer {
                            coord,
                            value,
                            first_frame: true,
                        });
                    }
                    match &mut s.edit {
                        Some(edit) => {
                            shown = false;
                            let r = cell_ui.show_editor(model, coord, &mut edit.value, &mut cell);
                            if edit.first_frame {
                                r.response.request_focus();
                                edit.first_frame = false;
                            }
                            if r.commit || cell.input(|i| i.key_pressed(Key::Enter)) {
                                commit_edit = Some(coord);
                            }
                            if cell.input(|i| i.key_pressed(Key::Escape))
                                && let Some(r) = &mut s.selected_range
                            {
                                r.set_editing(None);
                            }
                        }
                        // Not editable: show the value instead.
                        None => {
                            if let Some(r) = &mut s.selected_range {
                                r.set_editing(None);
                            }
                        }
                    }
                }
                data_cells.push(CellSlot {
                    ui: cell,
                    shown,
                    level: None,
                    tooltips: vec![],
                });
                data_slots.push((slot, col_idx, cell_bg, is_editing_current_cell, skipped));
            }
            cell_ui.show_row(
                model,
                row_uid,
                &mut RowCells::new(row_uid, columns, &mut data_cells),
            );

            let row_height = if uniform {
                estimate.span()
            } else {
                let content = tool_cell
                    .iter()
                    .map(|(_, c, _)| c)
                    .chain(data_cells.iter().map(|c| &c.ui))
                    .map(|c| c.min_rect().height())
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

            if let Some((slot, cell, _)) = tool_cell {
                s.column_widths
                    .observe(slot.key, cell.min_rect().width() + 2.0 * pad.x);
                let resp = finish_cell(cell, slot.rect(y_range));
                resp.context_menu(|ui| {
                    tool_column::tool_column_row_menu_ui(ui, model, caps, row_uid, &mut s.commands);
                });
                if resp.clicked() {
                    // Leaving the cell commits the edit (EDIT-1).
                    if let Some(coord) = s.selected_range.and_then(|r| r.editing()) {
                        commit_edit = Some(coord);
                    }
                    if let Some(r) = &mut s.selected_range
                        && ctx.input(|i| i.modifiers.shift)
                    {
                        r.stretch_multi_row(row_idx, columns.len());
                    } else {
                        s.selected_range = Some(SelectedRange::single_row(row_idx, columns.len()));
                    }
                }
            }

            for (cell, (slot, col_idx, cell_bg, is_editing_current_cell, skipped)) in
                data_cells.into_iter().zip(data_slots)
            {
                if !is_editing_current_cell {
                    s.column_widths
                        .observe(slot.key, cell.ui.min_rect().width() + 2.0 * pad.x);
                }
                let cell_rect = slot.rect(y_range);
                let content_rect = cell.ui.max_rect();
                let resp = finish_cell(cell.ui, cell_rect);
                let ColumnKey::Data(col_uid) = slot.key else {
                    continue;
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

                let meta = model.metadata(coord);
                let level = cell.level.or_else(|| meta.as_ref().and_then(|m| m.level));
                let in_selection = is_current_cell_in_selection && !is_editing_cell_on_this_row;
                let color = if let Some(level) = level {
                    Some(level_color(level, visual).gamma_multiply(if in_selection {
                        0.4
                    } else {
                        0.2
                    }))
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

                if let Some(corner) = meta.as_ref().and_then(|m| m.corner) {
                    let r = cell_rect.right_top();
                    painter.add(PathShape {
                        points: vec![Pos2::new(r.x - 10.0, r.y), r, Pos2::new(r.x, r.y + 10.0)],
                        closed: true,
                        fill: level_color(corner, visual),
                        stroke: PathStroke::NONE,
                    });
                }

                if skipped && !is_editing_current_cell {
                    // Cross out the cell.
                    let stroke = Stroke::new(1.0, visual.weak_text_color());
                    let r = content_rect;
                    painter.line_segment([r.min, r.max], stroke);
                    painter.line_segment([r.left_bottom(), r.right_top()], stroke);
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
                            if caps.edit_cells {
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

            y += row_height;
            row_idx += 1;
        }

        s.rows.finish_layout(row_idx, y - top, height);
        if repaint {
            ctx.request_repaint();
        }

        if let Some(coord) = commit_edit {
            s.commit_edit(coord);
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

    fn column_context_menu(
        col: &ColumnInfo,
        col_uid: ColumnUid,
        resp: Response,
        caps: Capabilities,
        commands: &mut Vec<TableCommand>,
    ) {
        resp.context_menu(|ui| {
            if col.is_sortable {
                if ui.button("Sort ascending").clicked() {
                    ui.close_kind(UiKind::Menu);
                }
                if ui.button("Sort descending").clicked() {
                    ui.close_kind(UiKind::Menu);
                }
            }
            if caps.create_columns && ui.button("Add column").clicked() {
                commands.push(TableCommand::CreateColumn);
                ui.close_kind(UiKind::Menu);
            }
            if ui.button("Hide").clicked() {
                ui.close_kind(UiKind::Menu);
            }
            if caps.skip_columns {
                let mut skipped = col.is_skipped;
                if ui.checkbox(&mut skipped, "Skip").changed() {
                    commands.push(TableCommand::SkipColumn {
                        col: col_uid,
                        skipped,
                    });
                    ui.close_kind(UiKind::Menu);
                }
            }
        });
    }

    fn column_name_hover_ui(col: &ColumnInfo, ui: &mut Ui) {
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

/// Color of a cell highlight in the current theme.
fn level_color(level: CellLevel, visuals: &Visuals) -> Color32 {
    match level {
        CellLevel::Info => visuals.hyperlink_color,
        CellLevel::Warning => visuals.warn_fg_color,
        CellLevel::Error => visuals.error_fg_color,
        CellLevel::Changed => visuals.selection.bg_fill,
        CellLevel::Custom(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
    }
}
