use std::collections::HashMap;

use crate::TableView;
use egui::{Event, Id, Key, Modal, Ui};
use itertools::Itertools;
use log::warn;
use rvariant::Variant;
use tabular_core::backend::{TableBackend, VisualRowIdx};
use tabular_core::{ColumnUid, RowUid};

impl TableView {
    pub(crate) fn handle_key_input(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(Key::C)) {
            // command+C don't work: https://github.com/emilk/egui/issues/4065
            if let Some(selected) = self.state.selected_range {
                let mut text = String::new();
                for mono_row_idx in selected.row_start()..=selected.row_end() {
                    let Some(row_uid) = data.row_uid(VisualRowIdx(mono_row_idx)) else {
                        continue;
                    };
                    for mono_col_idx in selected.col_start()..=selected.col_end() {
                        let Some(col_uid) = self.state.columns_ordered.get(mono_col_idx) else {
                            continue;
                        };
                        if let Some(v) = data.get((row_uid, *col_uid).into()) {
                            match v {
                                Variant::Str(s) => text += s.as_str(),
                                o => text += o.to_string().as_str(),
                            }
                        }
                        if mono_col_idx != selected.col_end() {
                            text += "\t";
                        }
                    }
                    if mono_row_idx != selected.row_end() {
                        text += "\n";
                    }
                }
                if !text.is_empty() {
                    ui.ctx().copy_text(text);
                }
            }
        }
        self.handle_selection_moves(data.row_count(), ui);
    }

    pub(crate) fn handle_paste(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        let paste = ui.input(|i| {
            i.events
                .iter()
                .find(|e| matches!(e, Event::Paste(_)))
                .cloned()
        });
        let Some(Event::Paste(text)) = paste else {
            return;
        };
        let mut rows = vec![];
        for row in text.split('\n') {
            let mut cols = vec![];
            for col in row.split('\t') {
                cols.push(col.trim().to_string());
            }
            rows.push(cols);
        }
        if rows.is_empty() {
            return;
        }
        self.state.pasting_block_width = rows[0].len();
        let is_equal_lengths =
            rows.iter()
                .map(|c| c.len())
                .tuple_windows()
                .fold(0i32, |acc, (l1, l2)| {
                    self.state.pasting_block_width = l1.max(l2);
                    acc + l1 as i32 - l2 as i32
                })
                == 0;
        self.state.pasting_block_with_holes = !is_equal_lengths;

        if let Some(selected_range) = &self.state.selected_range {
            let selection_is_exact = rows.len() == selected_range.height()
                && self.state.pasting_block_width == selected_range.width()
                && is_equal_lengths;
            self.state.about_to_paste_rows = rows;
            if selection_is_exact {
                self.paste_block(data);
            } else {
                // ask user what to do in handle_paste_continue
                self.state.create_rows_on_paste = false;
                self.state.fill_with_same_on_paste = false;
                self.state.create_cols_on_paste = false;
                // modal.open();
            }
        } else {
            warn!("Refusing to paste without selection"); // TODO: forward to toast
        }
    }

    pub(crate) fn handle_paste_continue(
        &mut self,
        data: &mut impl TableBackend,
        id: Id,
        ui: &mut Ui,
    ) {
        if self.state.about_to_paste_rows.is_empty() {
            return;
        }
        let rows = self.state.about_to_paste_rows.len();
        let cols = self.state.pasting_block_width;
        let Some(selected_range) = self.state.selected_range else {
            return;
        };

        let mut should_close = false;
        let mut should_paste = false;
        let _modal = Modal::new(id.with("egui_tabular_paste_modal")).show(ui.ctx(), |ui| {
            ui.set_width(250.);
            ui.heading("Paste");
            ui.horizontal(|ui| {
                // modal.icon(ui, egui_modal::Icon::Warning);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    let with_holes = if self.state.pasting_block_with_holes {
                        " (with holes)"
                    } else {
                        ""
                    };
                    ui.label(format!(
                        "You are about to paste {}x{} block{} into {}x{} selection",
                        rows,
                        cols,
                        with_holes,
                        selected_range.height(),
                        selected_range.width(),
                    ));
                    if selected_range.height() < rows {
                        ui.checkbox(&mut self.state.create_rows_on_paste, "Create more rows");
                    }
                    if selected_range.height() > rows || selected_range.width() > cols {
                        ui.checkbox(
                            &mut self.state.fill_with_same_on_paste,
                            "Fill with repeated values",
                        );
                    }
                    if selected_range.width() < cols {
                        ui.checkbox(&mut self.state.create_adhoc_cols_on_paste, "Create columns");
                    }
                });
            });
            ui.separator();
            egui::Sides::new().show(
                ui,
                |ui| {
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                },
                |ui| {
                    if ui.button("Paste").clicked() {
                        should_paste = true;
                    }
                },
            );
        });

        if should_paste {
            self.paste_block(data);
        }
        if should_close || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.state.about_to_paste_rows.clear();
        }
    }

    pub(crate) fn paste_block(&mut self, data: &mut impl TableBackend) {
        let Some(selected_range) = &self.state.selected_range else {
            return;
        };
        let mut row_ids: Vec<Option<RowUid>> = (0..selected_range.height())
            .map(|mono_row_idx| {
                data.row_uid(VisualRowIdx(mono_row_idx + selected_range.row_start()))
            })
            .collect();

        if self.state.create_rows_on_paste
            && self.state.about_to_paste_rows.len() > selected_range.height()
        {
            for _ in 0..self.state.about_to_paste_rows.len() - selected_range.height() {
                row_ids.push(data.create_row(HashMap::new()));
            }
        }

        let mut col_ids: Vec<Option<ColumnUid>> = (0..selected_range.width())
            .map(|mono_col_idx| self.state.columns_ordered.get(mono_col_idx).map(|col| *col))
            .collect();

        if self.state.create_adhoc_cols_on_paste {
            for _ in 0..self.state.about_to_paste_rows[0].len() - selected_range.width() {
                col_ids.push(data.create_column());
            }
        }
        let mut changed_coords = vec![];

        if self.state.fill_with_same_on_paste {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter().cycle())
            {
                let Some(row_uid) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter().cycle()) {
                    let Some(col_uid) = col_id_ty else {
                        continue;
                    };
                    let coord = (row_uid, *col_uid).into();
                    changed_coords.push(coord);
                    data.set(coord, Variant::Str(cell.clone()));
                }
            }
        } else {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter())
            {
                let Some(row_id) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter()) {
                    let Some(col_uid) = col_id_ty else {
                        continue;
                    };
                    let coord = (row_id, *col_uid).into();
                    changed_coords.push(coord);
                    data.set(coord, Variant::Str(cell.clone()));
                }
            }
        }

        // data.one_shot_flags_mut().cells_updated = changed_coords;
        self.state.about_to_paste_rows.clear();
    }

    fn handle_selection_moves(&mut self, row_count: usize, ui: &mut Ui) {
        let (left, right, up, down, shift) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.modifiers.shift,
            )
        });
        if left || right || up || down {
            // let was_selected = self.state.selected_range;
            if let Some(already_selected) = &mut self.state.selected_range {
                if left {
                    already_selected.move_left(shift);
                }
                if right {
                    already_selected.move_right(shift, self.state.columns_ordered.len());
                }
                if up {
                    already_selected.move_up(shift);
                }
                if down {
                    already_selected.move_down(shift, row_count);
                }
            }
            // if was_selected != self.state.selected_range {
            //     self.state.save_cell_changes_and_deselect = true;
            //     if let Some(r) = self.state.selected_range {
            //         self.state.last_pressed = Some((r.row_start, r.col_start));
            //     }
            // }
        }
        // let Some(e) = selection_event else {
        //     return;
        // };
        // debug!("{e:?}");
        // match e {
        //     SelectionEvent::Pressed(row, col) => {
        //         if self.state.last_pressed == Some((row, col)) {
        //             debug!("Ignoring");
        //         } else {
        //             if let Some(already_selected) = self.state.selected_range {
        //                 if key_navigation.shift {
        //                     debug!("Ignoring due to shift pressed");
        //                     return;
        //                 }
        //                 if already_selected.is_single_cell() {
        //                     debug!("Deselect and save if any changes");
        //                     self.state.save_cell_changes_and_deselect = true;
        //                 } else {
        //                     debug!("Dropping multi cell selection");
        //                     self.state.selected_range = None;
        //                 }
        //             }
        //             self.state.last_pressed = Some((row, col));
        //         }
        //     }
        //     SelectionEvent::Released(row, col) => {
        //         let Some(prev_pressed) = self.state.last_pressed else {
        //             return;
        //         };
        //         let new_selected_range =
        //             SelectedRange::ordered(prev_pressed.0, prev_pressed.1, row, col);
        //         if self.state.selected_range != Some(new_selected_range) {
        //             debug!("setting new selection range: {:?}", new_selected_range);
        //             self.state.selected_range = Some(new_selected_range);
        //         } else {
        //             debug!("ignoring same selection");
        //         }
        //     }
        // }
    }

    // pub(crate) fn handle_clear_request(&mut self, data: &mut impl TableBackend, modal: &mut Modal) {
    //     if !self.state.clear_requested {
    //         return;
    //     }
    //     modal.show(|ui| {
    //         modal.title(ui, "Clear all data");
    //         ui.horizontal(|ui| {
    //             modal.icon(ui, egui_modal::Icon::Warning);
    //             ui.label("About to clear all table's data, are you sure?");
    //         });
    //         // modal.frame(ui, |ui| {
    //         //     modal.icon(ui, egui_modal::Icon::Warning);
    //         //     modal.body(ui, "About to clear all table's data, are you sure?");
    //         // });
    //         modal.buttons(ui, |ui| {
    //             if modal.caution_button(ui, "Clear").clicked() {
    //                 self.state.clear_requested = false;
    //                 data.clear();
    //             }
    //             if modal.suggested_button(ui, "Cancel").clicked() {
    //                 self.state.clear_requested = false;
    //                 modal.close();
    //             }
    //             if ui.input(|i| i.key_pressed(Key::Escape)) {
    //                 self.state.clear_requested = false;
    //                 modal.close();
    //             }
    //         });
    //     });
    // }
}
