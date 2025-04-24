use super::required_column::RequiredColumns;
use crate::backend::TableBackend;
use crate::backends::variant::VariantBackend;
use crate::util::base_26;
use log::{trace, warn};
use rvariant::{Variant, VariantTy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, Read, Seek, SeekFrom};
use tabular_core::ColumnUid;

pub(crate) struct CsvImporter {
    required_columns: RequiredColumns,
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
    Loaded,
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
            IoStatus::Loaded => false,
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

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct CsvImporterConfig {
    pub(crate) separator: Separator,
    separator_u8: u8,
    pub(crate) skip_first_rows: usize,
    pub(crate) has_headers: bool,
}

impl Default for CsvImporterConfig {
    fn default() -> Self {
        CsvImporterConfig {
            separator: Default::default(),
            separator_u8: b',',
            skip_first_rows: 0,
            has_headers: true,
        }
    }
}

impl CsvImporter {
    pub fn new(required_columns: RequiredColumns) -> Self {
        CsvImporter {
            required_columns,
            state: State::default(),
        }
    }

    pub fn load<R: Read + Seek>(
        &mut self,
        config: &mut CsvImporterConfig,
        rdr: &mut BufReader<R>,
        backend: &mut VariantBackend,
        max_lines: Option<usize>,
    ) {
        trace!("CsvImporter: loading");

        backend.remove_all_columns();
        let separator = match self.determine_separator(config, rdr) {
            Some(value) => value,
            None => {
                self.state.status = IoStatus::UnknownSeparator;
                return;
            }
        };
        config.separator_u8 = separator;
        rdr.seek(SeekFrom::Start(0)).unwrap();

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(separator)
            .has_headers(false) // to be able to ignore first N rows
            .flexible(true)
            .from_reader(rdr);
        // .from_path(path.clone())
        let mut records = rdr.records();
        for _ in 0..config.skip_first_rows {
            records.next();
        }
        // TODO: handle no headers case
        let csv_to_col_uid = if config.has_headers {
            match records.next() {
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
            }
        } else {
            HashMap::new()
        };
        let mut lines_read = 0;
        let mut max_col_idx = 0;
        for (row_idx, record) in records.enumerate() {
            match record {
                Ok(record) => {
                    backend.insert_row(record.iter().enumerate().map(|(csv_idx, cell_value)| {
                        let col_uid = csv_to_col_uid
                            .get(&csv_idx)
                            .copied()
                            .unwrap_or(ColumnUid(csv_idx as u32));
                        let value = self.convert_cell_value(col_uid, cell_value);
                        max_col_idx = max_col_idx.max(csv_idx);
                        (col_uid, value)
                    }));
                    if let Some(max_lines) = max_lines {
                        lines_read += 1;
                        if lines_read >= max_lines {
                            break;
                        }
                    }
                }
                Err(e) => {
                    self.state.status =
                        IoStatus::ReaderErrorAtLine(row_idx + 1 + config.skip_first_rows, e);
                    break;
                }
            }
        }
        if csv_to_col_uid.is_empty() {
            for col_idx in 0..max_col_idx {
                backend.insert_column(
                    Some(ColumnUid(col_idx as u32)),
                    base_26(col_idx as u32 + 1),
                    vec![],
                    VariantTy::Str,
                    None,
                    false,
                    true,
                );
            }
        }
        self.state.status = IoStatus::Loaded;
        backend.one_shot_flags_mut().columns_reset = true;
        backend.one_shot_flags_mut().reloaded = true;
    }

    fn convert_cell_value(&self, col_uid: ColumnUid, value: &str) -> Variant {
        if let Some(r) = self.required_columns.get(col_uid) {
            Variant::from_str(value, r.ty)
        } else {
            Variant::Str(value.to_string())
        }
    }

    fn determine_separator<R: Read + Seek>(
        &mut self,
        config: &CsvImporterConfig,
        rdr: &mut BufReader<R>,
    ) -> Option<u8> {
        Some(match config.separator {
            Separator::Auto => {
                let mut counts: [(usize, u8); 3] = [(0, b','), (1, b'\t'), (2, b';')];
                for b in rdr.bytes().take(1024 * 1024) {
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
                Some(col_uid),
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
                    Some(next_absent_col_uid),
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

impl CsvImporterConfig {
    pub fn separator(&self) -> u8 {
        self.separator_u8
    }

    pub fn skip_first_rows(&self) -> usize {
        self.skip_first_rows
    }

    pub fn has_headers(&self) -> bool {
        self.has_headers
    }
}
