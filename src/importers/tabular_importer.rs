use super::csv::{CsvImporter, Separator};
use crate::backends::variant::VariantBackend;
use crate::{RequiredColumns, TableView};
use egui::{RichText, Slider, Ui};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use strum::IntoEnumIterator;

pub struct TabularImporter {
    csv: CsvImporter,
    pub backend: VariantBackend,
    pub table_view: TableView,
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

impl TabularImporter {
    pub fn new(required_columns: RequiredColumns) -> Self {
        let mut backend = VariantBackend::new([]);
        //     required_columns
        //         .required_columns
        //         .iter()
        //         .map(|(_, c)| (c.name.clone(), c.ty, c.default.clone())),
        // );
        for (uid, r) in required_columns.required_columns.iter() {
            backend.insert_column(
                *uid,
                r.name.clone(),
                r.synonyms.clone(),
                r.ty,
                r.default.clone(),
                true,
                true,
            );
        }
        TabularImporter {
            csv: CsvImporter::new(required_columns),
            backend,
            table_view: TableView::new(),
            state: PersistentState::default(),
            picked_file: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut reloaded = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("CSV Options").strong().monospace());

            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.picked_file = Some(path);
                    reloaded = true;
                    self.try_load();
                }
            }
            if ui.button("Reload").clicked() {
                reloaded = true;
                self.try_load();
            }
            ui.separator();

            ui.label("Separator:");
            let delim_changed = egui::ComboBox::from_label("")
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
                reloaded = true;
                self.try_load();
            }
            if ui
                .checkbox(&mut self.state.has_headers, "Has header row")
                .changed()
            {
                reloaded = true;
                self.try_load();
            }

            ui.separator();
            ui.label("Skip first rows:");
            if ui
                .add(Slider::new(&mut self.state.skip_first_rows, 0..=10))
                .on_hover_text("If file contains additional rows before header row, skip them")
                .changed()
            {
                reloaded = true;
                self.try_load();
            }
            ui.separator();
        });
        if self.csv.status().is_error() {
            // error_label(csv_table.status(), ui);
            ui.label(format!("{:?}", self.csv.status()));
        }
        self.table_view.show(&mut self.backend, ui);
        reloaded
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
