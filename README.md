TODO: Add crates and docs badge

# Customizable egui table viewer and editor.

Fast and responsive table viewer and editor, that only shows visible rows. Data backend is fully generic,
allowing implementations based on vectors, files, databases and other data structures.

TODO: Add web demo.

## Features
 
* [x] Cells UI and table information is provided through the TableBackend trait.
* [x] Custom cell viewer and editor ui, any egui or user widgets can be used.
* [ ] Data import with automatic column mapping based on names.
  * [ ] CSV support.
  * [ ] XLS support.
* [ ] Undo / Redo support. 
* [x] No need to keep all data in memory (if backend supports it).
* [ ] Support for sorting.
* [ ] Support for filtering based on custom user ui from the TableBackend trait.
* [ ] Keyboard shortcuts and navigation.
* [ ] Copy-paste support for cells and blocks of cells.
* [ ] Ability to add lints and icons to cells or change their background color.
* [x] Support for cells with various heights.
* [ ] Drag&drop column reordering.
* [ ] Export to CSV and XLS.
* [ ] Stick to bottom mode for viewing real time data.
* [ ] Visual state can be persisted on disk.
* [ ] Disable/enable rows and columns (show hatch pattern when disabled).

## Non-goals

* Become Excel or G.Sheets replacement.

## Potential features

* Provide an optional way to access data from code (as rvariant, separate trait?).
* Gate egui behind gui feature, disable it to work with data types and formats only.
* Derive macro to map Rust structs into rows of typed cells.
* Propagate change and other events to user code.

## Project status

Experimental - many of the essential features are implemented, but documentation is incomplete and examples are absent.

## Alternatives

This project borrows some ideas from the great [egui-data-table](https://github.com/kang-sw/egui-data-table).
Check it out if you don't need CSV/XLS import with column mapping or want to show some data based on a vector.
The idea behind TableBackend trait in this crate is to allow more advanced data retrieval, for example from a database.