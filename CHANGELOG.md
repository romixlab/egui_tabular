# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Sorting (SORT-1): `TableViewConfig::sort` takes multi-key `SortKey`s; stable; numbers by value, text case-insensitive and empties last (`RowOrderOptions`); `CellUi::compare` for custom columns; `TableModel::sorted_rows` for delegated sorting (e.g. SQL); `SortPolicy::{OnExternalChange, Live, OnDemand}` with `TableView::{refresh_rows, is_row_order_stale}`.
- Filtering (FILTER-1): `TableViewConfig::filter` = `RowFilter { quick, columns }` with `ColumnFilter::{Contains, OneOf, Range, Empty, Custom}` and the `CellUi::matches` hook; `TableView::row_counts()`.
- Saved view state (VIEW-12): column order, resized widths and hidden columns in `TableViewConfig` (serde, missing fields default); `TableEvent::ConfigChanged` when the view changed it.
- Rows by uid (VIEW-13): `TableEvent::{RowClicked, RowDoubleClicked, SortChanged}`, `TableView::{active_row, selected_rows, visible_rows, visible_columns, select_row, clear_selection, scroll_to_row}`.

### Changed

- The selection follows its rows when the row order changes; the row number column shows the model's row number.

### Fixed

- DND-5: user column order is saved in `TableViewConfig`.
