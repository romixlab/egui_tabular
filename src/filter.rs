use rvariant::Variant;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RowFilter {
    HideByUid(Vec<u32>),
    ShowByUid(Vec<u32>),
    ShowByVariant(VariantFilter),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VariantFilter {
    pub col_uid: u32,
    pub op: FilterOperation,
    pub value: Variant,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FilterOperation {
    Contains,
    Equals,
}
