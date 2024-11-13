use crate::backend::ColumnUid;
use rvariant::{Variant, VariantTy};

pub struct RequiredColumn {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: VariantTy,
    pub default: Option<Variant>,
}

pub struct RequiredColumns {
    pub(crate) required_columns: Vec<(ColumnUid, RequiredColumn)>,
}

impl RequiredColumn {
    pub fn new(name: impl AsRef<str>, ty: VariantTy) -> Self {
        RequiredColumn {
            name: name.as_ref().to_string(),
            synonyms: vec![],
            ty,
            default: None,
        }
    }

    pub fn str(name: impl AsRef<str>) -> Self {
        RequiredColumn {
            name: name.as_ref().to_string(),
            synonyms: vec![],
            ty: VariantTy::Str,
            default: None,
        }
    }

    pub fn u32(name: impl AsRef<str>) -> Self {
        RequiredColumn {
            name: name.as_ref().to_string(),
            synonyms: vec![],
            ty: VariantTy::U32,
            default: None,
        }
    }

    pub fn synonyms<'a>(self, synonyms: impl IntoIterator<Item = &'a str>) -> Self {
        RequiredColumn {
            name: self.name,
            synonyms: synonyms.into_iter().map(|s| s.to_lowercase()).collect(),
            ty: self.ty,
            default: self.default,
        }
    }

    pub fn default(self, default: Variant) -> Self {
        RequiredColumn {
            name: self.name,
            synonyms: self.synonyms,
            ty: self.ty,
            default: Some(default),
        }
    }

    fn contains_in_synonyms(&self, name: &str) -> bool {
        self.synonyms.iter().find(|s| s.as_str() == name).is_some()
    }
}

impl RequiredColumns {
    pub fn new(required_columns: impl IntoIterator<Item = RequiredColumn>) -> Self {
        RequiredColumns {
            required_columns: required_columns
                .into_iter()
                .enumerate()
                .map(|(idx, col)| (ColumnUid(idx as u32), col))
                .collect(),
        }
    }

    pub fn map_columns(
        &self,
        column_names: &[&str],
    ) -> Vec<((ColumnUid, &RequiredColumn), Option<usize>)> {
        let mut map = vec![];
        for (col_uid, col) in &self.required_columns {
            let col_name_lower = col.name.to_lowercase();
            if let Some(idx) = column_names
                .iter()
                .enumerate()
                .find(|(_, n)| **n == col_name_lower.as_str() || col.contains_in_synonyms(**n))
                .map(|(idx, _)| idx)
            {
                map.push(((*col_uid, col), Some(idx)));
            } else {
                map.push(((*col_uid, col), None));
            }
        }
        map
    }

    pub fn get(&self, col_uid: ColumnUid) -> Option<&RequiredColumn> {
        self.required_columns
            .iter()
            .find(|(uid, _)| *uid == col_uid)
            .map(|(_, c)| c)
    }
}
