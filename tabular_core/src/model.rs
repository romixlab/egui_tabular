//! The data side of a table: [`TableModel`] and the types it uses. No egui dependency.

use crate::{CellCoord, ColumnUid, RowUid};
use rvariant::{Variant, VariantTy};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// Change counters of a [`TableModel`].
///
/// The model bumps a counter on every change of that kind. Observers (any number of table views
/// and app code) keep their own last-seen copy and compare. Counters are never reset or consumed.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Revision {
    /// Column set, names, types and other [`ColumnInfo`] changes, except skipping.
    pub columns: u64,
    /// Row set: insert, remove, clear, reload.
    pub rows: u64,
    /// Any cell value.
    pub cells: u64,
    /// Row or column skip state. Separate, so that preprocessing that skips rows doesn't loop.
    pub skips: u64,
}

/// What a model supports. The view only issues commands that are allowed here.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Capabilities {
    pub edit_cells: bool,
    /// Append rows.
    pub create_rows: bool,
    /// Insert rows before or after another row, not only append.
    pub insert_rows: bool,
    pub remove_rows: bool,
    pub create_columns: bool,
    pub remove_columns: bool,
    pub clear: bool,
    pub skip_rows: bool,
    pub skip_columns: bool,
}

impl Capabilities {
    /// Read-only, no skipping.
    pub const NONE: Self = Capabilities {
        edit_cells: false,
        create_rows: false,
        insert_rows: false,
        remove_rows: false,
        create_columns: false,
        remove_columns: false,
        clear: false,
        skip_rows: false,
        skip_columns: false,
    };

    pub const ALL: Self = Capabilities {
        edit_cells: true,
        create_rows: true,
        insert_rows: true,
        remove_rows: true,
        create_columns: true,
        remove_columns: true,
        clear: true,
        skip_rows: true,
        skip_columns: true,
    };

    /// Without anything that changes data. Skipping stays as it is.
    pub fn read_only(self) -> Self {
        Capabilities {
            skip_rows: self.skip_rows,
            skip_columns: self.skip_columns,
            ..Self::NONE
        }
    }
}

/// Information about one column.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnInfo {
    pub name: String,
    /// Alternative names, used to map imported columns to this one.
    pub synonyms: Vec<String>,
    /// Value type. Picks the editor for empty cells, alignment and comparison.
    /// `None` for a custom column without values: not editable by the built-in editors.
    pub ty: Option<VariantTy>,
    /// Shown in the header instead of `ty`, e.g. "u8 (CAN DLC)".
    pub type_label: Option<String>,
    pub is_sortable: bool,
    pub is_required: bool,
    pub is_used: bool,
    pub is_skipped: bool,
}

impl ColumnInfo {
    pub fn new(name: impl Into<String>, ty: Option<VariantTy>) -> Self {
        ColumnInfo {
            name: name.into(),
            synonyms: vec![],
            ty,
            type_label: None,
            is_sortable: false,
            is_required: false,
            is_used: true,
            is_skipped: false,
        }
    }

    pub fn synonyms<S: Into<String>>(mut self, synonyms: impl IntoIterator<Item = S>) -> Self {
        self.synonyms = synonyms.into_iter().map(Into::into).collect();
        self
    }

    pub fn type_label(mut self, label: impl Into<String>) -> Self {
        self.type_label = Some(label.into());
        self
    }

    pub fn sortable(mut self, is_sortable: bool) -> Self {
        self.is_sortable = is_sortable;
        self
    }

    pub fn required(mut self, is_required: bool) -> Self {
        self.is_required = is_required;
        self
    }

    pub fn used(mut self, is_used: bool) -> Self {
        self.is_used = is_used;
        self
    }

    /// Text for the header: `type_label`, or the name of `ty`.
    pub fn type_text(&self) -> Option<Cow<'_, str>> {
        match (&self.type_label, &self.ty) {
            (Some(label), _) => Some(Cow::Borrowed(label.as_str())),
            (None, Some(ty)) => Some(Cow::Owned(ty.to_string())),
            (None, None) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModelError {
    /// The model doesn't support this operation (see [`Capabilities`]).
    Unsupported,
    /// The row or column doesn't exist.
    NotFound,
    TypeMismatch {
        expected: VariantTy,
    },
    Other(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::Unsupported => write!(f, "not supported by this table"),
            ModelError::NotFound => write!(f, "no such row or column"),
            ModelError::TypeMismatch { expected } => {
                write!(f, "expected a value of type {expected}")
            }
            ModelError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Where a new row goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RowPosition {
    Append,
    Before(RowUid),
    After(RowUid),
}

/// Table data: rows, columns and cell values. Presentation (order, selection, editing) belongs
/// to the view.
///
/// Rules:
/// - Bump the matching [`Revision`] counter on every change.
/// - A [`RowUid`] names the same row for its whole life and is never reused for another row.
///   Selection, scroll position and row heights are kept by uid.
/// - Values are optional: a model may render only through a custom cell UI and return `None`
///   from [`get`](Self::get). Copy, export and the built-in editors then have nothing to work with.
/// - Mutations return [`ModelError::Unsupported`] by default. The view only calls what
///   [`capabilities`](Self::capabilities) allows.
pub trait TableModel {
    fn revision(&self) -> Revision;
    fn capabilities(&self) -> Capabilities;

    /// Natural column order. The view starts from this and applies the user's order on top.
    fn columns(&self) -> impl Iterator<Item = ColumnUid>;
    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo>;

    /// Natural row order (insertion or file order).
    fn rows(&self) -> impl Iterator<Item = RowUid>;
    /// Override when cheaper than counting `rows()`.
    fn row_count(&self) -> usize {
        self.rows().count()
    }
    fn un_skipped_rows(&self) -> impl Iterator<Item = RowUid> {
        self.rows().filter(|row| !self.is_row_skipped(*row))
    }

    /// Cell value. `Cow`, so that computed models can return owned values and stored models
    /// references.
    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
        let _ = coord;
        None
    }
    /// Values of several cells of one row, appended to `out` in the order of `cols`. Override
    /// when finding the row is costly, so that copy and export look it up once.
    fn row_values(&self, row: RowUid, cols: &[ColumnUid], out: &mut Vec<Option<Variant>>) {
        out.extend(cols.iter().map(|col| {
            self.get(CellCoord {
                row_uid: row,
                col_uid: *col,
            })
            .map(Cow::into_owned)
        }));
    }
    /// Presentation hints for a cell, drawn by the view.
    fn metadata(&self, coord: CellCoord) -> Option<Cow<'_, CellMetadata>> {
        let _ = coord;
        None
    }
    fn is_row_skipped(&self, row: RowUid) -> bool {
        let _ = row;
        false
    }

    fn set(&mut self, coord: CellCoord, value: Variant) -> Result<(), ModelError> {
        let _ = (coord, value);
        Err(ModelError::Unsupported)
    }
    /// Create a row. Columns without a value in `values` get their default, if any.
    fn create_row(
        &mut self,
        at: RowPosition,
        values: Vec<(ColumnUid, Variant)>,
    ) -> Result<RowUid, ModelError> {
        let _ = (at, values);
        Err(ModelError::Unsupported)
    }
    fn remove_rows(&mut self, rows: &[RowUid]) -> Result<(), ModelError> {
        let _ = rows;
        Err(ModelError::Unsupported)
    }
    fn create_column(&mut self) -> Result<ColumnUid, ModelError> {
        Err(ModelError::Unsupported)
    }
    fn remove_columns(&mut self, cols: &[ColumnUid]) -> Result<(), ModelError> {
        let _ = cols;
        Err(ModelError::Unsupported)
    }
    /// Remove all rows, keep the columns.
    fn clear(&mut self) -> Result<(), ModelError> {
        Err(ModelError::Unsupported)
    }
    fn skip_rows(&mut self, rows: &[RowUid], skipped: bool) -> Result<(), ModelError> {
        let _ = (rows, skipped);
        Err(ModelError::Unsupported)
    }
    fn skip_column(&mut self, col: ColumnUid, skipped: bool) -> Result<(), ModelError> {
        let _ = (col, skipped);
        Err(ModelError::Unsupported)
    }
}

/// Presentation hints for a cell: background, corner marker, tooltips, text wrapping.
#[derive(Default, Clone, Debug)]
#[non_exhaustive]
pub struct CellMetadata {
    /// Tints the cell background.
    pub level: Option<CellLevel>,
    /// Small triangle in the top right corner.
    pub corner: Option<CellLevel>,
    pub tooltips: Vec<Arc<String>>,
    pub wrap_mode: Option<WrapMode>,
}

/// Meaning of a cell highlight. The view maps it to colors of the current theme.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CellLevel {
    Info,
    Warning,
    Error,
    /// The value changed recently or differs from a reference.
    Changed,
    /// A fixed color; prefer the semantic levels, which work in light and dark mode.
    Custom(Rgb),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl CellMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(self, level: CellLevel) -> Self {
        Self {
            level: Some(level),
            ..self
        }
    }

    pub fn level_opt(self, level: Option<CellLevel>) -> Self {
        Self { level, ..self }
    }

    pub fn corner(self, corner: CellLevel) -> Self {
        Self {
            corner: Some(corner),
            ..self
        }
    }

    pub fn corner_opt(self, corner: Option<CellLevel>) -> Self {
        Self { corner, ..self }
    }

    pub fn tooltip(mut self, tooltip: Arc<String>) -> Self {
        self.tooltips.push(tooltip);
        self
    }

    pub fn tooltip_opt(mut self, tooltip: Option<Arc<String>>) -> Self {
        self.tooltips.extend(tooltip);
        self
    }

    pub fn wrap_mode(self, wrap_mode: WrapMode) -> Self {
        Self {
            wrap_mode: Some(wrap_mode),
            ..self
        }
    }

    /// Fields of `self` win; tooltips of both are kept.
    pub fn merge(self, other: Self) -> Self {
        Self {
            level: self.level.or(other.level),
            corner: self.corner.or(other.corner),
            tooltips: self.tooltips.into_iter().chain(other.tooltips).collect(),
            wrap_mode: self.wrap_mode.or(other.wrap_mode),
        }
    }
}

impl Rgb {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const LIGHT_GREEN: Self = Self::from_rgb(0x90, 0xEE, 0x90);
    pub const BLUE: Self = Self::from_rgb(0, 0, 255);

    pub const CYAN: Self = Self::from_rgb(0, 255, 255);
    pub const MAGENTA: Self = Self::from_rgb(255, 0, 255);
    pub const YELLOW: Self = Self::from_rgb(255, 255, 0);
    pub const ORANGE: Self = Self::from_rgb(255, 165, 0);
    pub const PURPLE: Self = Self::from_rgb(0x80, 0, 0x80);
    pub const GOLD: Self = Self::from_rgb(255, 215, 0);
}

/// How to wrap and elide text (other ui elements might be supported in the future as well).
///
/// Same as egui::TextWrapMode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrapMode {
    /// The text should expand the `Ui` size when reaching its boundary.
    Extend,

    /// The text should wrap to the next line when reaching the `Ui` boundary.
    Wrap,

    /// The text should be elided using "…" when reaching the `Ui` boundary.
    Truncate,
}
