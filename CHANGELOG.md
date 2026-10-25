# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Sorting (SORT-1): `TableViewConfig::sort` takes multi-key `SortKey`s; stable; numbers by value, text case-insensitive and empties last (`RowOrderOptions`); `CellUi::compare` for custom columns; `TableModel::sorted_rows` for delegated sorting (e.g. SQL); `SortPolicy::{OnExternalChange, Live, OnDemand}` with `TableView::{refresh_rows, is_row_order_stale}`.
- Filtering (FILTER-1): `TableViewConfig::filter` = `RowFilter { quick, columns }` with `ColumnFilter::{Contains, OneOf, Range, Empty, Custom}` and the `CellUi::matches` hook; `TableView::row_counts()`.
- Saved view state (VIEW-12): column order, resized widths and hidden columns in `TableViewConfig` (serde, missing fields default); `TableEvent::ConfigChanged` when the view changed it.
- Sort control in column headers (SORT-2): click cycles ascending/descending/off, Shift+click adds a key, key numbers, stale marker under `SortPolicy::OnDemand`; header menu "Sort ascending/descending" and "Clear sort"; `SortIndicatorOptions`; `click_sort` for apps with their own headers.
- Rows by uid (VIEW-13): `TableEvent::{RowClicked, RowDoubleClicked, SortChanged}`, `TableView::{active_row, selected_rows, visible_rows, visible_columns, select_row, clear_selection, scroll_to_row}`.
- EDIT-13: `Date`, `Time` and `DateTime` cells. `FormattedCellUi` takes `CellFormats` (format strings per kind, optional relative days such as "3 d ago" or "in 2 d") and shows and edits with them; the text editor parses the format, ISO 8601 and relative words, and an invalid text is outlined and doesn't change the value. `show_value_with` / `edit_value_with` for custom `CellUi`s.
- Dates and date-times sort by their point in time, also mixed in one column, and filter with `ColumnFilter::Range`.
- Dependency on `chrono`.
- Hidden columns UI (VIEW-14): "Hide" in the column header menu and a "Columns" checkbox list in the tool-column header menu to show them again; both emit `TableEvent::ConfigChanged`.
- `TableViewOptions::header_types` (default true) hides the type line under column names; `CellFormats::list_separator` shows `StrList` cells on one line.
- `demos/simple` shows sorting, a quick filter box, a status filter, a date column, an "N of M rows" line, a row details side panel and saves `TableViewConfig` with eframe persistence.
- Row spans (VIEW-16): `TableModel::row_span` reports a cell that covers several rows of its column (`RowSpan { first, rows }`); the view draws it once over those rows, also when it starts above the viewport, and treats it as one unit for hover, click, selection and arrow keys. Read-only, shown in the natural row order. `demos/row_spans` shows a schedule.
- `just test --changed` runs only the workspace crates changed against main and their dependents (`tpm test changed`, nextest's `rdeps()`); plain `just test` runs everything, as `just test-all` does (tpm ops#22oct-2049).

### Changed

- **Breaking:** the core contract is new (DESIGN-8): `TableBackend`/`TableFrontend` and the flag system are replaced by `tabular_core::TableModel` (egui-free) and `CellUi` with the row builder, a `TableCommand` queue, `TableViewOptions`/`TableViewOutput`/`TableEvent`; `VariantBackend` is `VariantTable`, the derive macro generates `<Row>Table`. Apps on 0.2 port to these types; the renames below are breaking too.
- `DateFormats` is now `CellFormats` and `DateCellUi` is `FormattedCellUi`: the formats cover more than dates.
- `ColumnInfo::is_sortable` defaults to true.
- The selection follows its rows when the row order changes; the row number column shows the model's row number.

### Fixed

- VIEW-15: the selection outline is painted after all row fills and on all four sides, so the row below no longer covers its bottom edge.
- DND-1: dropping a column header moves the column instead of swapping it with the target.
- DND-3: a selected cell follows its column when columns are moved.
- DND-5: user column order is saved in `TableViewConfig`.

