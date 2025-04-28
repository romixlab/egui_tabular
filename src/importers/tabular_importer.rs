use super::csv::CsvImporter;
use crate::backends::variant::VariantBackend;
use crate::table_view::TableViewConfig;
use crate::{RequiredColumns, TableView};
use egui::{Id, RichText, Slider, Ui};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;
use tabular_core::{CsvImporterConfig, Separator};

pub struct TabularImporter {
    csv: CsvImporter,
    pub backend: VariantBackend,
    pub table_view: TableView,
    open_error: Option<String>,
    load_rows_limit: Option<usize>,
}

// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Serialize, Deserialize)]
pub struct TabularImporterConfig {
    pub view_config: TableViewConfig,
    pub importer_config: CsvImporterConfig,
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
                Some(*uid),
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
            open_error: None,
            load_rows_limit: None,
        }
    }

    pub fn show(&mut self, config: &mut TabularImporterConfig, ui: &mut Ui, id: Id) {
        ui.horizontal(|ui| {
            let label = if let Some(limit) = self.load_rows_limit {
                RichText::new(format!("Preview file ({limit} rows):"))
            } else {
                RichText::new("File:")
            };
            ui.label(label.strong().monospace());
            if let Some(path) = &config.picked_file {
                ui.label(
                    path.to_str()
                        .unwrap_or("Path contains invalid Unicode, but load should work anyway"),
                );
            } else {
                ui.label("Picked file:");
            }
            if let Some(e) = &self.open_error {
                ui.colored_label(ui.visuals().warn_fg_color, e);
            }
            if ui.button("Open").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    config.picked_file = Some(path.clone());

                    self.reload(config);
                }
            }
            if ui.button("Reload").clicked() {
                self.reload(config);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("CSV Options").strong().monospace());

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
                self.reload(config);
            }
            if ui
                .checkbox(&mut config.importer_config.has_headers, "Has header row")
                .changed()
            {
                self.reload(config);
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
                self.reload(config);
            }
        });
        if self.csv.status().is_error() {
            // error_label(csv_table.status(), ui);
            ui.label(format!("{:?}", self.csv.status()));
        }
        ui.separator();
        self.table_view
            .show(&mut self.backend, &mut config.view_config, ui, id);
    }

    pub fn load(&mut self, path: PathBuf, config: &mut TabularImporterConfig) {
        config.picked_file = Some(path.clone());
        self.reload(config);
    }

    pub fn reload(&mut self, config: &mut TabularImporterConfig) {
        let Some(path) = &config.picked_file else {
            self.open_error = None;
            return;
        };
        let file = match File::open(path) {
            Ok(file) => {
                self.open_error = None;
                file
            }
            Err(e) => {
                self.open_error = Some(format!("{e}"));
                return;
            }
        };
        let mut rdr = BufReader::new(file);
        self.csv.load(
            &mut config.importer_config,
            &mut rdr,
            &mut self.backend,
            self.load_rows_limit,
        );
    }

    pub fn has_warnings(&self) -> bool {
        self.open_error.is_some()
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
}

impl TabularImporterConfig {
    pub fn picked_file(&self) -> Option<PathBuf> {
        self.picked_file.clone()
    }

    pub fn picked_file_ref(&self) -> Option<&Path> {
        self.picked_file.as_ref().map(|p| p.as_path())
    }
}
