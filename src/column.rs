use std::collections::HashMap;

use rvariant::{Variant, VariantTy};

use crate::cell::CellKind;

/// Column type used by backends to communicate available columns.
/// Created from RequiredColumn's if provided or with adhoc CellKind and Str type if not.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackendColumn {
    pub name: String,
    pub ty: VariantTy,
    pub default_value: Option<Variant>,
    pub kind: CellKind,
}

/// Required column that must be present
/// When an array of required columns is provided to a backend, it ensures that columns indices match.
/// With that assumption in mind, user code could rely on this order when e.g. importing a CSV file.
#[derive(Clone)]
pub struct TableColumn {
    /// Displayed column name as provided
    pub name: String,
    /// Alternative column names as provided, displayed when hovering over column's name
    pub synonyms: Vec<String>,
    /// Lower case name and synonyms used to match with data backend columns
    match_synonyms: Vec<String>,
    /// Required column type, conversion will be attempted for imported or entered values
    pub ty: VariantTy,
    /// Allow changing column type, converting all the values in the process (not implemeted)
    pub ty_locked: bool,
    /// Default value for newly created cells (when adding rows)
    pub default_value: Option<Variant>,
    /// Whether this column is required or not, Add button will be displayed if required.
    pub is_required: bool,
}

impl TableColumn {
    pub fn new(
        name: impl AsRef<str>,
        synonyms: impl IntoIterator<Item = &'static str>,
        ty: VariantTy,
        is_required: bool,
        ty_locked: bool,
        default_value: Option<Variant>,
    ) -> Self {
        TableColumn {
            name: name.as_ref().to_string(),
            synonyms: vec![],
            match_synonyms: vec![],
            ty,
            ty_locked,
            default_value,
            is_required,
        }
        .synonyms(synonyms)
    }

    pub fn required(name: impl AsRef<str>, ty: VariantTy, default: Option<Variant>) -> Self {
        Self::new(name, [], ty, true, true, default)
    }

    pub fn optional(name: impl AsRef<str>, ty: VariantTy, default: Option<Variant>) -> Self {
        Self::new(name, [], ty, false, true, default)
    }

    pub fn required_str(name: impl AsRef<str>, default: Option<String>) -> Self {
        Self::new(
            name,
            [],
            VariantTy::Str,
            true,
            true,
            default.map(|s| Variant::Str(s)),
        )
    }

    pub fn required_u32(name: impl AsRef<str>, default: Option<u32>) -> Self {
        Self::new(
            name,
            [],
            VariantTy::U32,
            true,
            true,
            default.map(|x| Variant::U32(x)),
        )
    }

    pub fn optional_str(name: impl AsRef<str>, default: Option<String>) -> Self {
        Self::new(
            name,
            [],
            VariantTy::Str,
            false,
            true,
            default.map(|s| Variant::Str(s)),
        )
    }

    pub fn optional_u32(name: impl AsRef<str>, default: Option<u32>) -> Self {
        Self::new(
            name,
            [],
            VariantTy::U32,
            false,
            true,
            default.map(|x| Variant::U32(x)),
        )
    }

    pub fn synonyms(mut self, synonyms: impl IntoIterator<Item = &'static str>) -> Self {
        let mut match_synonyms = vec![self.name.to_lowercase(), self.name.replace(' ', "")];
        let mut synonyms_owned = vec![];
        for s in synonyms.into_iter() {
            synonyms_owned.push(s.to_string());
            match_synonyms.push(s.to_lowercase());
        }

        self.match_synonyms = match_synonyms;
        self.synonyms = synonyms_owned;
        self
    }

    pub fn ty_locked(mut self, ty_locked: bool) -> Self {
        self.ty_locked = ty_locked;
        self
    }

    /// Find an index of a matching column in the provided array of strings
    pub fn find_match_arr(&self, names: &[&str]) -> Option<usize> {
        for (idx, name) in names.iter().map(|n| n.to_lowercase()).enumerate() {
            for synonym in &self.match_synonyms {
                if &name == synonym {
                    return Some(idx);
                }
            }
        }
        None
    }

    pub fn find_match_map(&self, columns: &HashMap<u32, BackendColumn>) -> Option<u32> {
        for (data_col_id, column) in columns {
            let name = column.name.to_lowercase();
            if self.match_synonyms.contains(&name) {
                return Some(*data_col_id);
            }
        }
        None
    }
}
