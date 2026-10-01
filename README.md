# Customizable egui table viewer and editor.

![Crates.io Version](https://img.shields.io/crates/v/egui_tabular)

<img align="right" src="https://github.com/romixlab/egui_tabular/blob/main/assets/simple_demo.png?raw=true" alt="logo"/>

Fast and responsive table viewer and editor, that only shows visible rows. The data model is fully generic,
allowing implementations based on vectors, files, databases, live bus traces and other data structures.

TODO: Add web demo.

## Features

* [x] Data comes from the egui-free [TableModel](tabular_core/src/model.rs) trait; change detection with revision counters, so any number of views and app code can observe one model.
* [x] Custom cell viewer and editor ui through [CellUi](src/cell_ui.rs), any egui or user widgets can be used.
  A row is looked up once and its cells filled by column, values (`Variant`) are optional.
* [x] Built-in cell viewers and editors ([VariantCellUi](src/cell_ui.rs)) and an in-memory [VariantTable](src/backends/variant.rs):
    * [x] String, string list, numbers, booleans, custom enums
    * [ ] Date, SI values, currency
* [x] Data import with automatic column mapping based on names.
    * [x] CSV support.
    * [ ] XLS support.
* [x] Manual column mapping to one of the choices provided by the backend (combo box above columns).
* [ ] Undo / Redo support.
* [x] Cell values don't have to be in memory (if the model supports it); the view keeps one id per row.
* [ ] Support for sorting.
* [ ] Support for filtering based on custom user ui from the TableBackend trait.
* [x] Keyboard shortcuts and navigation.
* [x] Copy-paste support for cells and blocks of cells.
* [x] Ability to add lints to cells and change their background color.
    * [ ] Add icons
* [x] Support for cells with varying heights.
* [x] Resizable, auto-sized columns.
* [x] Scroll position anchored to a row: stays put when rows are inserted above.
* [x] Drag&drop column reordering.
* [x] Export to CSV.
* [x] Stick to bottom mode for viewing real time data.
* [x] Visual state can be persisted on disk.
* [x] Disable/enable rows and columns (disabled cells are crossed out).
* [x] Change a column type and try to turn data into requested type (VariantTable, only from code now).
* [x] Values are converted to the column type on paste and import; ones that don't convert are highlighted.
* [x] Derive macro to show Vec<UserRowStruct> as table.
* [ ] Improve drag&drop, like on DK

## Usage

```rust
use egui_tabular::rvariant::VariantTy;
use egui_tabular::{ColumnDef, TableView, TableViewConfig, TableViewOptions, VariantCellUi, VariantTable};

let mut table = VariantTable::new([ColumnDef::new("Name", VariantTy::Str)]);
let mut view = TableView::new(TableViewOptions::default());
let mut config = TableViewConfig::default(); // user preferences, serde

// Every frame:
let output = view.show(ui, &mut table, &mut VariantCellUi, &mut config);
for event in output.events { /* selection, committed edits, messages for the user, ... */ }
```

For your own data, implement `TableModel` (data only) and, for custom rendering, `CellUi`.
See `demos/` and `#[derive(TabularRow)]`.

## Non-goals

* Become Excel or G.Sheets replacement.

## Potential features

* Export to XLS / XLSX

## Keyboard shortcuts

* Ctrl+V - paste block
* Ctrl+C - copy block
* Ctrl+A - select all (when not editing)
* Tab - commit edit and edit cell to the right
* E - edit cell
* Esc - cancel edit or unselect
* Left, Right, Up, Down - move selection (scrolls it into view)
    * +Shift - expand selection
* N - append new row and scroll to it
* Shift + click - expand selection
* Drag column edge - resize, double-click it - back to auto width

## Project status

Experimental — many of the essential features are implemented, but documentation is incomplete and examples are absent.

## Alternatives

This project borrows some ideas from the great [egui-data-table](https://github.com/kang-sw/egui-data-table).
Check it out if you don't need CSV/XLS import with column mapping or want to show some data based on a vector.
The idea behind the TableModel trait in this crate is to allow more advanced data retrieval, for example from a database.

## Compatible version with egui

Version | egui
--- | ---
0.1 | 0.33
0.2 | 0.36
