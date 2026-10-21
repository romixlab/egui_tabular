use crate::row_order::RowFilter;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tabular_core::{ColumnUid, SortKey};

/// User preferences and the view state worth keeping between runs: column order, widths and
/// visibility, sort and filter. Serde; owned and persisted by the app. The view reads it every
/// frame, so the app may also change it (e.g. set a sort from its own toolbar), and the view
/// reports its own changes with [`TableEvent::ConfigChanged`](crate::TableEvent::ConfigChanged).
///
/// Columns are keyed by [`ColumnUid`]: persist it only for models whose column uids are the same
/// in every run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableViewConfig {
    /// Row height will not be lower that this value.
    pub minimum_row_height: f32,
    /// Row height will be determined based on its contents.
    /// There might be some speed and memory penalty for doing this.
    pub use_heterogeneous_row_heights: bool,
    pub column_mapped_to: HashMap<ColumnUid, String>,
    /// Column order chosen by the user. Columns the model doesn't have (yet) keep their place
    /// here; model columns that aren't listed follow in the model's order.
    pub column_order: Vec<ColumnUid>,
    /// Widths (including padding) of columns the user resized. Columns not listed size to fit.
    pub column_widths: BTreeMap<ColumnUid, f32>,
    /// Columns not shown.
    pub hidden_columns: BTreeSet<ColumnUid>,
    /// Sort keys, the first one is the primary key. Empty: the model's order.
    pub sort: Vec<SortKey>,
    /// Which rows are shown.
    pub filter: RowFilter,
}

impl Default for TableViewConfig {
    fn default() -> Self {
        TableViewConfig {
            minimum_row_height: 15.0,
            use_heterogeneous_row_heights: true,
            column_mapped_to: Default::default(),
            column_order: vec![],
            column_widths: Default::default(),
            hidden_columns: Default::default(),
            sort: vec![],
            filter: RowFilter::default(),
        }
    }
}
