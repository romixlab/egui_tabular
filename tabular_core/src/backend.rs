use crate::{CellCoord, ColumnUid, RowUid};
use rvariant::Variant;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct VisualRowIdx(pub usize);

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
    fn clear(&mut self);

    /// Send to server or write to disk all the changes made while commit_immediately was false.
    fn commit_all(&mut self) {}
    /// Whether to immediately send or write to disk all the changes as they are being made.
    fn commit_immediately(&mut self, enabled: bool) {
        let _ = enabled;
    }

    fn persistent_flags(&self) -> &PersistentFlags;
    /// Returns one shot flags with 1 frame delay, so that user code gets a change to react to flag changes.
    fn one_shot_flags(&self) -> &OneShotFlags;
    /// Returns one shot flags without delay, only to be used in TableView, cleared when show is called.
    fn one_shot_flags_internal(&self) -> &OneShotFlags;
    /// Called in TableView::show() to copy current flags to the ones that will be returned via one_shot_flags()
    fn one_shot_flags_archive(&mut self);
    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags;

    /// Process requests, talk to backend, watch for file changes, etc.
    /// Must be called periodically, for example each frame.
    /// Should not block or take too long on each run.
    fn poll(&mut self) {}

    /// Returns all available columns.
    fn available_columns(&self) -> impl Iterator<Item = ColumnUid>;
    /// Returns actually used columns, unused data is e.g. not sent over the network.
    fn used_columns(&self) -> impl Iterator<Item = ColumnUid>;
    fn column_info(&self, col_uid: ColumnUid) -> Option<&BackendColumn>;

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

    /// Get value as Variant, not necessary to implement, but useful if using TableBackend without UI.
    fn get(&self, _coord: CellCoord) -> Option<&Variant> {
        None
    }

    /// Set value as Variant, not necessary to implement, but useful if using TableBackend without UI.
    fn set(&mut self, _coord: CellCoord, _variant: Variant) {}

    fn commit_cell_edit(&mut self, coord: CellCoord);
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
        _values: impl IntoIterator<Item = (ColumnUid, Variant)>,
    ) -> Option<RowUid> {
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

    /// Return true if skipping rows is supported / required
    fn are_rows_skippable(&self) -> bool {
        false
    }
    /// Mark row as disabled, VariantView will show all cells as strike-through
    fn skip_row(&mut self, row_uid: RowUid, skipped: bool) {
        let _ = row_uid;
        let _ = skipped;
    }
    /// Return true if disable_row(uid, true) was previously called for this row
    fn is_row_skipped(&self, row_uid: RowUid) -> bool {
        let _ = row_uid;
        false
    }

    /// Return true if skipping cols is supported / required
    fn are_cols_skippable(&self) -> bool {
        false
    }
    /// Mark col as disabled, VariantView will show all cells as strike-through
    fn skip_col(&mut self, col_uid: ColumnUid, skipped: bool) {
        let _ = col_uid;
        let _ = skipped;
    }
    /// Return true if disable_col(uid, true) was previously called for this col
    fn is_col_skipped(&self, col_uid: ColumnUid) -> bool {
        let _ = col_uid;
        false
    }
}

#[derive(Default)]
pub struct PersistentFlags {
    // Persistent flags: value is kept across poll() calls
    /// True until reload() is called while e.g. file was changed on disk and before reload() called.
    pub is_reload_recommended: bool,
    /// True until reload() is called if data was heavily modified on the backend.
    pub is_reload_required: bool,
    /// True when data should not be modified
    pub is_read_only: bool,
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
}

/// One shot flags: all flags are reset to false after poll() call
#[derive(Default, Copy, Clone)]
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
}
