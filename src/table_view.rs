pub mod config;
mod interaction;
mod state;
mod tool_column;

use crate::frontend::TableFrontend;
use crate::table_view::state::SelectedRange;
pub use config::TableViewConfig;
use egui::{
    CornerRadius, CursorIcon, Id, Key, Label, PointerButton, Response, RichText, ScrollArea, Sense,
    Stroke, TextWrapMode, Ui,
};
use egui_extras::{Column, TableBody};
use std::collections::HashMap;
use tabular_core::backend::{BackendColumn, OneShotFlags, TableBackend, VisualRowIdx};
use tabular_core::{CellCoord, ColumnUid};
use tap::Tap;

pub struct TableView {
    state: state::State,
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
        if ui.rect_contains_pointer(ui.max_rect()) {
            self.handle_paste(is_no_columns, table, ui);
        }

        self.check_col_set_updated(table, &mut is_no_columns);
        self.check_row_set_updated(table, config);

        if is_no_columns {
            table.one_shot_flags_archive();
            *table.one_shot_flags_mut() = OneShotFlags::default();
            if ui.button("Create column").clicked() {
                table.create_column();
            }
            return ui.label("No columns, but can paste tabular data from clipboard");
        }

        if ui.rect_contains_pointer(ui.max_rect()) {
            self.handle_key_input(table, ui);
        }
        self.handle_paste_continue(table, id, ui);

        let ctx = &ui.ctx().clone();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let ui_layer_id = ui.layer_id();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;
        // Temporarily take out columns Vec, to satisfy the borrow checker.
        let columns = core::mem::take(&mut self.state.columns_ordered);
        let mut swap_columns = None;
        let show_tool_column = true;

        ScrollArea::horizontal()
            .drag_to_scroll(false)
            .show(ui, |ui| {
                let mut builder = egui_extras::TableBuilder::new(ui);
                builder = if show_tool_column {
                    builder.column(
                        Column::auto_with_initial_suggestion(48.0)
                            .clip(!config.use_heterogeneous_row_heights),
                    )
                } else {
                    builder
                };
                let mut builder = if let Some(m) = max_height {
                    builder.max_scroll_height(m)
                } else {
                    builder
                };
                for _column in &columns {
                    // Note on clip: At least labels won't try to enlarge cell's area,
                    // effectively rendering heterogeneous row heights logic useless.
                    // So disable clipping if heterogeneous row heights are used.
                    builder =
                        builder.column(Column::auto().clip(!config.use_heterogeneous_row_heights));
                    // builder = builder.column(
                    //     Column::initial(column.name.len() as f32 * 8.0)
                    //         .at_least(36.0)
                    //         .clip(true),
                    // );
                }
                builder
                    // .drag_to_scroll(false) // Drag is used for selection
                    .striped(true)
                    .resizable(true)
                    .sense(Sense::click())
                    .header(20., |mut h| {
                        if show_tool_column {
                            h.col(|_ui| {});
                        }
                        for column_uid in columns.iter().copied() {
                            let Some(backend_column) = self.state.columns.get(&column_uid) else {
                                continue;
                            };
                            let mut painter = None;
                            let (_, resp) = h.col(|ui| {
                                // ui.horizontal_centered(|ui| {
                                table.custom_column_ui(column_uid, ui, id);
                                let changed = Self::column_mapping_ui(
                                    table.column_mapping_choices(),
                                    column_uid,
                                    &mut config.column_mapped_to,
                                    ui,
                                    id,
                                );
                                if changed {
                                    table.one_shot_flags_mut().column_mapping_changed =
                                        Some(column_uid);
                                }
                                let col_name = if backend_column.name.is_empty() {
                                    "No name"
                                } else {
                                    backend_column.name.as_str()
                                };
                                let col_name =
                                    Label::new(RichText::new(col_name).strong().monospace())
                                        .selectable(false)
                                        .wrap_mode(TextWrapMode::Extend);
                                ui.add(col_name)
                                    .on_hover_cursor(CursorIcon::Grab)
                                    .on_hover_ui(|ui| {
                                        Self::column_name_hover_ui(&backend_column, ui);
                                    });
                                // });
                                ui.add(
                                    Label::new(backend_column.ty.as_str())
                                        .wrap_mode(TextWrapMode::Extend),
                                );

                                if painter.is_none() {
                                    painter = Some(ui.painter().clone());
                                }
                            });

                            // Set drag payload for column reordering.
                            resp.dnd_set_drag_payload(column_uid);

                            if resp.dragged() {
                                egui::popup::show_tooltip_text(
                                    ctx,
                                    ui_layer_id,
                                    "_egui_tabular_column_move".into(),
                                    backend_column.name.as_str(),
                                );
                            }

                            let mut rect_fix = resp.rect;
                            rect_fix.set_height(rect_fix.height() * 0.66);
                            // if resp.hovered() && backend_column.is_sortable {
                            //     if let Some(p) = &painter {
                            //         p.rect_filled(
                            //             rect_fix,
                            //             CornerRadius::ZERO,
                            //             visual.selection.bg_fill.gamma_multiply(0.2),
                            //         );
                            //     }
                            // }

                            if resp.dnd_hover_payload::<ColumnUid>().is_some() {
                                if let Some(p) = &painter {
                                    p.rect_filled(
                                        rect_fix,
                                        CornerRadius::ZERO,
                                        visual.selection.bg_fill.gamma_multiply(0.5),
                                    );
                                }
                            }

                            if let Some(payload) = resp.dnd_release_payload::<ColumnUid>() {
                                swap_columns = Some((column_uid, *payload));
                            }

                            Self::column_context_menu(backend_column, column_uid, resp, table);
                        }

                        // Account for header response to calculate total response.
                        resp_total = Some(h.response());
                    })
                    .tap_mut(|table| {
                        table.ui_mut().separator();
                    })
                    .body(|body| {
                        resp_ret = self.show_body(
                            table,
                            config,
                            body,
                            painter,
                            (),
                            ctx,
                            &style,
                            id,
                            &columns,
                            resp_total,
                            show_tool_column,
                        );
                    });
            });

        self.state.columns_ordered = columns.tap_mut(|columns| {
            if let Some((c1, c2)) = swap_columns {
                Self::swap_columns(columns, c1, c2, &mut self.state.selected_range);
            }
        });

        self.check_col_set_updated(table, &mut is_no_columns);

        if table.row_count() == 0 {
            let create_row = ui.button("Add row");
            if create_row.clicked() {
                table.create_row([]);
            }
            create_row.on_hover_text(
                "When table is not empty, right click tool column cell to create more rows",
            );
        }
        self.check_row_set_updated(table, config); // if modified rows during this render cycle

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
            table.one_shot_flags_mut().rows_selected = Some(rows_selected);
        }

        table.one_shot_flags_archive();
        *table.one_shot_flags_mut() = OneShotFlags::default();
        resp_ret.unwrap_or_else(|| ui.label("??"))
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

    fn check_row_set_updated(
        &mut self,
        table: &mut impl TableBackend,
        config: &mut TableViewConfig,
    ) {
        if table.one_shot_flags_internal().row_set_updated {
            self.state
                .row_heights
                .resize(table.row_count(), config.minimum_row_height);
            self.state.row_heights.fill(config.minimum_row_height);
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
                    ui.close_menu();
                }
                if ui.button("Sort descending").clicked() {
                    ui.close_menu();
                }
                if ui.button("Add column").clicked() {
                    data.create_column();
                    ui.close_menu();
                }
            }
            if ui.button("Hide").clicked() {
                ui.close_menu();
            }
            if data.are_cols_skippable() {
                let mut skipped = data.is_col_skipped(col_uid);
                if ui.checkbox(&mut skipped, "Skip").changed() {
                    data.skip_col(col_uid, skipped);
                    ui.close_menu();
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

    fn show_body<T: TableFrontend + TableBackend>(
        &mut self,
        table: &mut T,
        config: &TableViewConfig,
        body: TableBody<'_>,
        _painter: egui::Painter,
        _commands: (),
        ctx: &egui::Context,
        style: &egui::Style,
        id: Id,
        columns: &[ColumnUid],
        mut resp_total: Option<Response>,
        show_tool_column: bool,
    ) -> Option<Response> {
        let visual = &style.visuals;
        let s = &mut self.state;
        let row_heights = &mut s.row_heights;
        let mut row_heights_updates = Vec::new();
        // let pointer_primary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));
        let mut commit_edit = None;
        let row_count = table.row_count();

        let render_fn = |mut row: egui_extras::TableRow| {
            let row_idx = row.index();
            let row_uid = table.row_uid(VisualRowIdx(row_idx)).unwrap();
            let is_editing_cell_on_this_row = s
                .selected_range
                .map(|r| r.is_editing() && r.contains_row(row_idx))
                .unwrap_or(false);
            if is_editing_cell_on_this_row {
                row.set_selected(true);
            }

            if show_tool_column {
                let (_, resp) = row.col(|ui| {
                    ui.add(Label::new(format!("{row_idx}")).selectable(false));
                });
                resp.context_menu(|ui| {
                    tool_column::tool_column_context_menu_ui(ui, table, row_uid);
                });
                if resp.clicked() {
                    if let Some(r) = &mut s.selected_range {
                        if ctx.input(|i| i.modifiers.shift) {
                            r.stretch_multi_row(row_idx, columns.len());
                        } else {
                            *r = SelectedRange::single_row(row_idx, columns.len());
                        }
                    } else {
                        s.selected_range = Some(SelectedRange::single_row(row_idx, columns.len()));
                    }
                }
            }

            let mut next_frame_row_height = config.minimum_row_height;
            for (col_idx, col_uid) in columns.iter().copied().enumerate() {
                let current_cell = SelectedRange::single_cell(row_idx, col_idx);
                let (
                    is_first_row_in_selection,
                    is_last_row_in_selection,
                    is_current_cell_in_selection,
                    is_editing_current_cell,
                ) = s
                    .selected_range
                    .map(|r| {
                        (
                            r.row_start() == row_idx,
                            r.row_end() == row_idx,
                            r.contains(row_idx, col_idx),
                            r.is_editing() && r == current_cell,
                        )
                    })
                    .unwrap_or((false, false, false, false));

                let coord = CellCoord { row_uid, col_uid };
                let (rect, resp) = row.col(|ui| {
                    let ui_max_rect = ui.max_rect();
                    const EXPAND_X: f32 = 2.0;

                    let color = if let Some(backend_color) = table.cell_color(coord) {
                        Some(
                            if is_current_cell_in_selection && !is_editing_cell_on_this_row {
                                backend_color.gamma_multiply(0.4)
                            } else {
                                backend_color.gamma_multiply(0.2)
                            },
                        )
                    } else if is_current_cell_in_selection && !is_editing_cell_on_this_row {
                        // Light orange background inside selection
                        Some(visual.warn_fg_color.gamma_multiply(0.2))
                    } else {
                        None
                    };
                    if let Some(color) = color {
                        ui.painter().rect_filled(
                            ui_max_rect.expand2([EXPAND_X, 0.0].into()),
                            CornerRadius::ZERO,
                            color,
                        );
                    }

                    // Lines on the first and last row of selection
                    let st = Stroke {
                        width: 1.,
                        color: visual.warn_fg_color.gamma_multiply(0.5),
                    };
                    let xr = ui_max_rect.x_range().expand(EXPAND_X);
                    let yr = ui_max_rect.y_range();
                    if is_first_row_in_selection {
                        ui.painter().hline(xr, yr.min, st);
                    }
                    if is_last_row_in_selection {
                        ui.painter().hline(xr, yr.max, st);
                    }
                    // Vertical lines
                    // if is_current_cell_in_selection && !is_editing_cell_on_this_row {
                    //     ui.painter().vline(xr.min, yr, st);
                    //     ui.painter().vline(xr.max, yr, st);
                    // }

                    ui.style_mut()
                        .visuals
                        .widgets
                        .noninteractive
                        .fg_stroke
                        .color = visual.strong_text_color();

                    if is_editing_current_cell {
                        let _resp = table.show_cell_editor(coord, ui, id);
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            commit_edit = Some(coord);
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            if let Some(r) = &mut s.selected_range {
                                r.set_editing(None);
                            }
                        }
                    } else {
                        ui.add_enabled_ui(false, |ui| {
                            table.show_cell_view(coord, ui, id);
                        });
                    }
                });
                next_frame_row_height = rect.height().max(next_frame_row_height);

                if resp.clicked_by(PointerButton::Primary) {
                    if let Some(r) = &mut s.selected_range {
                        if ctx.input(|i| i.modifiers.shift) {
                            r.stretch_to(row_idx, col_idx);
                        } else {
                            if *r == current_cell {
                                r.set_editing(Some(coord));
                            } else {
                                if let Some(coord) = r.editing() {
                                    commit_edit = Some(coord);
                                }
                                s.selected_range = Some(current_cell);
                            }
                        }
                    } else {
                        s.selected_range = Some(current_cell);
                    }
                }
                if resp.double_clicked_by(PointerButton::Primary) {}
                if let Some(tooltip) = table.cell_tooltip(coord) {
                    resp.on_hover_text(tooltip);
                }
            } // for col_uid in used_columns

            if config.use_heterogeneous_row_heights {
                if let Some(prev_row_height) = row_heights.get(row_idx) {
                    if (next_frame_row_height - *prev_row_height).abs() > 0.1 {
                        row_heights_updates.push((row_idx, next_frame_row_height));
                    }
                } else {
                    log::warn!("Row heights wrong len");
                }
            }

            // Accumulate response
            if let Some(resp) = &mut resp_total {
                *resp = resp.union(row.response());
            } else {
                resp_total = Some(row.response());
            }
        };

        if config.use_heterogeneous_row_heights {
            body.heterogeneous_rows(row_heights.iter().copied(), render_fn);
            for (row_idx, next_frame_row_height) in row_heights_updates {
                row_heights[row_idx] = next_frame_row_height;
            }
        } else {
            body.rows(config.minimum_row_height, row_count, render_fn);
        }

        if let Some(coord) = commit_edit {
            table.commit_cell_edit(coord);
            if let Some(r) = &mut s.selected_range {
                r.set_editing(None);
            }
        }

        resp_total
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
