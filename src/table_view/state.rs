use crate::backend::{ColumnUid, RowUid};
use std::collections::HashMap;

pub(super) struct State {
    pub(super) row_heights: HashMap<RowUid, f32>,
    pub(super) columns: Vec<ColumnUid>,
}

impl Default for State {
    fn default() -> Self {
        State {
            row_heights: HashMap::new(),
            columns: Vec::new(),
        }
    }
}
