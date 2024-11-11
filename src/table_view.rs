use crate::backend::{CellCoord, TableBackend};
use egui::{Label, PointerButton, Response, Sense, TextBuffer, Ui, Widget};
use egui_extras::{Column, TableBody};
use tap::Tap;

pub struct TableView {}

struct VisColumnPos(u32);

impl TableView {
    pub fn new() -> Self {
        TableView {}
    }

    pub fn show(&mut self, data: &mut impl TableBackend, ui: &mut Ui) -> Response {
        let ctx = &ui.ctx().clone();
        let ui_id = ui.id();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let ui_layer_id = ui.layer_id();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;

        let mut builder = egui_extras::TableBuilder::new(ui);
        for column in data.used_columns() {
            builder = builder.column(Column::auto());
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
                for column in data.used_columns() {
                    let mut painter = None;
                    let (_, resp) = h.col(|ui| {
                        ui.horizontal_centered(|ui| {
                            Label::new(column.name.as_str()).selectable(false).ui(ui);
                        });

                        if painter.is_none() {
                            painter = Some(ui.painter().clone());
                        }
                    });

                    // Set drag payload for column reordering.
                    resp.dnd_set_drag_payload(VisColumnPos(column.col_id));

                    if resp.dragged() {
                        egui::popup::show_tooltip_text(
                            ctx,
                            ui_layer_id,
                            "_EGUI_DATATABLE__COLUMN_MOVE__".into(),
                            column.name.as_str(),
                        );
                    }

                    if resp.hovered() && column.is_sortable {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                resp.rect,
                                egui::Rounding::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.2),
                            );
                        }
                    }

                    if column.is_sortable && resp.clicked_by(PointerButton::Primary) {
                        println!("Sort {}", column.name);
                    }

                    if resp.dnd_hover_payload::<VisColumnPos>().is_some() {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                resp.rect,
                                egui::Rounding::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.5),
                            );
                        }
                    }

                    if let Some(payload) = resp.dnd_release_payload::<VisColumnPos>() {
                        println!("Release: {}", payload.0);
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
                resp_ret = self.show_body(data, body, painter, (), ctx, &style, ui_id, resp_total);
            });

        resp_ret.unwrap_or_else(|| ui.label("??"))
    }

    fn show_body(
        &mut self,
        data: &mut impl TableBackend,
        body: TableBody<'_>,
        mut _painter: egui::Painter,
        commands: (),
        ctx: &egui::Context,
        style: &egui::Style,
        ui_id: egui::Id,
        mut resp_total: Option<Response>,
    ) -> Option<Response> {
        let visual = &style.visuals;

        let render_fn = |mut row: egui_extras::TableRow| {
            for column in data.used_columns() {
                let is_editing = false;
                let cci_selected = false;
                row.set_selected(is_editing || cci_selected);
                let mono_idx = row.index();

                let (rect, resp) = row.col(|ui| {
                    let ui_max_rect = ui.max_rect();

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
                        data.show_cell_view(mono_idx, column.col_id, ui);
                    });
                });
            }

            // Accumulate response
            if let Some(resp) = &mut resp_total {
                *resp = resp.union(row.response());
            } else {
                resp_total = Some(row.response());
            }
        };

        body.rows(32.0, data.visible_row_count(), render_fn);

        resp_total
    }
}
