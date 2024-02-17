use rvariant::{Variant, VariantTy};

use crate::cell::CellKind;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Column {
    pub name: String,
    pub ty: VariantTy,
    pub default: Option<Variant>,
    pub kind: CellKind,
    pub col_uid: u32,
}

#[derive(Clone)]
pub struct RequiredColumn {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: VariantTy,
}

impl RequiredColumn {
    pub fn new<S: AsRef<str>>(
        name: S,
        synonyms: impl IntoIterator<Item = S>,
        ty: VariantTy,
    ) -> Self {
        RequiredColumn {
            name: name.as_ref().to_string(),
            synonyms: synonyms
                .into_iter()
                .map(|s| s.as_ref().to_lowercase())
                .collect(),
            ty,
        }
    }

    pub fn find_match(&self, names: &[&str]) -> Option<usize> {
        for (idx, name) in names.iter().map(|n| n.to_lowercase()).enumerate() {
            if name == self.name.to_lowercase() {
                return Some(idx);
            }
            for synonym in &self.synonyms {
                if &name == synonym {
                    return Some(idx);
                }
            }
        }
        None
    }
}
