use crate::{CellCoord, ColumnUid, RowUid};
use rvariant::Variant;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct VisualRowIdx(pub usize);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct VisualColIdx(pub usize);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
pub struct BackendColumn {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: String,
    pub is_sortable: bool,
    pub is_required: bool,
    pub is_used: bool,
    pub is_skipped: bool,
}

pub trait TableBackend {
    /// Drop all data from memory and start loading from scratch. No-op if memory based backend.
    fn reload(&mut self) {}
    // Fetch all remote data without waiting fot it to be queried
    // fn fetch_all(&mut self);
    // fn fetch(&mut self, col_uid_set: impl Iterator<Item = u32>);
    /// Clear all row data from memory, but leave the columns' info.
    /// If not supported, do nothing and return false from is_clearable().
    fn clear(&mut self);

    fn is_clearable(&self) -> bool {
        true
    }

    /// Send to server or write to disk all the changes made while commit_immediately was false.
    fn commit_all(&mut self) {}
    /// Whether to immediately send or write to disk all the changes as they are being made.
    fn commit_immediately(&mut self, enabled: bool) {
        let _ = enabled;
    }

    /// Returns flags that do not change from frame to frame.
    /// Recommended implementation: `&self.persistent_flags`
    fn persistent_flags(&self) -> &PersistentFlags;

    /// Returns one shot flags with 1 frame delay, so that user code gets a change to react to flag changes.
    /// Recommended implementation: `&self.one_shot_flags_delay`
    fn one_shot_flags(&self) -> &OneShotFlags;

    /// Returns one shot flags without delay, only to be used in TableView, cleared when show is called.
    /// Used by TableView.
    /// Recommended implementation: `&self.one_shot_flags`
    fn one_shot_flags_internal(&self) -> &OneShotFlags;

    /// Called in TableView::show() to copy current flags to the ones that will be returned via one_shot_flags()
    /// Recommended implementation: `self.one_shot_flags_delay = self.one_shot_flags.clone();`
    fn one_shot_flags_archive(&mut self);

    /// Must return the same OneShotFlags as on_shot_flags_internal, but as a mutable reference.
    /// Used by TableView.
    /// Recommended implementation: `&mut self.one_shot_flags`
    fn one_shot_flags_internal_mut(&mut self) -> &mut OneShotFlags;

    /// Process requests, talk to backend, watch for file changes, etc.
    /// Must be called periodically, for example each frame.
    /// Should not block or take too long on each run.
    fn poll(&mut self) {}

    /// Returns all available columns.
    fn available_columns(&self) -> impl Iterator<Item = ColumnUid>;
    /// Returns actually used columns, unused data is e.g. not sent over the network.
    fn used_columns(&self) -> impl Iterator<Item = ColumnUid> {
        self.available_columns()
    }
    fn column_info(&self, col_uid: ColumnUid) -> Option<&BackendColumn>;
    fn col_uid(&self, col_idx: VisualColIdx) -> Option<ColumnUid>;

    /// Choose whether to use a certain column or not.
    fn use_column(&mut self, col_uid: ColumnUid, is_used: bool) {
        let (_, _) = (col_uid, is_used);
    }
    // Choose whether to use certain columns or not.
    // fn use_columns(&mut self, cols: impl Iterator<Item = (usize, bool)>);

    /// Returns row count, with filters applied.
    fn row_count(&self) -> usize;
    /// Map index from [0..row_count) range to unique row id, applying sort order in the process.
    fn row_uid(&self, row_idx: VisualRowIdx) -> Option<RowUid>;
    /// Returns all row RowUid's.
    fn rows(&self) -> impl Iterator<Item = RowUid>;
    /// If backend supports row skipping, this function must return non-skipped RowUid's.
    fn un_skipped_rows(&self) -> impl Iterator<Item = RowUid> {
        self.rows()
    }

    /// Get value as Variant, not necessary to implement, but could be useful. CSV export works by calling get().
    fn get(&self, coord: CellCoord) -> Option<&Variant> {
        let _ = coord;
        None
    }

    /// Get value as Variant and try to convert to &str, not necessary to implement, but could be useful.
    fn get_as_str(&self, coord: CellCoord) -> Option<&str> {
        let value = self.get(coord)?;
        value.as_str().ok()
    }

    /// Get value as Variant and try to convert to &str, not necessary to implement, but could be useful.
    fn get_as_string(&self, coord: CellCoord) -> Option<String> {
        let value = self.get(coord)?;
        value.as_string().ok()
    }

    /// Set value as Variant, not necessary to implement, but could be useful.
    fn set(&mut self, coord: CellCoord, variant: Variant) {
        let (_, _) = (coord, variant);
    }

    fn commit_cell_edit(&mut self, coord: CellCoord) {
        let _ = coord;
    }
    // fn modify_one(&mut self, cell: CellCoord, new_value: Variant);
    // fn modify_many(&mut self, new_values: impl Iterator<Item = (CellCoord, Value)>, commit: bool);
    // fn remove_one(&mut self, cell: CellCoord, commit: bool);
    // fn create_one(&mut self, cell: CellCoord, value: Variant);
    // Create one row at the end and return it's uid if table is not read only
    // Use provided values, if no value is provided, Column's default will be used.
    // If there are not default value for Column, ui should show warning and do not allow committing.
    // If commit is tried anyway, it will be rejected.
    fn create_row(
        &mut self,
        values: impl IntoIterator<Item = (ColumnUid, Variant)>,
    ) -> Option<RowUid> {
        let _ = values;
        None
    }
    // fn remove_rows(&mut self, row_ids: Vec<u32>);
    /// Create new column if possible
    fn create_column(&mut self) -> Option<ColumnUid> {
        None
    }

    /// Called when a cell is selected/highlighted.
    fn on_highlight_cell(&mut self, coord: CellCoord) {
        let _ = coord;
    }

    // Removes all row filters
    // fn clear_row_filters(&mut self);
    // Hides some rows by their IDs
    // fn add_row_filter(&mut self, filter: RowFilter, additive: bool, name: impl AsRef<str>);
    // Remove one filter by its index and replay all remaining filters.
    // fn remove_row_filter(&mut self, idx: usize);
    // Get currently used row filters
    // fn row_filters(&self) -> &[(RowFilter, String)];

    fn column_mapping_choices(&self) -> &[String] {
        &[]
    }

    /// Mark row as disabled, VariantView will show all cells as strike-through
    fn skip_row(&mut self, row_uid: RowUid, skipped: bool) {
        let (_, _) = (row_uid, skipped);
    }
    fn un_skip_all_rows(&mut self) {}
    /// Return true if disable_row(uid, true) was previously called for this row
    fn is_row_skipped(&self, row_uid: RowUid) -> bool {
        let _ = row_uid;
        false
    }

    /// Mark col as disabled, VariantView will show all cells as strike-through
    fn skip_col(&mut self, col_uid: ColumnUid, skipped: bool) {
        let (_, _) = (col_uid, skipped);
    }
    fn un_skip_all_columns(&mut self) {}
    /// Return true if disable_col(uid, true) was previously called for this col
    fn is_col_skipped(&self, col_uid: ColumnUid) -> bool {
        let _ = col_uid;
        false
    }

    /// Set cell color or tooltip
    fn set_metadata(&mut self, coord: CellCoord, meta: CellMetadata, merge: bool) {
        let (_, _, _) = (coord, meta, merge);
    }
}

pub struct PersistentFlags {
    // Persistent flags: value is kept across poll() calls
    /// True until reload() is called while e.g. file was changed on disk and before reload() called.
    pub is_reload_recommended: bool,
    /// True until reload() is called if data was heavily modified on the backend.
    pub is_reload_required: bool,
    /// Whether table data is read only. Append row button will be inactive if this is true.
    pub is_read_only: bool,
    /// Whether table data can be cleared or not. TableView won't show clear button and won't call clear() if false.
    pub is_clearable: bool,
    /// True when column information is available.
    pub column_info_present: bool,
    /// True when full row uid set is available
    pub row_set_present: bool,
    /// True while awaiting cell's data
    pub cells_loading: bool,
    /// True while having all remote data locally cached. More can be added from the server, then this flags is cleared.
    pub have_all_cells: bool,
    /// True while locally made changes are not saved
    pub have_uncommitted_data: bool,
    /// True when locally modified cell was also updated remotely
    pub have_collisions: bool,
    /// Return true if skipping cols is supported / required
    pub are_cols_skippable: bool,
    /// Return true if skipping rows is supported / required
    pub are_rows_skippable: bool,
    /// Returns true if table cell's can be read as [Variant](Variant). For example, CSV export will work if this is true.
    pub is_get_variant_supported: bool,
}

impl Default for PersistentFlags {
    fn default() -> Self {
        Self {
            is_reload_recommended: false,
            is_reload_required: false,
            is_read_only: false,
            is_clearable: true,
            column_info_present: true,
            row_set_present: true,
            cells_loading: false,
            have_all_cells: true,
            have_uncommitted_data: false,
            have_collisions: false,
            are_cols_skippable: true,
            are_rows_skippable: true,
            is_get_variant_supported: false,
        }
    }
}

/// One shot flags: all flags are reset to false after poll() call
#[derive(Debug, Clone)]
pub struct OneShotFlags {
    /// Set once data backend is created
    pub first_pass: bool,
    /// Set once reload() is called or full load is initiated through other means
    pub reloaded: bool,
    /// Set once column names, types and default values was loaded
    pub columns_reset: bool,
    /// Set when one or more columns are changed (name / type)
    pub columns_changed: bool,
    /// Set once after row uid set was loaded or changed
    pub row_set_updated: bool,
    /// Set once when visible row set was changed (after filtering or sorting)
    pub visible_row_vec_updated: bool,
    // Set once when received updated for already available cell's
    // pub cells_updated: Vec<CellCoord>,
    /// Set once when clear() is called.
    pub cleared: bool,
    /// Set when different mapping is selected for a column
    pub column_mapping_changed: Option<ColumnUid>,
    /// Set when one or more rows where skipped or unskipped
    pub row_skip_set_changed: bool,
    /// Set when one or more cols where skipped or unskipped
    pub col_skip_set_changed: bool,
    pub rows_selected: Option<Vec<RowUid>>,
}

impl Default for OneShotFlags {
    fn default() -> Self {
        Self {
            columns_reset: true,
            row_set_updated: true,
            ..OneShotFlags::zero()
        }
    }
}

impl OneShotFlags {
    pub fn zero() -> Self {
        Self {
            first_pass: false,
            reloaded: false,
            columns_reset: false,
            columns_changed: false,
            row_set_updated: false,
            visible_row_vec_updated: false,
            cleared: false,
            column_mapping_changed: None,
            row_skip_set_changed: false,
            col_skip_set_changed: false,
            rows_selected: None,
        }
    }

    /// Returns true if any of the one shot flags is set to true, except first_pass.
    pub fn any_changed(&self) -> bool {
        self.reloaded
            || self.columns_reset
            || self.columns_changed
            || self.row_set_updated
            || self.visible_row_vec_updated
            || self.cleared
            || self.column_mapping_changed.is_some()
            || self.row_skip_set_changed
            || self.col_skip_set_changed
            || self.rows_selected.is_some()
    }
}

#[derive(Default)]
pub struct CellMetadata {
    pub color: Option<Rgb>,
    pub tooltip: Option<Arc<String>>,
}

#[derive(Copy, Clone)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl BackendColumn {
    pub fn new(name: impl AsRef<str>, ty: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            synonyms: vec![],
            ty: ty.as_ref().to_string(),
            is_sortable: false,
            is_required: false,
            is_used: false,
            is_skipped: false,
        }
    }
}

impl CellMetadata {
    pub fn color(rgb: Rgb) -> Self {
        Self {
            color: Some(rgb),
            tooltip: None,
        }
    }

    pub fn tooltip(tooltip: Arc<String>) -> Self {
        Self {
            color: None,
            tooltip: Some(tooltip),
        }
    }

    pub fn merge(&self, other: Self) -> Self {
        Self {
            color: self.color.or(other.color),
            tooltip: self.tooltip.clone().or(other.tooltip),
        }
    }
}

impl Rgb {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const LIGHT_GREEN: Self = Self::from_rgb(0x90, 0xEE, 0x90);
    pub const BLUE: Self = Self::from_rgb(0, 0, 255);

    pub const CYAN: Self = Self::from_rgb(0, 255, 255);
    pub const MAGENTA: Self = Self::from_rgb(255, 0, 255);
    pub const YELLOW: Self = Self::from_rgb(255, 255, 0);
    pub const ORANGE: Self = Self::from_rgb(255, 165, 0);
    pub const PURPLE: Self = Self::from_rgb(0x80, 0, 0x80);
    pub const GOLD: Self = Self::from_rgb(255, 215, 0);
}