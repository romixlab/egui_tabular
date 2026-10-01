use crate::TableView;
use crate::cell_ui::CellUi;
use crate::commands::TableCommand;
use crate::table_view::TableEvent;
use crate::table_view::state::SelectedRange;
use egui::{Event, Id, Key, Modal, Ui};
use itertools::Itertools;
use tabular_core::{
    Capabilities, CellCoord, CellLevel, ColumnUid, RowPosition, RowUid, TableModel,
};

impl TableView {
    pub(crate) fn handle_key_input<M: TableModel, C: CellUi<M>>(
        &mut self,
        model: &M,
        cell_ui: &C,
        caps: Capabilities,
        ui: &mut Ui,
    ) {
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(Key::C)) {
            // command+C don't work: https://github.com/emilk/egui/issues/4065
            if let Some(text) = self.selection_text(model, cell_ui) {
                ui.ctx().copy_text(text);
            }
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::A)) {
            self.state.selected_range = Some(SelectedRange::rect(
                self.state.columns_ordered.len(),
                self.state.rows.len(),
            ));
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.state.selected_range = None;
        }
        if caps.create_rows && ui.input(|i| i.key_pressed(Key::N)) {
            self.state.commands.push(TableCommand::CreateRows {
                at: RowPosition::Append,
                count: 1,
            });
        }
        self.handle_selection_moves(caps, ui);
    }

    /// The selected cells as tab-separated text.
    fn selection_text<M: TableModel, C: CellUi<M>>(
        &self,
        model: &M,
        cell_ui: &C,
    ) -> Option<String> {
        let selected = self.state.selected_range?;
        let cols: Vec<ColumnUid> = (selected.col_start()..=selected.col_end())
            .filter_map(|idx| self.state.columns_ordered.get(idx).copied())
            .collect();
        let mut text = String::new();
        for row_idx in selected.row_start()..=selected.row_end() {
            let Some(row_uid) = self.state.rows.rows().get(row_idx).copied() else {
                continue;
            };
            text += &crate::util::row_texts(model, cell_ui, row_uid, &cols).join("\t");
            if row_idx != selected.row_end() {
                text += "\n";
            }
        }
        (!text.is_empty()).then_some(text)
    }

    pub(crate) fn handle_key_input_when_editing(&mut self, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(Key::Tab)) {
            if let Some(coord) = self.state.selected_range.and_then(|r| r.editing()) {
                self.state.commit_edit(coord);
            }
            if let Some(selected) = &mut self.state.selected_range {
                selected.move_right(false, self.state.columns_ordered.len());
            }
            self.edit_selected_cell();
        }

        if ui.input(|i| i.key_pressed(Key::Escape))
            && let Some(selected) = &mut self.state.selected_range
        {
            selected.set_editing(None);
        }
    }

    /// Start editing the selected cell, if a single cell is selected. The cell is taken from the
    /// view's row and column order.
    fn edit_selected_cell(&mut self) {
        let Some(selected) = &mut self.state.selected_range else {
            return;
        };
        if !selected.is_single_cell() {
            return;
        }
        let row = self.state.rows.rows().get(selected.row_start());
        let col = self.state.columns_ordered.get(selected.col_start());
        if let (Some(row_uid), Some(col_uid)) = (row, col) {
            selected.set_editing(Some(CellCoord {
                row_uid: *row_uid,
                col_uid: *col_uid,
            }));
        }
    }

    pub(crate) fn handle_paste(&mut self, paste_from_empty: bool, caps: Capabilities, ui: &mut Ui) {
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

        if paste_from_empty {
            if !(caps.create_columns && caps.create_rows) {
                self.message(
                    CellLevel::Warning,
                    "This table can't create columns and rows to paste into",
                );
                return;
            }
            self.state.selected_range = Some(SelectedRange::rect(
                self.state.pasting_block_width,
                rows.len(),
            ));
            self.state.commands.push(TableCommand::Paste {
                rows: vec![],
                create_rows: rows.len(),
                columns: vec![],
                create_columns: self.state.pasting_block_width,
                block: rows,
                repeat: false,
            });
            return;
        }
        if let Some(selected_range) = &self.state.selected_range {
            let selection_is_exact = rows.len() == selected_range.height()
                && self.state.pasting_block_width == selected_range.width()
                && is_equal_lengths;
            self.state.about_to_paste_rows = rows;
            if selection_is_exact {
                self.paste_block();
            } else {
                // ask user what to do in handle_paste_continue
                self.state.create_rows_on_paste = false;
                self.state.fill_with_same_on_paste = false;
                self.state.create_cols_on_paste = false;
            }
        } else {
            self.message(CellLevel::Info, "Select a cell to paste into");
        }
    }

    pub(super) fn message(&mut self, level: CellLevel, text: impl Into<String>) {
        self.state.events.push(TableEvent::Message {
            level,
            text: text.into(),
        });
    }

    pub(crate) fn handle_paste_continue(&mut self, id: Id, ui: &mut Ui) {
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
                    if ui.button("Paste clip").clicked() {
                        should_paste = true;
                    }
                    // if ui.button("Paste overflow").clicked() {
                    // TODO: paste overflow
                    // }
                },
            );
        });

        if should_paste {
            self.paste_block();
        }
        if should_close || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.state.about_to_paste_rows.clear();
        }
    }

    /// Queue pasting `about_to_paste_rows` into the selection, with the options of the paste dialog.
    pub(crate) fn paste_block(&mut self) {
        let Some(selected) = self.state.selected_range else {
            return;
        };
        let block = std::mem::take(&mut self.state.about_to_paste_rows);
        let rows: Vec<RowUid> = (selected.row_start()..=selected.row_end())
            .filter_map(|idx| self.state.rows.rows().get(idx).copied())
            .collect();
        let columns: Vec<ColumnUid> = (selected.col_start()..=selected.col_end())
            .filter_map(|idx| self.state.columns_ordered.get(idx).copied())
            .collect();
        let create_rows = if self.state.create_rows_on_paste {
            block.len().saturating_sub(selected.height())
        } else {
            0
        };
        let create_columns = if self.state.create_adhoc_cols_on_paste {
            self.state
                .pasting_block_width
                .saturating_sub(selected.width())
        } else {
            0
        };
        self.state.commands.push(TableCommand::Paste {
            rows,
            create_rows,
            columns,
            create_columns,
            block,
            repeat: self.state.fill_with_same_on_paste,
        });
    }

    fn handle_selection_moves(&mut self, caps: Capabilities, ui: &mut Ui) {
        let (left, right, up, down, edit, shift) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::E),
                i.modifiers.shift,
            )
        });
        let row_count = self.state.rows.len();
        if left || right || up || down {
            if let Some(already_selected) = &mut self.state.selected_range {
                if left {
                    already_selected.move_left(shift);
                }
                if right {
                    already_selected.move_right(shift, self.state.columns_ordered.len());
                }
                if up {
                    already_selected.move_up(shift);
                    self.state.rows.reveal(already_selected.row_start());
                }
                if down {
                    already_selected.move_down(shift, row_count);
                    self.state.rows.reveal(already_selected.row_end());
                }
            } else if row_count > 0 && !self.state.columns_ordered.is_empty() {
                self.state.selected_range = Some(SelectedRange::single_cell(0, 0));
            }
        }
        if edit && caps.edit_cells {
            self.edit_selected_cell();
        }
    }
}
