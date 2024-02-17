use std::collections::HashMap;

use crate::cell::{CellCoord, TableCellRef};
use crate::column::Column;
use crate::filter::RowFilter;
use rvariant::Variant;

pub trait TableBackend {
    /// Drop all data and start loading from scratch.
    fn reload(&mut self);
    /// Fetch all remote data without waiting fot it to be queried
    fn fetch_all(&mut self);
    fn fetch(&mut self, col_uid_set: impl Iterator<Item = u32>);

    /// Send to server or write to disk all the changes made while commit_immediately was false.
    fn commit_all(&mut self);
    /// Whether to immediately send or write to disk all the changes as they are being made.
    fn commit_immediately(&mut self, enabled: bool);

    fn persistent_flags(&self) -> &PersistentFlags;
    fn one_shot_flags(&self) -> &OneShotFlags;
    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags;

    /// Process requests, talk to backend, watch for file changes, etc.
    fn poll(&mut self);

    /// Returns all available columns. Columns use unique identifiers. Some are global, some context dependent.
    fn available_columns(&self) -> &HashMap<u32, Column>;
    /// Returns actually used columns, unused data is e.g. not sent over the network.
    fn used_columns(&self) -> &HashMap<u32, Column>;
    /// Choose whether to use a certain column or not.
    fn use_column(&mut self, col_uid: usize, is_used: bool);
    // Choose whether to use certain columns or not.
    // fn use_columns(&mut self, cols: impl Iterator<Item = (usize, bool)>);

    /// Returns total row count.
    fn row_count(&self) -> usize;
    /// Get unique IDs of all rows
    fn row_uid_set(&self) -> Vec<u32>;
    /// Map index from 0..row_count() range to external data source row id
    fn row_uid(&self, monotonic_idx: usize) -> Option<u32>;
    /// Map unique row id back into monotonic index, if it is in the current view.
    /// Can be used to jump to another row.
    fn row_monotonic(&self, row_uid: u32) -> Option<usize>;

    /// Get cell value if available, remember to load it otherwise.
    /// Remember to map monotonic indices to uid through row_uid() method.
    /// Columns are also uid's, can directly use what's in the available_columns() hashmap
    fn cell(&mut self, cell: CellCoord) -> Option<TableCellRef>;
    fn modify_one(&mut self, cell: CellCoord, new_value: Variant);
    // fn modify_many(&mut self, new_values: impl Iterator<Item = (CellCoord, Value)>, commit: bool);
    // fn remove_one(&mut self, cell: CellCoord, commit: bool);
    fn create_one(&mut self, cell: CellCoord, value: Variant);
    /// Create one row at the end and return it's uid if table is not read only
    /// Use provided values, if no value is provided, Column's default will be used.
    /// If there are not default value for Column, ui should show warning and do not allow committing.
    /// If commit is tried anyway, it will be rejected.
    fn create_row(&mut self, values: HashMap<u32, Variant>) -> Option<u32>;
    fn remove_rows(&mut self, row_ids: Vec<u32>);
    fn clear(&mut self);

    /// Removes all row filters
    fn clear_row_filters(&mut self);
    /// Hides some rows by their IDs
    fn add_row_filter(&mut self, filter: RowFilter, additive: bool, name: impl AsRef<str>);
    /// Remove one filter by its index and replay all remaining filters.
    fn remove_row_filter(&mut self, idx: usize);
    /// Get currently used row filters
    fn row_filters(&self) -> &[(RowFilter, String)];
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
#[derive(Default)]
pub struct OneShotFlags {
    /// Set once reload() is called.
    pub reload_started: bool,
    /// Set once column names, types and default values was loaded
    pub column_info_updated: bool,
    /// Set once after row uid set was loaded or changed
    pub row_set_updated: bool,
    /// Set once when visible row set was changed (after filtering or sorting)
    pub visible_row_vec_updated: bool,
    /// Set once when received updated for already available cell's
    pub cells_updated: Vec<CellCoord>,
    /// Set once when clear() is called.
    pub cleared: bool,
}
