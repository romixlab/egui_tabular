use serde::{Deserialize, Serialize};

pub mod backend;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ColumnUid(pub u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RowUid(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct CellCoord {
    pub row_uid: RowUid,
    pub col_uid: ColumnUid,
}
