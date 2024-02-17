use super::csv::{CsvBackend, Separator};
use egui::{RichText, Slider, Ui};
use std::path::PathBuf;
use strum::IntoEnumIterator;

pub struct CsvBackendUi {
    state: PersistentState,
    picked_path: Option<PathBuf>,

    edit_header_names: bool,
    edit_mode: bool,
    // edit_text: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct PersistentState {
    separator: Separator,
    has_headers: bool,
    skip_first_rows: usize,

    row_height: f32,
    // skippable_rows: bool,
    // skippable_columns: bool,
}

impl Default for PersistentState {
    fn default() -> Self {
        PersistentState {
            separator: Separator::default(),
            has_headers: true,
            skip_first_rows: 0,
            row_height: 30.0,
            // skippable_rows: true,
            // skippable_columns: true,
        }
    }
}

impl CsvBackendUi {
    pub fn new() -> Self {
        CsvBackendUi {
            state: PersistentState::default(),
            picked_path: None,
            edit_header_names: false,
            edit_mode: false,
            // edit_text: None,
        }
    }

    pub fn show(&mut self, csv_backend: &mut CsvBackend, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("CSV Options").strong().monospace());

            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.picked_path = Some(path);
                    self.try_load(csv_backend);
                }
            }
            if ui.button("Reload").clicked() {
                self.try_load(csv_backend);
            }
            ui.separator();

            let delim_changed = egui::ComboBox::from_label("Separator")
                .selected_text(format!("{}", self.state.separator))
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    for s in Separator::iter() {
                        changed |= ui
                            .selectable_value(&mut self.state.separator, s, s.to_string())
                            .changed();
                    }
                    changed
                })
                .inner;
            if let Some(true) = delim_changed {
                self.try_load(csv_backend);
            }
            if ui
                .checkbox(&mut self.state.has_headers, "Has header row")
                .changed()
            {
                self.try_load(csv_backend);
            }

            ui.separator();
            if ui
                .add(Slider::new(&mut self.state.skip_first_rows, 0..=10).text("Skip first rows"))
                .on_hover_text("If file contains additional rows before header row, skip them")
                .changed()
            {
                self.try_load(csv_backend);
            }
            ui.separator();

            ui.checkbox(&mut self.edit_mode, "Edit mode")
                .on_hover_text("Allow editing of the data in the table");
            if self.state.has_headers {
                ui.checkbox(&mut self.edit_header_names, "Edit header names");
            }
            ui.separator();

            ui.add(
                Slider::new(&mut self.state.row_height, 10.0..=64.0)
                    .text("Row height")
                    .step_by(2.0),
            )
        });
        if csv_backend.status().is_error() {
            // error_label(csv_table.status(), ui);
            ui.label(format!("{:?}", csv_backend.status()));
        }
    }

    fn try_load(&mut self, csv_backend: &mut CsvBackend) {
        let Some(path) = self.picked_path.clone() else {
            return;
        };
        csv_backend.set_separator(self.state.separator);
        csv_backend.skip_rows_on_load(self.state.skip_first_rows);
        csv_backend.load(path);
    }

    pub fn has_warnings(&self) -> bool {
        false
    }
}
