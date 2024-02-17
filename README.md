# egui_tabular
Table data structures and GUI

## Goals

* Show / edit large amount of data in tabular form
* Enforce types for columns
* Support sorting and filtering
* Access local or remote data through trait interface
* Derive macro to map Rust structs into rows of typed cells
* Key navigation
* Copy-paste support for cells and blocks of cells
* Fast and responsive (only visible cells are requested and rendered)
* Ability to add lints and icons to cells or change their color

## Non-goals

* Become Excel or G.Sheets replacement

## Additional features

* egui is gated under gui feature, disable it to work with data types and formats only
* CSV import and export with column mapping and data conversion

## Project status

Experimental - many of the essential features are implemented, but documentation is incomplete and examples are absent.