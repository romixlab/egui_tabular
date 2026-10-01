pub mod backends;
pub mod cell_ui;
pub mod commands;
pub mod importers;
pub mod table_view;
mod util;

pub use importers::required_column::{RequiredColumn, RequiredColumns};
pub use importers::tabular_importer::TabularImporter;

pub use backends::variant::{ColumnDef, VariantTable};
pub use cell_ui::{CellUi, EditorResponse, RowCells, VariantCellUi};
pub use commands::TableCommand;
pub use table_view::{TableEvent, TableView, TableViewConfig, TableViewOptions, TableViewOutput};

pub use egui;
pub use rvariant;
pub use tabular_core;
pub use tabular_core::{
    Capabilities, CellCoord, CellLevel, CellMetadata, ColumnInfo, ColumnUid, ModelError, Revision,
    Rgb, RowPosition, RowUid, TableModel, WrapMode,
};

pub use tabular_derive::TabularRow;

/// Support code for `#[derive(TabularRow)]`. Not public API.
#[doc(hidden)]
pub mod __derive {
    use rvariant::Variant;
    use std::fmt::Debug;

    /// Picks `Into<Variant>` when the field type has it, `Debug` text otherwise (autoref
    /// specialization: call as `(&&Wrap(&field)).to_variant()`).
    pub struct Wrap<'a, T>(pub &'a T);

    pub trait ViaInto {
        fn to_variant(&self) -> Variant;
    }

    impl<T: Clone + Into<Variant>> ViaInto for &Wrap<'_, T> {
        fn to_variant(&self) -> Variant {
            self.0.clone().into()
        }
    }

    pub trait ViaDebug {
        fn to_variant(&self) -> Variant;
    }

    impl<T: Debug> ViaDebug for Wrap<'_, T> {
        fn to_variant(&self) -> Variant {
            Variant::Str(format!("{:?}", self.0))
        }
    }
}
