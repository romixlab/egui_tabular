mod config;
mod state;

use crate::backend::{CellCoord, ColumnUid, OneShotFlags, TableBackend, VisualRowIdx};
use egui::{Label, PointerButton, Response, ScrollArea, Sense, Ui, Widget};
use egui_extras::{Column, TableBody};
use tap::Tap;

pub struct TableView {
    state: state::State,
    config: config::TableViewConfig,
    // frame_n: usize,
}

impl TableView {
    pub fn new() -> Self {
        TableView {
            state: state::State::default(),
            config: config::TableViewConfig::default(),
            // frame_n: 0
        }
    }

    pub fn show(&mut self, backend: &mut impl TableBackend, ui: &mut Ui) -> Response {
        if backend.one_shot_flags().column_info_updated {
            println!("Updating col info");
            self.state.columns = backend.used_columns().collect();
            self.state.columns.sort();
        }
        if self.state.columns.is_empty() {
            return ui.label("No columns");
        }
        *backend.one_shot_flags_mut() = OneShotFlags::default();

        let ctx = &ui.ctx().clone();
        let ui_id = ui.id();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let ui_layer_id = ui.layer_id();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;
        /// Temporarily take out columns Vec, to satisfy borrow checker.
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
                    builder = builder
                        .column(Column::auto().clip(!self.config.use_heterogeneous_row_heights));
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
                    .sense(Sense::click_and_drag().tap_mut(|s| s.focusable = true))
                    .header(20., |mut h| {
                        for column_uid in columns.iter().copied() {
                            let backend_column = backend.column_info(column_uid).unwrap();
                            let mut painter = None;
                            let (_, resp) = h.col(|ui| {
                                // ui.horizontal_centered(|ui| {
                                Label::new(backend_column.name.as_str())
                                    .selectable(false)
                                    .ui(ui);
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
                                    "_EGUI_TABULAR__COLUMN_MOVE__".into(),
                                    backend_column.name.as_str(),
                                );
                            }

                            if resp.hovered() && backend_column.is_sortable {
                                if let Some(p) = &painter {
                                    p.rect_filled(
                                        resp.rect,
                                        egui::Rounding::ZERO,
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
                                        resp.rect,
                                        egui::Rounding::ZERO,
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
                Self::swap_columns(columns, c1, c2);
            }
        });
        resp_ret.unwrap_or_else(|| ui.label("??"))
    }

    fn swap_columns(columns: &mut Vec<ColumnUid>, c1: ColumnUid, c2: ColumnUid) {
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
        }
    }

    fn show_body(
        &mut self,
        backend: &mut impl TableBackend,
        body: TableBody<'_>,
        mut _painter: egui::Painter,
        _commands: (),
        _ctx: &egui::Context,
        style: &egui::Style,
        _ui_id: egui::Id,
        columns: &[ColumnUid],
        mut resp_total: Option<Response>,
    ) -> Option<Response> {
        let visual = &style.visuals;
        let row_heights = core::mem::take(&mut self.state.row_heights);
        let mut row_heights_updates = Vec::new();

        let render_fn = |mut row: egui_extras::TableRow| {
            let visual_row_idx = VisualRowIdx(row.index());
            let row_uid = backend.row_uid(visual_row_idx).unwrap();

            let mut next_frame_row_height = self.config.minimum_row_height;
            for col_uid in columns.iter().copied() {
                let is_editing = false;
                let cci_selected = false;
                row.set_selected(is_editing || cci_selected);

                let (rect, _resp) = row.col(|ui| {
                    // let ui_max_rect = ui.max_rect();

                    // if is_interactive_cell {
                    //     ui.painter().rect_filled(
                    //         ui_max_rect.expand(3.),
                    //         no_rounding,
                    //         visual.warn_fg_color.gamma_multiply(0.2),
                    //     );
                    // } else if !cci_selected && selected {
                    //     ui.painter()
                    //         .rect_filled(ui_max_rect, no_rounding, visual.extreme_bg_color);
                    // }

                    ui.style_mut()
                        .visuals
                        .widgets
                        .noninteractive
                        .fg_stroke
                        .color = visual.strong_text_color();

                    ui.add_enabled_ui(false, |ui| {
                        backend.show_cell_view(CellCoord { row_uid, col_uid }, ui);
                    });
                });
                // if row_uid.0 == 2 {
                //     println!("{}", rect.height());
                // }
                next_frame_row_height = rect.height().max(next_frame_row_height);
            } // for col_uid in used_columns

            if self.config.use_heterogeneous_row_heights {
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

        if self.config.use_heterogeneous_row_heights {
            body.heterogeneous_rows(
                (0..backend.row_count()).map(|idx| {
                    let row_uid = backend.row_uid(VisualRowIdx(idx)).unwrap();
                    row_heights
                        .get(&row_uid)
                        .copied()
                        .unwrap_or(self.config.minimum_row_height)
                }),
                render_fn,
            );

            self.state.row_heights = row_heights.tap_mut(|row_heights| {
                for (row_uid, next_frame_row_height) in row_heights_updates {
                    row_heights.insert(row_uid, next_frame_row_height);
                }
            });
        } else {
            body.rows(
                self.config.minimum_row_height,
                backend.row_count(),
                render_fn,
            );
        }

        resp_total
    }
}
