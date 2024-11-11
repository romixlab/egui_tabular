use egui::Ui;
use egui_extras::Column as TableColumnConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
// #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
pub struct CellCoord {
    pub row: u32,
    pub col_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
pub struct BackendColumn {
    pub col_id: u32,
    pub name: String,
    pub ty: String,
    pub is_sortable: bool,
}

pub trait TableBackend {
    /// Drop all data and start loading from scratch.
    fn reload(&mut self) {}
    // Fetch all remote data without waiting fot it to be queried
    // fn fetch_all(&mut self);
    // fn fetch(&mut self, col_uid_set: impl Iterator<Item = u32>);
    fn clear(&mut self);

    /// Send to server or write to disk all the changes made while commit_immediately was false.
    fn commit_all(&mut self) {}
    /// Whether to immediately send or write to disk all the changes as they are being made.
    fn commit_immediately(&mut self, enabled: bool) {
        let _ = enabled;
    }

    fn persistent_flags(&self) -> &PersistentFlags;
    fn one_shot_flags(&self) -> &OneShotFlags;
    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags;

    /// Process requests, talk to backend, watch for file changes, etc.
    /// Must be called periodically, for example each frame.
    fn poll(&mut self) {}

    /// Returns all available columns. Columns contain unique identifiers.
    fn available_columns(&self) -> &[BackendColumn];
    /// Returns actually used columns, unused data is e.g. not sent over the network.
    fn used_columns(&self) -> &[BackendColumn];

    /// Choose whether to use a certain column or not.
    fn use_column(&mut self, col_id: u32, is_used: bool) {
        let (_, _) = (col_id, is_used);
    }
    // Choose whether to use certain columns or not.
    // fn use_columns(&mut self, cols: impl Iterator<Item = (usize, bool)>);
    fn is_sortable_column(&self, col_id: u32) -> bool {
        false
    }

    /// Returns the rendering configuration for the column.
    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        let _ = column;
        TableColumnConfig::auto().resizable(true)
    }

    /// Returns visible row count, with filters applied.
    fn visible_row_count(&self) -> usize;
    // Get unique IDs of all rows
    // fn row_uid_set(&self) -> Vec<u32>;
    // Map index from 0..row_count() range to external data source row id
    // fn row_uid(&self, monotonic_idx: u32) -> Option<u32>;
    // Map unique row id back into monotonic index, if it is in the current view.
    // Can be used to jump to another row.
    // fn row_monotonic(&self, row_uid: u32) -> Option<u32>;

    fn show_cell_view(&self, row_mono: usize, col_uid: u32, ui: &mut Ui);
    fn show_cell_editor(&mut self, cell: CellCoord, ui: &mut Ui) -> Option<egui::Response>;
    // fn modify_one(&mut self, cell: CellCoord, new_value: Variant);
    // fn modify_many(&mut self, new_values: impl Iterator<Item = (CellCoord, Value)>, commit: bool);
    // fn remove_one(&mut self, cell: CellCoord, commit: bool);
    // fn create_one(&mut self, cell: CellCoord, value: Variant);
    // Create one row at the end and return it's uid if table is not read only
    // Use provided values, if no value is provided, Column's default will be used.
    // If there are not default value for Column, ui should show warning and do not allow committing.
    // If commit is tried anyway, it will be rejected.
    // fn create_row(&mut self, values: HashMap<u32, Variant>) -> Option<u32>;
    // fn remove_rows(&mut self, row_ids: Vec<u32>);

    /// Use this to check if given cell is going to take any dropped payload / use as drag
    /// source.
    fn on_cell_view_response(&mut self, cell: CellCoord, resp: &egui::Response) -> Option<()> {
        let _ = (cell, resp);
        None
    }

    /// Called when a cell is selected/highlighted.
    fn on_highlight_cell(&mut self, cell: CellCoord) {
        let _ = cell;
    }

    // Removes all row filters
    // fn clear_row_filters(&mut self);
    // Hides some rows by their IDs
    // fn add_row_filter(&mut self, filter: RowFilter, additive: bool, name: impl AsRef<str>);
    // Remove one filter by its index and replay all remaining filters.
    // fn remove_row_filter(&mut self, idx: usize);
    // Get currently used row filters
    // fn row_filters(&self) -> &[(RowFilter, String)];
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
    /// Set once data backend is created
    pub first_pass: bool,
    /// Set once reload() is called or full load is initiated through other means
    pub reloaded: bool,
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
