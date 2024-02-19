use std::collections::HashMap;

use egui::{Event, Key, Ui};
use egui_modal::Modal;
use itertools::Itertools;
use log::{debug, warn};
use rvariant::{Variant, VariantTy};

use crate::{backend::TableBackend, cell::CellCoord, table_view::table::SelectedRange};

use super::{table::SelectionEvent, SelectionKeyNavigation, TableView};

impl TableView {
    pub(crate) fn handle_key_input(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            if self.state.cell_when_editing.is_some() {
                self.state.save_cell_changes_and_deselect = true;
            } else {
                self.state.selected_range = None;
                self.state.last_pressed = None;
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            if self.state.cell_when_editing.is_some() {
                self.state.discard_cell_changes_and_deselect = true;
            } else {
                self.state.selected_range = None;
                self.state.last_pressed = None;
            }
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(Key::C)) {
            // command+C don't work anymore for some reason
            if let Some(selected) = self.state.selected_range {
                let mut text = String::new();
                for mono_row_idx in selected.row_start..=selected.row_end {
                    let Some(row_uid) = data.row_uid(mono_row_idx) else {
                        continue;
                    };
                    for mono_col_idx in selected.col_start..=selected.col_end {
                        let Some(col_uid) = self
                            .state
                            .columns
                            .get(mono_col_idx as usize)
                            .map(|u| u.col_uid)
                        else {
                            continue;
                        };
                        match data.cell(CellCoord(row_uid, col_uid)) {
                            crate::cell::TableCellRef::Available { value, .. } => {
                                if let Variant::Str(s) = value {
                                    text += s.as_str();
                                } else {
                                    text += value.to_string().as_str();
                                }
                            }
                            _ => {}
                        }
                        if mono_col_idx != selected.col_end {
                            text += "\t";
                        }
                    }
                    if mono_row_idx != selected.row_end {
                        text += "\n";
                    }
                }
                if !text.is_empty() {
                    ui.output_mut(|i| i.copied_text = text);
                }
            }
        }
    }

    pub(crate) fn handle_paste(
        &mut self,
        ui: &mut Ui,
        data: &mut impl TableBackend,
        modal: &mut Modal,
    ) {
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
        self.state.pasting_block_width = rows[0].len() as u32;
        let is_equal_lengths =
            rows.iter()
                .map(|c| c.len())
                .tuple_windows()
                .fold(0i32, |acc, (l1, l2)| {
                    self.state.pasting_block_width = l1.max(l2) as u32;
                    acc + l1 as i32 - l2 as i32
                })
                == 0;
        self.state.pasting_block_with_holes = !is_equal_lengths;

        if let Some(selected_range) = &self.state.selected_range {
            let selection_is_exact = rows.len() == selected_range.height() as usize
                && self.state.pasting_block_width == selected_range.width()
                && is_equal_lengths;
            self.state.about_to_paste_rows = rows;
            if selection_is_exact {
                self.paste_block(data);
            } else {
                // ask user what to do in handle_paste_continue
                self.state.create_rows_on_paste = false;
                self.state.fill_with_same_on_paste = false;
                self.state.create_adhoc_cols_on_paste = false;
                modal.open();
            }
        } else {
            warn!("Refusing to paste without selection");
        }
    }

    pub(crate) fn handle_paste_continue(
        &mut self,
        data: &mut impl TableBackend,
        modal: &mut Modal,
    ) {
        if self.state.about_to_paste_rows.is_empty() {
            return;
        }
        let rows = self.state.about_to_paste_rows.len() as u32;
        let cols = self.state.pasting_block_width as u32;
        let Some(selected_range) = self.state.selected_range else {
            return;
        };

        modal.show(|ui| {
            modal.title(ui, "Paste");
            ui.horizontal(|ui| {
                modal.icon(ui, egui_modal::Icon::Warning);
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
                        ui.checkbox(
                            &mut self.state.create_adhoc_cols_on_paste,
                            "Create adhoc columns (not implemented yet)",
                        );
                    }
                });
            });
            modal.buttons(ui, |ui| {
                if modal.button(ui, "Close").clicked() {
                    self.state.about_to_paste_rows.clear();
                }
                if modal.suggested_button(ui, "Paste").clicked() {
                    self.paste_block(data);
                }
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.state.about_to_paste_rows.clear();
                modal.close();
            }
            if ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.ctrl) {
                self.paste_block(data);
            }
        });
    }

    pub(crate) fn handle_selections(
        &mut self,
        selection_event: Option<SelectionEvent>,
        key_navigation: SelectionKeyNavigation,
    ) {
        if key_navigation.any_moves() {
            let was_selected = self.state.selected_range;
            if let Some(already_selected) = &mut self.state.selected_range {
                if key_navigation.left {
                    already_selected.move_left(key_navigation.shift);
                }
                if key_navigation.right {
                    already_selected
                        .move_right(key_navigation.shift, self.state.columns.len() as u32);
                }
                if key_navigation.up {
                    already_selected.move_up(key_navigation.shift);
                }
                if key_navigation.down {
                    already_selected.move_down(key_navigation.shift, 100); // TODO: unhardcode
                }
            }
            if was_selected != self.state.selected_range {
                self.state.save_cell_changes_and_deselect = true;
                if let Some(r) = self.state.selected_range {
                    self.state.last_pressed = Some((r.row_start, r.col_start));
                }
            }
        }
        let Some(e) = selection_event else {
            return;
        };
        debug!("{e:?}");
        match e {
            SelectionEvent::Pressed(row, col) => {
                if self.state.last_pressed == Some((row, col)) {
                    debug!("Ignoring");
                } else {
                    if let Some(already_selected) = self.state.selected_range {
                        if key_navigation.shift {
                            debug!("Ignoring due to shift pressed");
                            return;
                        }
                        if already_selected.is_single_cell() {
                            debug!("Deselect and save if any changes");
                            self.state.save_cell_changes_and_deselect = true;
                        } else {
                            debug!("Dropping multi cell selection");
                            self.state.selected_range = None;
                        }
                    }
                    self.state.last_pressed = Some((row, col));
                }
            }
            SelectionEvent::Released(row, col) => {
                let Some(prev_pressed) = self.state.last_pressed else {
                    return;
                };
                let new_selected_range =
                    SelectedRange::ordered(prev_pressed.0, prev_pressed.1, row, col);
                if self.state.selected_range != Some(new_selected_range) {
                    debug!("setting new selection range: {:?}", new_selected_range);
                    self.state.selected_range = Some(new_selected_range);
                } else {
                    debug!("ignoring same selection");
                }
            }
        }
    }

    pub(crate) fn paste_block(&mut self, data: &mut impl TableBackend) {
        let Some(selected_range) = &self.state.selected_range else {
            return;
        };
        let mut row_ids: Vec<Option<u32>> = (0..selected_range.height())
            .map(|mono_row_idx| data.row_uid(mono_row_idx + selected_range.row_start))
            .collect();

        if self.state.create_rows_on_paste
            && self.state.about_to_paste_rows.len() > selected_range.height() as usize
        {
            for _ in 0..self.state.about_to_paste_rows.len() - selected_range.height() as usize {
                row_ids.push(data.create_row(HashMap::new()));
            }
        }

        let col_ids: Vec<Option<(u32, VariantTy)>> = (0..selected_range.width())
            .map(|mono_col_idx| {
                self.state
                    .columns
                    .get((mono_col_idx + selected_range.col_start) as usize)
                    .map(|col| (col.col_uid, col.ty))
            })
            .collect();

        // TODO: create adhoc columns on paste
        // if self.state.create_adhoc_cols_on_paste {
        //     for _ in 0..self.state.about_to_paste_rows[0].len() - selected_range.width() {
        //         col_ids.push(data.create_column());
        //     }
        // }
        let mut changed_coords = vec![];

        if self.state.fill_with_same_on_paste {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter().cycle())
            {
                let Some(row_id) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter().cycle()) {
                    let Some((col_id, ty)) = col_id_ty else {
                        continue;
                    };
                    let coord = CellCoord(row_id, *col_id);
                    changed_coords.push(coord);
                    data.modify_one(coord, Variant::from_str(cell, *ty));
                }
            }
        } else {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter())
            {
                let Some(row_id) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter()) {
                    let Some((col_id, ty)) = col_id_ty else {
                        continue;
                    };
                    let coord = CellCoord(row_id, *col_id);
                    changed_coords.push(coord);
                    data.modify_one(coord, Variant::from_str(cell, *ty));
                }
            }
        }

        data.one_shot_flags_mut().cells_updated = changed_coords;
        self.state.about_to_paste_rows.clear();
    }

    pub(crate) fn handle_clear_request(&mut self, data: &mut impl TableBackend, modal: &mut Modal) {
        if !self.state.clear_requested {
            return;
        }
        modal.show(|ui| {
            modal.title(ui, "Clear all data");
            ui.horizontal(|ui| {
                modal.icon(ui, egui_modal::Icon::Warning);
                ui.label("About to clear all table's data, are you sure?");
            });
            // modal.frame(ui, |ui| {
            //     modal.icon(ui, egui_modal::Icon::Warning);
            //     modal.body(ui, "About to clear all table's data, are you sure?");
            // });
            modal.buttons(ui, |ui| {
                if modal.caution_button(ui, "Clear").clicked() {
                    self.state.clear_requested = false;
                    data.clear();
                }
                if modal.suggested_button(ui, "Cancel").clicked() {
                    self.state.clear_requested = false;
                    modal.close();
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.state.clear_requested = false;
                    modal.close();
                }
            });
        });
    }
}
