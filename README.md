TODO: Add crates and docs badge

# Customizable egui table viewer and editor.

Fast and responsive table viewer and editor, that only shows visible rows. Data backend is fully generic,
allowing implementations based on vectors, files, databases and other data structures.

TODO: Add web demo.

## Features

* [x] Cells UI and table information is provided through the [TableBackend](src/backend.rs) trait.
* [x] Custom cell viewer and editor ui, any egui or user widgets can be used.
* [x] Built-in cell viewers and editors through [VariantBackend](src/backends/variant.rs):
    * [x] String, string list, numbers, booleans, custom enums
    * [ ] Date, SI values, currency
* [x] Data import with automatic column mapping based on names.
    * [x] CSV support.
    * [ ] XLS support.
* [x] Manual column mapping to one of the choices provided by the backend (combo box above columns).
* [ ] Undo / Redo support.
* [x] No need to keep all data in memory (if backend supports it).
* [ ] Support for sorting.
* [ ] Support for filtering based on custom user ui from the TableBackend trait.
* [ ] Keyboard shortcuts and navigation.
* [ ] Copy-paste support for cells and blocks of cells.
* [x] Ability to add lints and icons to cells or change their background color.
* [x] Support for cells with various heights.
* [x] Drag&drop column reordering.
* [ ] Export to CSV and XLS.
* [ ] Stick to bottom mode for viewing real time data.
* [x] Visual state can be persisted on disk.
* [ ] Disable/enable rows and columns (show hatch pattern when disabled).
* [x] Change a column type and try to turn data into requested type (VariantBackend, only from code now).

## Non-goals

* Become Excel or G.Sheets replacement.

## Potential features

* Gate egui behind gui feature, disable it to work with data types and formats only.
* Derive macro to map Rust structs into rows of typed cells.

## Project status

Experimental - many of the essential features are implemented, but documentation is incomplete and examples are absent.

## Alternatives

This project borrows some ideas from the great [egui-data-table](https://github.com/kang-sw/egui-data-table).
Check it out if you don't need CSV/XLS import with column mapping or want to show some data based on a vector.
The idea behind TableBackend trait in this crate is to allow more advanced data retrieval, for example from a database.