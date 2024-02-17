#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SortBy {
    pub col_uid: u32,
    pub ascending: bool,
}
