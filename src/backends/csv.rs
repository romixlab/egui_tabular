use crate::backend::{OneShotFlags, PersistentFlags, TableBackend};
use crate::cell::{CellCoord, CellKind, StaticCellKind, TableCell, TableCellRef};
use crate::column::{BackendColumn, TableColumn};
use crate::filter::RowFilter;
use log::{trace, warn};
use rvariant::{Variant, VariantTy};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

pub struct CsvBackend {
    required_columns: Vec<TableColumn>,
    separator: Separator,
    skip_first_rows: usize,

    state: State,
}

#[derive(Default)]
struct State {
    persistent_flags: PersistentFlags,
    one_shot_flags: OneShotFlags,

    columns: HashMap<u32, BackendColumn>,
    status: IoStatus,
    cells: HashMap<CellCoord, TableCell>,
    row_uid: Vec<u32>,
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

#[derive(strum::EnumIter, strum::Display, PartialEq, Copy, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Separator {
    Auto,
    #[default]
    Comma,
    Tab,
    Semicolon,
}

impl CsvBackend {
    pub fn new(required_columns: impl IntoIterator<Item = TableColumn>) -> Self {
        CsvBackend {
            required_columns: required_columns.into_iter().collect(),
            separator: Default::default(),
            skip_first_rows: 0,
            state: State {
                one_shot_flags: OneShotFlags {
                    first_pass: true,
                    ..OneShotFlags::default()
                },
                ..State::default()
            },
        }
    }

    pub fn create_new_table(&mut self) {
        self.state = State::default();
        self.state.columns = self
            .required_columns
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                (
                    idx as u32,
                    BackendColumn {
                        name: c.name.clone(),
                        ty: c.ty,
                        default_value: None,
                        kind: CellKind::Static(StaticCellKind::Plain),
                    },
                )
            })
            .collect();
        self.state.persistent_flags.column_info_present = true;
        self.state.one_shot_flags.column_info_updated = true;
    }

    pub fn set_separator(&mut self, separator: Separator) {
        self.separator = separator;
    }

    pub fn skip_rows_on_load(&mut self, count: usize) {
        self.skip_first_rows = count;
    }

    pub fn load(&mut self, path: PathBuf) {
        trace!("CsvTable: loading: {path:?}");

        self.clear();
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
                let csv_to_coord = match records.next() {
                    Some(Ok(headers)) => {
                        if self.required_columns.is_empty() {
                            self.state.columns = headers
                                .iter()
                                .enumerate()
                                .map(|(idx, s)| {
                                    (
                                        idx as u32,
                                        BackendColumn {
                                            name: s.to_owned(),
                                            ty: VariantTy::Str,
                                            default_value: None,
                                            kind: CellKind::Adhoc,
                                        },
                                    )
                                })
                                .collect();
                            HashMap::new()
                        } else {
                            let headers: Vec<&str> = headers.iter().collect();
                            let (columns, csv_to_coord) = self.map_columns(headers);
                            self.state.columns = columns;
                            csv_to_coord
                        }
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
                            self.state.row_uid.push(row_idx as u32);
                            for (csv_idx, field) in record.iter().enumerate() {
                                let col_idx =
                                    csv_to_coord.get(&csv_idx).cloned().unwrap_or(csv_idx) as u32;
                                let value = self.convert_cell_value(col_idx, field);
                                if !value.is_empty() {
                                    self.state.cells.insert(
                                        CellCoord(row_idx as u32, col_idx),
                                        TableCell::Available {
                                            value,
                                            is_dirty: false,
                                            in_conflict: false,
                                        },
                                    );
                                }
                            }
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
        self.state.one_shot_flags.column_info_updated = true;
        self.state.one_shot_flags.reloaded = true;
    }

    fn convert_cell_value(&self, col_idx: u32, value: &str) -> Variant {
        if let Some(r) = self.required_columns.get(col_idx as usize) {
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
        &self,
        csv_columns: Vec<&str>,
    ) -> (HashMap<u32, BackendColumn>, HashMap<usize, usize>) {
        let mut columns = HashMap::new();
        let mut csv_to_coord = HashMap::new();

        // Place required columns first, if match is not found in a loaded file - map to empty columns
        for (required_idx, required) in self.required_columns.iter().enumerate() {
            if let Some(csv_idx) = required.find_match_arr(&csv_columns) {
                if csv_to_coord.contains_key(&csv_idx) {
                    warn!("Double match for column: {}", required.name);
                } else {
                    csv_to_coord.insert(csv_idx, required_idx);
                }
            }

            columns.insert(
                required_idx as u32,
                BackendColumn {
                    name: required.name.clone(),
                    ty: required.ty,
                    default_value: None,
                    kind: CellKind::Static(StaticCellKind::Plain),
                },
            );
        }
        trace!("csv_to_coord required: {csv_to_coord:?}");

        // Put all additional columns to the right of required ones
        let next_absent_idx = self.required_columns.len();
        for (csv_idx, column) in csv_columns.iter().enumerate() {
            if !csv_to_coord.contains_key(&csv_idx) {
                let col_idx = next_absent_idx + csv_idx;
                csv_to_coord.insert(csv_idx, col_idx);
                let col_idx = col_idx as u32;
                columns.insert(
                    col_idx,
                    BackendColumn {
                        name: column.to_string(),
                        ty: VariantTy::Str,
                        default_value: None,
                        kind: CellKind::Adhoc,
                    },
                );
            }
        }
        trace!("csv_to_coord all: {csv_to_coord:?}");
        (columns, csv_to_coord)
    }

    pub fn status(&self) -> &IoStatus {
        &self.state.status
    }
}

impl TableBackend for CsvBackend {
    fn reload(&mut self) {}

    fn fetch_all(&mut self) {}

    fn fetch(&mut self, _col_uid_set: impl Iterator<Item = u32>) {}

    fn commit_all(&mut self) {
        todo!()
    }

    fn commit_immediately(&mut self, _enabled: bool) {
        todo!()
    }

    fn persistent_flags(&self) -> &PersistentFlags {
        &self.state.persistent_flags
    }

    fn one_shot_flags(&self) -> &OneShotFlags {
        &self.state.one_shot_flags
    }

    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags {
        &mut self.state.one_shot_flags
    }

    fn poll(&mut self) {
        self.state.one_shot_flags = OneShotFlags::default();
    }

    fn available_columns(&self) -> &HashMap<u32, BackendColumn> {
        &self.state.columns
    }

    fn used_columns(&self) -> &HashMap<u32, BackendColumn> {
        &self.state.columns
    }

    fn use_column(&mut self, _col: u32, _is_used: bool) {}

    fn row_count(&self) -> u32 {
        self.state.row_uid.len() as u32
    }

    fn row_uid(&self, monotonic_idx: u32) -> Option<u32> {
        self.state.row_uid.get(monotonic_idx as usize).cloned()
    }

    fn row_monotonic(&self, _uid: u32) -> Option<u32> {
        todo!()
    }

    fn cell(&mut self, cell: CellCoord) -> TableCellRef {
        self.state
            .cells
            .get(&cell)
            .map(|c| c.as_ref())
            .unwrap_or(TableCellRef::Empty)
    }

    fn modify_one(&mut self, coord: CellCoord, new_value: Variant) {
        self.state.one_shot_flags.cells_updated.push(coord);
        if new_value.is_empty() {
            self.state.cells.remove(&coord);
            return;
        }
        self.state.cells.entry(coord).and_modify(|cell| {
            if let TableCell::Available {
                value, is_dirty, ..
            } = cell
            {
                *value = new_value;
                *is_dirty = true;
            }
        });
        self.state.status = IoStatus::Edited;
    }

    fn create_one(&mut self, coord: CellCoord, value: Variant) {
        self.state.cells.insert(
            coord,
            TableCell::Available {
                value,
                is_dirty: false,
                in_conflict: false,
            },
        );
        self.state.status = IoStatus::Edited;
        self.state.one_shot_flags.cells_updated.push(coord);
    }

    fn create_row(&mut self, mut values: HashMap<u32, Variant>) -> Option<u32> {
        let row_uid = self.state.row_uid.len() as u32;
        self.state.row_uid.push(row_uid);
        for col_uid in 0..self.required_columns.len() as u32 {
            if let Some(value) = values.remove(&col_uid) {
                if value.is_empty() {
                    continue;
                }
                let coord = CellCoord(row_uid, col_uid);
                self.state.cells.insert(
                    coord,
                    TableCell::Available {
                        value,
                        is_dirty: false,
                        in_conflict: false,
                    },
                );
                self.state.one_shot_flags.cells_updated.push(coord);
            }
        }
        for (col_id, value) in values {
            if value.is_empty() {
                continue;
            }
            let coord = CellCoord(row_uid, col_id);
            self.state.cells.insert(
                coord,
                TableCell::Available {
                    value,
                    is_dirty: false,
                    in_conflict: false,
                },
            );
            self.state.one_shot_flags.cells_updated.push(coord);
        }
        self.state.status = IoStatus::Edited;
        self.state.one_shot_flags.row_set_updated = true;
        self.state.one_shot_flags.visible_row_vec_updated = true;
        Some(row_uid)
    }

    fn remove_rows(&mut self, row_ids: Vec<u32>) {
        self.state.row_uid.retain(|id| !row_ids.contains(id));
        self.state.cells.retain(|c, _| !row_ids.contains(&c.0));
        self.state.one_shot_flags.row_set_updated = true;
        self.state.one_shot_flags.visible_row_vec_updated = true;
        if self.state.cells.is_empty() {
            self.state.status = IoStatus::Empty;
        } else {
            self.state.status = IoStatus::Edited;
        }
    }

    fn clear(&mut self) {
        self.state.cells.clear();
        self.state.row_uid.clear();
        self.state.one_shot_flags.cleared = true;
        self.state.status = IoStatus::Empty;
    }

    fn clear_row_filters(&mut self) {}

    fn add_row_filter(&mut self, _filter: RowFilter, _additive: bool, _name: impl AsRef<str>) {
        todo!()
    }

    fn remove_row_filter(&mut self, _idx: usize) {
        todo!()
    }

    fn row_filters(&self) -> &[(RowFilter, String)] {
        &[]
    }
}
