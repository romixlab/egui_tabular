use crate::util::base_26;
use indexmap::IndexMap;
use rvariant::{Variant, VariantTy};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tabular_core::{
    Capabilities, CellCoord, CellLevel, CellMetadata, ColumnInfo, ColumnUid, ModelError, Revision,
    RowPosition, RowUid, TableModel,
};

/// In-memory table of `Variant` values. Show it with [`VariantCellUi`](crate::VariantCellUi).
pub struct VariantTable {
    cells: HashMap<CellCoord, Variant>,
    metadata: HashMap<CellCoord, VariantCellMetadata>,
    rows: Vec<RowUid>,
    skipped_rows: HashSet<RowUid>,
    next_row_uid: u64,
    columns: IndexMap<ColumnUid, Column>,
    revision: Revision,
    capabilities: Capabilities,
}

/// Definition of a [`VariantTable`] column.
#[derive(Clone, Debug)]
pub struct ColumnDef {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: VariantTy,
    /// Value of this column in new rows.
    pub default: Option<Variant>,
    pub is_required: bool,
    pub is_used: bool,
}

struct Column {
    info: ColumnInfo,
    ty: VariantTy,
    default: Option<Variant>,
}

#[derive(Default)]
struct VariantCellMetadata {
    conversion_fail_message: Option<Arc<String>>,
    common: CellMetadata,
}

impl ColumnDef {
    pub fn new(name: impl Into<String>, ty: VariantTy) -> Self {
        ColumnDef {
            name: name.into(),
            synonyms: vec![],
            ty,
            default: None,
            is_required: false,
            is_used: true,
        }
    }

    pub fn synonyms<S: Into<String>>(mut self, synonyms: impl IntoIterator<Item = S>) -> Self {
        self.synonyms = synonyms.into_iter().map(Into::into).collect();
        self
    }

    pub fn default(mut self, default: Variant) -> Self {
        self.default = Some(default);
        self
    }

    pub fn required(mut self, is_required: bool) -> Self {
        self.is_required = is_required;
        self
    }

    pub fn used(mut self, is_used: bool) -> Self {
        self.is_used = is_used;
        self
    }
}

impl VariantTable {
    /// A table with these columns (uids 0, 1, ...) and no rows. Editable.
    pub fn new(columns: impl IntoIterator<Item = ColumnDef>) -> Self {
        let mut table = VariantTable {
            cells: Default::default(),
            metadata: Default::default(),
            rows: vec![],
            skipped_rows: Default::default(),
            next_row_uid: 0,
            columns: Default::default(),
            revision: Revision::default(),
            capabilities: Capabilities::ALL,
        };
        for def in columns {
            table.insert_column(None, def);
        }
        table
    }

    /// Append a row. Columns without a value get their default.
    pub fn insert_row(&mut self, values: impl IntoIterator<Item = (ColumnUid, Variant)>) -> RowUid {
        self.insert_row_at(self.rows.len(), values)
    }

    fn insert_row_at(
        &mut self,
        idx: usize,
        values: impl IntoIterator<Item = (ColumnUid, Variant)>,
    ) -> RowUid {
        let row_uid = RowUid(self.next_row_uid);
        self.next_row_uid += 1;
        for (col_uid, value) in values {
            self.store(CellCoord { row_uid, col_uid }, value);
        }
        for (col_uid, col) in &self.columns {
            let coord = CellCoord {
                row_uid,
                col_uid: *col_uid,
            };
            if let Some(default) = &col.default
                && !self.cells.contains_key(&coord)
            {
                self.cells.insert(coord, default.clone());
            }
        }
        self.rows.insert(idx, row_uid);
        self.revision.rows += 1;
        row_uid
    }

    /// Add a column, with the given uid or the next free one.
    pub fn insert_column(&mut self, col_uid: Option<ColumnUid>, def: ColumnDef) -> ColumnUid {
        let col_uid = col_uid.unwrap_or_else(|| {
            ColumnUid(self.columns.keys().map(|uid| uid.0 + 1).max().unwrap_or(0))
        });
        let info = ColumnInfo::new(def.name, Some(def.ty.clone()))
            .synonyms(def.synonyms)
            .sortable(true)
            .required(def.is_required)
            .used(def.is_used);
        self.columns.insert(
            col_uid,
            Column {
                info,
                ty: def.ty,
                default: def.default,
            },
        );
        self.revision.columns += 1;
        col_uid
    }

    /// Remove all columns and all data.
    pub fn remove_all_columns(&mut self) {
        self.columns.clear();
        self.revision.columns += 1;
        self.clear_rows();
    }

    pub fn column_ty(&self, col_uid: ColumnUid) -> Option<VariantTy> {
        self.columns.get(&col_uid).map(|c| c.ty.clone())
    }

    /// Change a column's type and convert its values. Values that don't convert are kept as
    /// they are and marked with a warning.
    pub fn turn_column_into(&mut self, col_uid: ColumnUid, ty: VariantTy) {
        let Some(col) = self.columns.get_mut(&col_uid) else {
            return;
        };
        if col.ty == ty {
            return;
        }
        col.ty = ty.clone();
        col.info.ty = Some(ty);
        for row in self.rows.clone() {
            let coord = (row, col_uid).into();
            if let Some(value) = self.cells.remove(&coord) {
                self.store(coord, value);
            }
        }
        self.revision.columns += 1;
        self.revision.cells += 1;
    }

    pub fn set_metadata(&mut self, coord: CellCoord, meta: CellMetadata, merge: bool) {
        let m = self.metadata.entry(coord).or_default();
        if merge {
            m.common = m.common.clone().merge(meta);
        } else {
            m.common = meta;
        }
        self.revision.cells += 1;
    }

    pub fn clear_metadata(&mut self) {
        self.metadata.clear();
        self.revision.cells += 1;
    }

    /// Disallow (or allow again) all changes to the data. Skipping rows and columns stays allowed.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.capabilities = if read_only {
            Capabilities::ALL.read_only()
        } else {
            Capabilities::ALL
        };
    }

    /// Store a value converted to its column's type. A value that doesn't convert is stored as
    /// it is and marked with a warning.
    fn store(&mut self, coord: CellCoord, value: Variant) {
        let Some(ty) = self.columns.get(&coord.col_uid).map(|c| &c.ty) else {
            self.cells.insert(coord, value);
            return;
        };
        let (value, error) = convert(value, ty);
        match error {
            Some(error) => {
                self.metadata
                    .entry(coord)
                    .or_default()
                    .conversion_fail_message = Some(Arc::new(error));
            }
            None => {
                if let Some(m) = self.metadata.get_mut(&coord) {
                    m.conversion_fail_message = None;
                }
            }
        }
        self.cells.insert(coord, value);
    }

    fn clear_rows(&mut self) {
        self.cells.clear();
        self.metadata.clear();
        self.rows.clear();
        self.skipped_rows.clear();
        self.revision.rows += 1;
        self.revision.cells += 1;
    }

    fn row_index(&self, row: RowUid) -> Result<usize, ModelError> {
        self.rows
            .iter()
            .position(|r| *r == row)
            .ok_or(ModelError::NotFound)
    }

    fn check(&self, allowed: bool) -> Result<(), ModelError> {
        if allowed {
            Ok(())
        } else {
            Err(ModelError::Unsupported)
        }
    }
}

/// Convert `value` to `ty`. Text is parsed; blank text in a non-text column becomes `Empty`.
fn convert(value: Variant, ty: &VariantTy) -> (Variant, Option<String>) {
    if matches!(value, Variant::Empty) || VariantTy::from(&value) == *ty {
        return (value, None);
    }
    let converted = match &value {
        Variant::Str(s) if s.trim().is_empty() => Ok(Variant::Empty),
        Variant::Str(s) => Variant::try_from_str(s, ty),
        _ => value.clone().convert_to(ty),
    };
    match converted {
        Ok(converted) => (converted, None),
        Err(e) => (value, Some(format!("Not a valid {ty}: {e}"))),
    }
}

impl TableModel for VariantTable {
    fn revision(&self) -> Revision {
        self.revision
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    fn columns(&self) -> impl Iterator<Item = ColumnUid> {
        self.columns.keys().copied()
    }

    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo> {
        self.columns.get(&col).map(|c| &c.info)
    }

    fn rows(&self) -> impl Iterator<Item = RowUid> {
        self.rows.iter().copied()
    }

    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
        self.cells.get(&coord).map(Cow::Borrowed)
    }

    fn metadata(&self, coord: CellCoord) -> Option<Cow<'_, CellMetadata>> {
        let meta = self.metadata.get(&coord)?;
        Some(match &meta.conversion_fail_message {
            Some(msg) => Cow::Owned(
                CellMetadata::new()
                    .level(CellLevel::Warning)
                    .tooltip(msg.clone())
                    .merge(meta.common.clone()),
            ),
            None => Cow::Borrowed(&meta.common),
        })
    }

    fn is_row_skipped(&self, row: RowUid) -> bool {
        self.skipped_rows.contains(&row)
    }

    fn set(&mut self, coord: CellCoord, value: Variant) -> Result<(), ModelError> {
        self.check(self.capabilities.edit_cells)?;
        if !self.columns.contains_key(&coord.col_uid) {
            return Err(ModelError::NotFound);
        }
        self.store(coord, value);
        self.revision.cells += 1;
        Ok(())
    }

    fn create_row(
        &mut self,
        at: RowPosition,
        values: Vec<(ColumnUid, Variant)>,
    ) -> Result<RowUid, ModelError> {
        let idx = match at {
            RowPosition::Append => {
                self.check(self.capabilities.create_rows)?;
                self.rows.len()
            }
            RowPosition::Before(row) => {
                self.check(self.capabilities.insert_rows)?;
                self.row_index(row)?
            }
            RowPosition::After(row) => {
                self.check(self.capabilities.insert_rows)?;
                self.row_index(row)? + 1
            }
        };
        Ok(self.insert_row_at(idx, values))
    }

    fn remove_rows(&mut self, rows: &[RowUid]) -> Result<(), ModelError> {
        self.check(self.capabilities.remove_rows)?;
        let rows: HashSet<RowUid> = rows.iter().copied().collect();
        self.rows.retain(|r| !rows.contains(r));
        self.cells.retain(|c, _| !rows.contains(&c.row_uid));
        self.metadata.retain(|c, _| !rows.contains(&c.row_uid));
        self.skipped_rows.retain(|r| !rows.contains(r));
        self.revision.rows += 1;
        Ok(())
    }

    fn create_column(&mut self) -> Result<ColumnUid, ModelError> {
        self.check(self.capabilities.create_columns)?;
        let name = base_26(self.columns.len() as u32 + 1);
        Ok(self.insert_column(None, ColumnDef::new(name, VariantTy::Str)))
    }

    fn remove_columns(&mut self, cols: &[ColumnUid]) -> Result<(), ModelError> {
        self.check(self.capabilities.remove_columns)?;
        for col in cols {
            self.columns.shift_remove(col);
        }
        self.cells.retain(|c, _| !cols.contains(&c.col_uid));
        self.metadata.retain(|c, _| !cols.contains(&c.col_uid));
        self.revision.columns += 1;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), ModelError> {
        self.check(self.capabilities.clear)?;
        self.clear_rows();
        Ok(())
    }

    fn skip_rows(&mut self, rows: &[RowUid], skipped: bool) -> Result<(), ModelError> {
        self.check(self.capabilities.skip_rows)?;
        for row in rows {
            if skipped {
                self.skipped_rows.insert(*row);
            } else {
                self.skipped_rows.remove(row);
            }
        }
        self.revision.skips += 1;
        Ok(())
    }

    fn skip_column(&mut self, col: ColumnUid, skipped: bool) -> Result<(), ModelError> {
        self.check(self.capabilities.skip_columns)?;
        let col = self.columns.get_mut(&col).ok_or(ModelError::NotFound)?;
        col.info.is_skipped = skipped;
        self.revision.skips += 1;
        Ok(())
    }
}
