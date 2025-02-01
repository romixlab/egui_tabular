use super::required_column::RequiredColumns;
use crate::backend::{ColumnUid, TableBackend};
use crate::backends::variant::VariantBackend;
use log::{trace, warn};
use rvariant::{Variant, VariantTy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

pub(crate) struct CsvImporter {
    required_columns: RequiredColumns,

    separator: Separator,
    skip_first_rows: usize,

    state: State,
}

#[derive(Default)]
struct State {
    status: IoStatus,
}

#[allow(dead_code)]
#[derive(Default, Debug)]
pub enum IoStatus {
    #[default]
    Empty,
    IoError(std::io::Error),
    ReaderError(csv::Error),
    ReaderErrorAtLine(usize, csv::Error),
    Loaded(PathBuf),
    Edited,
    UnknownSeparator,
    // Warning,
}

impl IoStatus {
    pub fn is_error(&self) -> bool {
        match self {
            IoStatus::Empty => false,
            IoStatus::IoError(_) | IoStatus::ReaderError(_) | IoStatus::ReaderErrorAtLine(_, _) => {
                true
            }
            IoStatus::Loaded(_) => false,
            IoStatus::Edited => false,
            IoStatus::UnknownSeparator => true,
        }
    }
}

#[derive(
    strum::EnumIter, strum::Display, PartialEq, Copy, Clone, Default, Serialize, Deserialize,
)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Separator {
    #[default]
    Auto,
    Comma,
    Tab,
    Semicolon,
}

impl CsvImporter {
    pub fn new(required_columns: RequiredColumns) -> Self {
        CsvImporter {
            required_columns,
            separator: Default::default(),
            skip_first_rows: 0,
            state: State::default(),
        }
    }

    pub fn set_separator(&mut self, separator: Separator) {
        self.separator = separator;
    }

    pub fn skip_rows_on_load(&mut self, count: usize) {
        self.skip_first_rows = count;
    }

    pub fn load(&mut self, path: PathBuf, backend: &mut VariantBackend) {
        trace!("CsvImporter: loading: {path:?}");

        backend.remove_all_columns();
        let separator = match self.determine_separator(&path) {
            Some(value) => value,
            None => {
                self.state.status = IoStatus::UnknownSeparator;
                return;
            }
        };

        match csv::ReaderBuilder::new()
            .delimiter(separator)
            .has_headers(false) // to be able to ignore first N rows
            .flexible(true)
            .from_path(path.clone())
        {
            Ok(mut rdr) => {
                let mut records = rdr.records();
                for _ in 0..self.skip_first_rows {
                    records.next();
                }
                let csv_to_col_uid = match records.next() {
                    Some(Ok(headers)) => {
                        let headers: Vec<&str> = headers.iter().collect();
                        let csv_to_col_uid = self.map_columns(headers, backend);
                        // self.state.columns = columns;
                        csv_to_col_uid
                    }
                    Some(Err(e)) => {
                        self.state.status = IoStatus::ReaderError(e);
                        return;
                    }
                    None => {
                        self.state.status = IoStatus::Empty;
                        return;
                    }
                };
                for (row_idx, record) in records.enumerate() {
                    match record {
                        Ok(record) => {
                            backend.insert_row(record.iter().enumerate().map(
                                |(csv_idx, cell_value)| {
                                    let col_uid = csv_to_col_uid.get(&csv_idx).copied().unwrap();
                                    let value = self.convert_cell_value(col_uid, cell_value);
                                    (col_uid, value)
                                },
                            ));
                        }
                        Err(e) => {
                            self.state.status =
                                IoStatus::ReaderErrorAtLine(row_idx + 1 + self.skip_first_rows, e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                self.state.status = IoStatus::ReaderError(e);
            }
        }
        self.state.status = IoStatus::Loaded(path);
        backend.one_shot_flags_mut().column_info_updated = true;
        backend.one_shot_flags_mut().reloaded = true;
    }

    fn convert_cell_value(&self, col_uid: ColumnUid, value: &str) -> Variant {
        if let Some(r) = self.required_columns.get(col_uid) {
            Variant::from_str(value, r.ty)
        } else {
            Variant::Str(value.to_string())
        }
    }

    fn determine_separator(&mut self, path: &PathBuf) -> Option<u8> {
        Some(match self.separator {
            Separator::Auto => {
                let file = match File::open(path) {
                    Ok(file) => file,
                    Err(e) => {
                        self.state.status = IoStatus::IoError(e);
                        return None;
                    }
                };
                let reader = BufReader::new(file);
                let mut counts: [(usize, u8); 3] = [(0, b','), (1, b'\t'), (2, b';')];
                for b in reader.bytes() {
                    let Ok(b) = b else {
                        break;
                    };
                    match b {
                        b',' => counts[0].0 += 1,
                        b'\t' => counts[1].0 += 1,
                        b';' => counts[2].0 += 1,
                        _ => {}
                    }
                }
                counts.sort_by(|a, b| a.0.cmp(&b.0));
                // debug!("{counts:?}");
                counts[2].1
            }
            Separator::Comma => b',',
            Separator::Tab => b'\t',
            Separator::Semicolon => b';',
        })
    }

    fn map_columns(
        &mut self,
        csv_columns: Vec<&str>,
        backend: &mut VariantBackend,
    ) -> HashMap<usize, ColumnUid> {
        // let mut columns = HashMap::new();
        let mut csv_to_col_uid = HashMap::new();

        // Place required columns first, if match is not found in a loaded file - map to empty columns
        let mapped_columns = self.required_columns.map_columns(&csv_columns);
        let mut next_absent_col_uid = ColumnUid(mapped_columns.len() as u32);
        for ((col_uid, col), csv_col_idx) in mapped_columns {
            if let Some(csv_col_idx) = csv_col_idx {
                if csv_to_col_uid.contains_key(&csv_col_idx) {
                    warn!("Double match for column: {}", col.name);
                }
                csv_to_col_uid.insert(csv_col_idx, col_uid);
            }
            backend.insert_column(
                col_uid,
                col.name.clone(),
                col.synonyms.clone(),
                col.ty,
                col.default.clone(),
                true,
                true,
            );
        }

        // Put all additional columns to the right of required ones
        for (csv_idx, column) in csv_columns.iter().enumerate() {
            if !csv_to_col_uid.contains_key(&csv_idx) {
                csv_to_col_uid.insert(csv_idx, next_absent_col_uid);
                backend.insert_column(
                    next_absent_col_uid,
                    column.to_string(),
                    vec![],
                    VariantTy::Str,
                    None,
                    false,
                    false,
                );
                next_absent_col_uid = ColumnUid(next_absent_col_uid.0 + 1);
            }
        }

        csv_to_col_uid
    }

    pub fn status(&self) -> &IoStatus {
        &self.state.status
    }
}
