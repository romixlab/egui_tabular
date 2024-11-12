use std::collections::HashMap;
use crate::backend::RowUid;

pub(super) struct State {
    pub(super) row_heights_a: HashMap<RowUid, f32>,
    pub(super) row_heights_b: HashMap<RowUid, f32>,
}

impl Default for State {
    fn default() -> Self {
        State {
            row_heights_a: HashMap::new(),
            row_heights_b: HashMap::new(),
        }
    }
}