use serde::{Deserialize, Serialize};

pub mod backend;

pub use rvariant::{Variant, VariantTy};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ColumnUid(pub u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RowUid(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct CellCoord {
    pub row_uid: RowUid,
    pub col_uid: ColumnUid,
}

impl From<(RowUid, ColumnUid)> for CellCoord {
    fn from(value: (RowUid, ColumnUid)) -> Self {
        CellCoord {
            row_uid: value.0,
            col_uid: value.1,
        }
    }
}

impl From<(RowUid, &ColumnUid)> for CellCoord {
    fn from(value: (RowUid, &ColumnUid)) -> Self {
        CellCoord {
            row_uid: value.0,
            col_uid: *value.1,
        }
    }
}

#[derive(
    strum::EnumIter, strum::Display, PartialEq, Copy, Clone, Default, Serialize, Deserialize,
)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Separator {
    #[default]
    Auto,
    Comma,
    Tab,
    Semicolon,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct CsvImporterConfig {
    pub separator: Separator,
    pub separator_u8: u8,
    pub skip_first_rows: usize,
    pub has_headers: bool,
}

impl Default for CsvImporterConfig {
    fn default() -> Self {
        CsvImporterConfig {
            separator: Default::default(),
            separator_u8: b',',
            skip_first_rows: 0,
            has_headers: true,
        }
    }
}

impl CsvImporterConfig {
    pub fn separator(&self) -> u8 {
        self.separator_u8
    }

    pub fn skip_first_rows(&self) -> usize {
        self.skip_first_rows
    }

    pub fn has_headers(&self) -> bool {
        self.has_headers
    }
}
