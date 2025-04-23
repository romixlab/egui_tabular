pub mod config;
mod state;

use crate::backend::{CellCoord, ColumnUid, OneShotFlags, TableBackend, VisualRowIdx};
use crate::table_view::state::SelectedRange;
pub use config::TableViewConfig;
use egui::{CornerRadius, Key, Label, PointerButton, Response, ScrollArea, Sense, Stroke, Ui};
use egui_extras::{Column, TableBody};
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

    pub fn show(
        &mut self,
        backend: &mut impl TableBackend,
        config: &mut TableViewConfig,
        ui: &mut Ui,
    ) -> Response {
        if backend.one_shot_flags().column_info_updated {
            println!("Updating col info");
            self.state.columns = backend.used_columns().collect();
            self.state.columns.sort();
        }
        *backend.one_shot_flags_mut() = OneShotFlags::default();
        if self.state.columns.is_empty() {
            return ui.label("No columns");
        }

        let ctx = &ui.ctx().clone();
        let ui_id = ui.id();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let ui_layer_id = ui.layer_id();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;
        // Temporarily take out columns Vec, to satisfy borrow checker.
        let columns = core::mem::take(&mut self.state.columns);
        let mut swap_columns = None;
        // self.frame_n += 1;

        ScrollArea::horizontal()
            .drag_to_scroll(false)
            .show(ui, |ui| {
                let mut builder = egui_extras::TableBuilder::new(ui);
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
                    .drag_to_scroll(false) // Drag is used for selection
                    .striped(true)
                    .resizable(true)
                    .max_scroll_height(f32::MAX)
                    .sense(Sense::click_and_drag())
                    .header(20., |mut h| {
                        for column_uid in columns.iter().copied() {
                            let backend_column = backend.column_info(column_uid).unwrap();
                            let mut painter = None;
                            let (_, resp) = h.col(|ui| {
                                // ui.horizontal_centered(|ui| {
                                let col_name =
                                    Label::new(backend_column.name.as_str()).selectable(false);
                                ui.add(col_name).on_hover_ui(|ui| {
                                    if backend_column.is_required {
                                        ui.label("Required column, synonyms:");
                                        for synonym_name in &backend_column.synonyms {
                                            ui.horizontal(|ui| {
                                                ui.label(synonym_name);
                                                ui.label("or");
                                                ui.label(synonym_name.to_lowercase());
                                            });
                                        }
                                    } else {
                                        if backend_column.is_used {
                                            ui.label("Additional column, used");
                                        } else {
                                            ui.label("Additional column, not used");
                                        }
                                    }
                                });
                                // });

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
                            if resp.hovered() && backend_column.is_sortable {
                                if let Some(p) = &painter {
                                    p.rect_filled(
                                        rect_fix,
                                        CornerRadius::ZERO,
                                        visual.selection.bg_fill.gamma_multiply(0.2),
                                    );
                                }
                            }

                            if backend_column.is_sortable && resp.clicked_by(PointerButton::Primary)
                            {
                                println!("Sort {}", backend_column.name);
                            }

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

                            resp.context_menu(|ui| {
                                if ui.button("Hide").clicked() {
                                    ui.close_menu();
                                }
                            });
                        }

                        // Account for header response to calculate total response.
                        resp_total = Some(h.response());
                    })
                    .tap_mut(|table| {
                        table.ui_mut().separator();
                    })
                    .body(|body| {
                        resp_ret = self.show_body(
                            backend,
                            config,
                            body,
                            painter,
                            (),
                            ctx,
                            &style,
                            ui_id,
                            &columns,
                            resp_total,
                        );
                    });
            });

        self.state.columns = columns.tap_mut(|columns| {
            if let Some((c1, c2)) = swap_columns {
                Self::swap_columns(columns, c1, c2, &mut self.state.selected_range);
            }
        });
        resp_ret.unwrap_or_else(|| ui.label("??"))
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

    fn show_body(
        &mut self,
        backend: &mut impl TableBackend,
        config: &TableViewConfig,
        body: TableBody<'_>,
        mut _painter: egui::Painter,
        _commands: (),
        ctx: &egui::Context,
        style: &egui::Style,
        _ui_id: egui::Id,
        columns: &[ColumnUid],
        mut resp_total: Option<Response>,
    ) -> Option<Response> {
        let visual = &style.visuals;
        let s = &mut self.state;
        let row_heights = core::mem::take(&mut s.row_heights);
        let mut row_heights_updates = Vec::new();
        // let pointer_primary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));
        let mut commit_edit = None;

        let render_fn = |mut row: egui_extras::TableRow| {
            let row_idx = row.index();
            let row_uid = backend.row_uid(VisualRowIdx(row_idx)).unwrap();
            let is_editing_cell_on_this_row = s
                .selected_range
                .map(|r| r.is_editing() && r.contains_row(row_idx))
                .unwrap_or(false);
            if is_editing_cell_on_this_row {
                row.set_selected(true);
            }

            let mut next_frame_row_height = config.minimum_row_height;
            for (col_idx, col_uid) in columns.iter().copied().enumerate() {
                let current_cell = SelectedRange::single(row_idx, col_idx);
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
                let (rect, resp) = row.col(|ui| {
                    let ui_max_rect = ui.max_rect();

                    if is_current_cell_in_selection && !is_editing_cell_on_this_row {
                        // Light orange background inside selection
                        ui.painter().rect_filled(
                            ui_max_rect.expand(0.),
                            CornerRadius::ZERO,
                            visual.warn_fg_color.gamma_multiply(0.2),
                        );
                    }

                    // Lines on first and last row of selection
                    let st = Stroke {
                        width: 1.,
                        color: visual.warn_fg_color.gamma_multiply(0.5),
                    };
                    let xr = ui_max_rect.x_range();
                    let yr = ui_max_rect.y_range();
                    if is_first_row_in_selection {
                        ui.painter().hline(xr, yr.min, st);
                    }
                    if is_last_row_in_selection {
                        ui.painter().hline(xr, yr.max, st);
                    }

                    ui.style_mut()
                        .visuals
                        .widgets
                        .noninteractive
                        .fg_stroke
                        .color = visual.strong_text_color();

                    if is_editing_current_cell {
                        let coord = CellCoord { row_uid, col_uid };
                        let _resp = backend.show_cell_editor(coord, ui);
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            commit_edit = Some(coord)
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            s.selected_range = None;
                        }
                    } else {
                        ui.add_enabled_ui(false, |ui| {
                            backend.show_cell_view(CellCoord { row_uid, col_uid }, ui);
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
                                r.set_editing(true);
                            } else {
                                s.selected_range = Some(current_cell);
                            }
                        }
                    } else {
                        s.selected_range = Some(current_cell);
                    }
                }
                if resp.double_clicked_by(PointerButton::Primary) {}
            } // for col_uid in used_columns

            if config.use_heterogeneous_row_heights {
                if let Some(prev_row_height) = row_heights.get(&row_uid) {
                    if (next_frame_row_height - *prev_row_height).abs() > 0.1 {
                        row_heights_updates.push((row_uid, next_frame_row_height));
                    }
                } else {
                    row_heights_updates.push((row_uid, next_frame_row_height));
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
            body.heterogeneous_rows(
                (0..backend.row_count()).map(|idx| {
                    let row_uid = backend.row_uid(VisualRowIdx(idx)).unwrap();
                    row_heights
                        .get(&row_uid)
                        .copied()
                        .unwrap_or(config.minimum_row_height)
                }),
                render_fn,
            );

            s.row_heights = row_heights.tap_mut(|row_heights| {
                for (row_uid, next_frame_row_height) in row_heights_updates {
                    row_heights.insert(row_uid, next_frame_row_height);
                }
            });
        } else {
            body.rows(config.minimum_row_height, backend.row_count(), render_fn);
        }

        if let Some(coord) = commit_edit {
            backend.commit_cell_edit(coord);
            s.selected_range = None;
        }

        resp_total
    }
}
