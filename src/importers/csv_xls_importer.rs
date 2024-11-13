use super::csv::{CsvImporter, Separator};
use crate::backends::variant::VariantBackend;
use crate::{RequiredColumns, TableView};
use egui::{RichText, Slider, Ui};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use strum::IntoEnumIterator;

pub struct CsvXlsImporter {
    csv: CsvImporter,
    backend: VariantBackend,
    table_view: TableView,
    state: PersistentState,
    picked_file: Option<PathBuf>,
}

// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Serialize, Deserialize)]
struct PersistentState {
    separator: Separator,
    has_headers: bool,
    skip_first_rows: usize,
}

impl Default for PersistentState {
    fn default() -> Self {
        PersistentState {
            separator: Separator::default(),
            has_headers: true,
            skip_first_rows: 0,
        }
    }
}

impl CsvXlsImporter {
    pub fn new(required_columns: RequiredColumns) -> Self {
        let backend = VariantBackend::new(
            required_columns
                .required_columns
                .iter()
                .map(|(_, c)| (c.name.clone(), c.ty, c.default.clone())),
        );
        CsvXlsImporter {
            csv: CsvImporter::new(required_columns),
            backend,
            table_view: TableView::new(),
            state: PersistentState::default(),
            picked_file: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("CSV Options").strong().monospace());

            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.picked_file = Some(path);
                    self.try_load();
                }
            }
            if ui.button("Reload").clicked() {
                self.try_load();
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
                self.try_load();
            }
            if ui
                .checkbox(&mut self.state.has_headers, "Has header row")
                .changed()
            {
                self.try_load();
            }

            ui.separator();
            if ui
                .add(Slider::new(&mut self.state.skip_first_rows, 0..=10).text("Skip first rows"))
                .on_hover_text("If file contains additional rows before header row, skip them")
                .changed()
            {
                self.try_load();
            }
            ui.separator();
        });
        if self.csv.status().is_error() {
            // error_label(csv_table.status(), ui);
            ui.label(format!("{:?}", self.csv.status()));
        }
        self.table_view.show(&mut self.backend, ui);
    }

    fn try_load(&mut self) {
        let Some(path) = self.picked_file.clone() else {
            return;
        };
        self.csv.set_separator(self.state.separator);
        self.csv.skip_rows_on_load(self.state.skip_first_rows);
        self.csv.load(path, &mut self.backend);
    }

    pub fn has_warnings(&self) -> bool {
        false
    }

    pub fn picked_file(&self) -> Option<PathBuf> {
        self.picked_file.clone()
    }

    pub fn backend(&self) -> &VariantBackend {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut VariantBackend {
        &mut self.backend
    }
}
