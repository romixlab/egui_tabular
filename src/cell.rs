use rvariant::Variant;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellKind {
    /// Column is stored and processed by backend in a defined data structure.
    /// Renaming, changing type and removing is not permitted.
    Static(StaticCellKind),
    /// Type, name, description and other parameters are stored globally and can be reused.
    /// Global parameter's name and type can be changed through other tool, but not from TableViewer's.
    /// Can be removed.
    Global,
    /// Column exists only in the context of a particular table, even if it's name is the same.
    /// Name and type can be freely changed. Can be removed.
    Adhoc,
    // needed?
    // Calculated with an expression from other cells around
}

impl CellKind {
    pub const GLOBAL_DOC: &'static str = "Global parameter, can be used everywhere, same type is enforced\nand you can add short names and other metadata";
    pub const ADHOC_DOC: &'static str =
        "Adhoc parameter, can be removed or edited without any side-effects";
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StaticCellKind {
    /// Column's cells simply store data with a specified type.
    Plain,
    /// Changing cell value will cause some other cells to change as well.
    /// e.g. kind column that chooses what parameters are available or not.
    /// Must be soft-committed? in online mode even if local edit mode is enabled, in order to receive
    /// updated cells types / values.
    CausesSideEffects,
    /// Cells values are auto generated by backend, thus they are also read only.
    AutoGenerated,
}

impl StaticCellKind {
    pub const PLAIN_DOC: &'static str =
        "Parameter stored in a proper data structure and processed by the system";
    pub const CAUSES_SIDE_EFFECTS_DOC: &'static str =
        "Changing this parameter will cause others to recalculate\nor cause other side effects";
    pub const AUTO_GENERATED_DOC: &'static str =
        "This columns is not actually stored, but calculated from other local or global data";
}

// impl From<(Parameter, u32)> for Column {
//     fn from((value, col_uid): (Parameter, u32)) -> Self {
//         Column {
//             name: value.name,
//             ty: value.ty,
//             default: value.default,
//             kind: CellKind::Adhoc,
//             col_uid,
//         }
//     }
// }

// impl<'a> From<(&'a Parameter, u32)> for Column {
//     fn from((value, col_uid): (&Parameter, u32)) -> Self {
//         Column {
//             name: value.name.clone(),
//             ty: value.ty,
//             default: value.default.clone(),
//             kind: CellKind::Adhoc,
//             col_uid,
//         }
//     }
// }

/// row, col
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellCoord(pub u32, pub u32);

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct TableCellRef<'a> {
    pub value: &'a Variant,
    /// true if cell contains uncommitted data
    pub is_dirty: bool,
}

pub struct TableCell {
    pub value: Variant,
    /// true if cell contains uncommitted data
    pub is_dirty: bool,
    /// Cell was modified locally and then remotely as well
    pub in_conflict: bool,
}

impl TableCell {
    pub fn new(value: Variant) -> Self {
        TableCell {
            value,
            is_dirty: false,
            in_conflict: false,
        }
    }

    pub fn as_ref(&self) -> TableCellRef {
        TableCellRef {
            value: &self.value,
            is_dirty: self.is_dirty,
        }
    }
}

impl<'i> Display for TableCellRef<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}