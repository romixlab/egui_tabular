use crate::backend::RowUid;
use std::collections::HashMap;

pub(super) struct State {
    pub(super) row_heights: HashMap<RowUid, f32>,
}

impl Default for State {
    fn default() -> Self {
        State {
            row_heights: HashMap::new(),
        }
    }
}
