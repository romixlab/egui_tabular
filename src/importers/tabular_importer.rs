use super::csv::{CsvImporter, CsvImporterConfig, Separator};
use crate::backends::variant::VariantBackend;
use crate::table_view::TableViewConfig;
use crate::{RequiredColumns, TableView};
use egui::{RichText, Slider, Ui};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;

pub struct TabularImporter {
    csv: CsvImporter,
    pub backend: VariantBackend,
    pub table_view: TableView,
    file: Option<File>,
    load_rows_limit: Option<usize>,
}

// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Serialize, Deserialize)]
pub struct TabularImporterConfig {
    view_config: TableViewConfig,
    importer_config: CsvImporterConfig,
    picked_file: Option<PathBuf>,
}

impl Default for TabularImporterConfig {
    fn default() -> Self {
        TabularImporterConfig {
            view_config: Default::default(),
            importer_config: Default::default(),
            picked_file: None,
        }
    }
}

impl TabularImporter {
    pub fn new(required_columns: RequiredColumns) -> Self {
        let mut backend = VariantBackend::new([]);
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
            file: None,
            load_rows_limit: None,
        }
    }

    pub fn show(&mut self, config: &mut TabularImporterConfig, ui: &mut Ui) -> bool {
        let mut reloaded = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("CSV Options").strong().monospace());

            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    config.picked_file = Some(path.clone());
                    let file = File::open(path).unwrap();
                    self.file = Some(file);
                    reloaded = true;
                    self.try_load(config);
                }
            }
            if ui.button("Reload").clicked() {
                reloaded = true;
                self.try_load(config);
            }
            ui.separator();

            ui.label("Separator:");
            let delim_changed = egui::ComboBox::from_label("")
                .selected_text(format!("{}", config.importer_config.separator))
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    for s in Separator::iter() {
                        changed |= ui
                            .selectable_value(
                                &mut config.importer_config.separator,
                                s,
                                s.to_string(),
                            )
                            .changed();
                    }
                    changed
                })
                .inner;
            if let Some(true) = delim_changed {
                reloaded = true;
                self.try_load(config);
            }
            if ui
                .checkbox(&mut config.importer_config.has_headers, "Has header row")
                .changed()
            {
                reloaded = true;
                self.try_load(config);
            }

            ui.separator();
            ui.label("Skip first rows:");
            if ui
                .add(Slider::new(
                    &mut config.importer_config.skip_first_rows,
                    0..=10,
                ))
                .on_hover_text("If file contains additional rows before header row, skip them")
                .changed()
            {
                reloaded = true;
                self.try_load(config);
            }
        });
        if self.csv.status().is_error() {
            // error_label(csv_table.status(), ui);
            ui.label(format!("{:?}", self.csv.status()));
        }
        self.table_view
            .show(&mut self.backend, &mut config.view_config, ui);
        reloaded
    }

    fn try_load(&mut self, config: &TabularImporterConfig) {
        let Some(file) = &self.file else {
            return;
        };
        let mut rdr = BufReader::new(file);
        self.csv.load(
            &config.importer_config,
            &mut rdr,
            &mut self.backend,
            self.load_rows_limit,
        );
    }

    pub fn has_warnings(&self) -> bool {
        false
    }

    pub fn backend(&self) -> &VariantBackend {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut VariantBackend {
        &mut self.backend
    }

    pub fn set_max_lines(&mut self, max_lines: usize) {
        self.load_rows_limit = Some(max_lines);
    }

    pub fn load(&mut self, path: PathBuf, config: &mut TabularImporterConfig) {
        config.picked_file = Some(path.clone());
        let file = File::open(path).unwrap();
        self.file = Some(file);
        self.try_load(config);
    }

    pub fn reload(&mut self, config: &TabularImporterConfig) {
        if let Some(path) = &config.picked_file {
            let file = File::open(path).unwrap();
            self.file = Some(file);
            self.try_load(config);
        }
    }

    pub fn take_file(&mut self) -> Option<File> {
        self.file.take()
    }
}

impl TabularImporterConfig {
    pub fn picked_file(&self) -> Option<PathBuf> {
        self.picked_file.clone()
    }

    pub fn picked_file_ref(&self) -> Option<&Path> {
        self.picked_file.as_ref().map(|p| p.as_path())
    }
}
