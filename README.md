<p align="center">
    egui table viewer and editor
</p>

# Features

* Entry point for tabular data for further processing in a structured manner.
* Show / edit large amount of data in tabular form.
    * Fast and responsive (only visible cells are requested and rendered).
* Map columns by names on import from CSV/XLS, enforce types.
* Provide an easy way to access mapped columns from code.
* Change events are propagated to user code.
* Support sorting and filtering.
* Cell data and table information is retreived through a trait interface.
  * CSV implementation provided.
  * XLS planned.
* Derive macro to map Rust structs into rows of typed cells (tbd).
* Key navigation.
* Copy-paste support for cells and blocks of cells.
* Custom cell viewer and editor ui, any egui widgets can be used.
  * State is provided for each cell.
  * Can return events to user code.
* Ability to add lints and icons to cells or change their color.

# Non-goals

* Become Excel or G.Sheets replacement.

# Additional features

* egui is gated under gui feature, disable it to work with data types and formats only.
* CSV import and export.

# Project status

Experimental - many of the essential features are implemented, but documentation is incomplete and examples are absent.