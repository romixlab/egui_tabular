use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tabular_core::ColumnUid;

#[derive(Serialize, Deserialize)]
pub struct TableViewConfig {
    /// Row height will not be lower that this value.
    pub minimum_row_height: f32,
    /// Row height will be determined based on its contents.
    /// There might be some speed and memory penalty for doing this.
    pub use_heterogeneous_row_heights: bool,
    pub column_mapped_to: HashMap<ColumnUid, String>,
}

impl Default for TableViewConfig {
    fn default() -> Self {
        TableViewConfig {
            minimum_row_height: 15.0,
            use_heterogeneous_row_heights: true,
            column_mapped_to: Default::default(),
        }
    }
}
