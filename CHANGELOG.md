# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Sorting (SORT-1): `TableViewConfig::sort` takes multi-key `SortKey`s; stable; numbers by value, text case-insensitive and empties last (`RowOrderOptions`); `CellUi::compare` for custom columns; `TableModel::sorted_rows` for delegated sorting (e.g. SQL); `SortPolicy::{OnExternalChange, Live, OnDemand}` with `TableView::{refresh_rows, is_row_order_stale}`.
- Filtering (FILTER-1): `TableViewConfig::filter` = `RowFilter { quick, columns }` with `ColumnFilter::{Contains, OneOf, Range, Empty, Custom}` and the `CellUi::matches` hook; `TableView::row_counts()`.
- Saved view state (VIEW-12): column order, resized widths and hidden columns in `TableViewConfig` (serde, missing fields default); `TableEvent::ConfigChanged` when the view changed it.
- Rows by uid (VIEW-13): `TableEvent::{RowClicked, RowDoubleClicked, SortChanged}`, `TableView::{active_row, selected_rows, visible_rows, visible_columns, select_row, clear_selection, scroll_to_row}`.
- EDIT-13: `Date`, `Time` and `DateTime` cells. `DateCellUi` takes `DateFormats` (format strings per kind, optional relative days such as "3 d ago" or "in 2 d") and shows and edits with them; the text editor parses the format, ISO 8601 and relative words, and an invalid text is outlined and doesn't change the value. `show_value_with` / `edit_value_with` for custom `CellUi`s.
- `date_sort_key` and `DateRange` (from/to, both optional and inclusive) to sort and filter date columns by value.
- Dependency on `chrono`.

### Changed

- The selection follows its rows when the row order changes; the row number column shows the model's row number.

### Fixed

- DND-5: user column order is saved in `TableViewConfig`.
