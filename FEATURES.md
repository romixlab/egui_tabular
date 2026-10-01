# egui_tabular — Features, Known Issues and Planned Work

This file is the **single source of truth** for what the crate does, how well it does it, what is
broken and what is planned. README.md is the public pitch; this file is the engineering record.

- Every change that adds, removes, fixes or alters a feature **must update this file in the same
  commit** (see [Maintaining this file](#maintaining-this-file)).
- Issue IDs (`EDIT-1`, `DND-3`, ...) are stable. Never renumber or reuse an ID; mark it fixed instead.
- File/line references were taken at commit `29fddab` and will drift; the function name is the
  durable anchor. Many `table_view.rs` line numbers already moved with DESIGN-9; refresh them when
  touching an entry.

Last full review: 2026-10-01 (baseline `29fddab`, egui 0.36, egui_extras 0.36.1). Gap review
against other table implementations (Qt, TanStack, AG Grid, Glide, Excel/Sheets, egui-data-table):
2026-10-01, see [Goals, use cases and targets](#goals-use-cases-and-targets) and DESIGN-10 – DESIGN-14. The breaking
redesign in [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui) is in progress.
egui_extras was dropped in [DESIGN-9](#design-9-anchor-based-layout-without-egui_extras).

> **Branch `v0.3.0` is under heavy development.** Breaking changes are not only accepted but
> preferred whenever they lead to a better design: no compatibility shims, deprecation paths or
> adapters for the old API. Every known downstream user will be ported.

---

## Contents

1. [Status legend](#status-legend)
2. [Goals, use cases and targets](#goals-use-cases-and-targets)
3. [Crate layout](#crate-layout)
4. [Feature inventory](#feature-inventory)
5. [Keyboard and mouse reference](#keyboard-and-mouse-reference)
6. [Known bugs](#known-bugs)
7. [Design issues and planned rework](#design-issues-and-planned-rework)
8. [Unused API, dead code and housekeeping](#unused-api-dead-code-and-housekeeping)
9. [Roadmap](#roadmap)
10. [Fixed issues](#fixed-issues)
11. [Maintaining this file](#maintaining-this-file)

---

## Status legend

| Status | Meaning |
|--------|---------|
| ✅ done | Implemented and believed correct. |
| 🚧 partial | In progress or partially done; the note says what is missing. |
| 🐛 buggy | Implemented, but with known bugs (linked by ID). |
| ⬜ stub | UI or API exists but does nothing. |
| 📋 planned | Not implemented; intended. |
| 💡 idea | Not implemented; worth considering, not committed to. |
| ⛔ blocked | Can't proceed; the note says on what. |
| 🔍 verify | Probably done or obsolete; needs a check before closing. |

The same icons mark roadmap steps.

Severity for bugs: **crash** (panic), **data-loss** (user edits or data silently lost/corrupted),
**major** (feature visibly broken), **minor** (cosmetic or edge case).

---

## Goals, use cases and targets

### Use cases

The design has to serve all of these. When a change helps one and hurts another, record the
trade-off here.

| # | Use case | Model | What matters most |
|---|----------|-------|-------------------|
| U1 | **Import and map** CSV/XLSX files to required columns | `VariantTable` via `TabularImporter` | Column mapping, skip rows/columns, paste, export, error highlighting. |
| U2 | **Editable in-memory tables** | `VariantTable` | Spreadsheet-grade keyboard, clipboard, undo, sort, filter. |
| U3 | **Live data from hardware buses** (CAN, I2C, USART, SPI): bus traces, register maps, decoded signals | Custom model over the app's own structs, often **without `Variant` values** | Many updates per second, appends and evictions every frame, stick to bottom, custom widgets in cells (some interactive, e.g. writing a register), one data lookup per row (DESIGN-10). |
| U4 | **Read-only views of `Vec<Struct>`** | `#[derive(TabularRow)]` | Zero boilerplate, copy/export work. |
| U5 | **Database or remote data** | Custom model, delegated sorting | Rows not all in memory; row uids enumerable (see the DESIGN-8 known limit). |

### Feature availability for models without values

U3 models may render everything through a custom `CellUi` and return `None` from
`TableModel::get` for some or all columns ([DESIGN-11](#design-11-live-data-and-models-without-variant-values)).
The view must degrade per column, never panic or show broken UI. Features that need a value:

| Feature | Needs | Without it |
|---------|-------|------------|
| Rendering, selection, navigation, scrolling, column resize/reorder/hide, row heights, stick to bottom | nothing | Works fully. |
| Copy, CSV/XLSX export, find, quick filter | `get()` or `CellUi::text()` | Cells without either copy as empty; the column is left out of find/filter. |
| Sort by column | `get()` (or `sorted_rows` delegation) | "Sort" is disabled for the column. |
| Built-in editing (`VariantCellUi`), paste, Delete/clear, fill | `get()` + `set()` + `Capabilities::edit_cells` | Read-only; a custom `CellUi` may still host interactive widgets that write through `&mut M`. |
| Selection status bar (sum/avg/count) | numeric `get()` | Shows count only. |
| Undo/redo | view commands | Only covers view-originated commands; writes done by custom widgets are not undoable. |

### Non-goals

- An Excel or Google Sheets replacement: no formulas, merged cells, multiple disjoint selections,
  or charts.
- Tables whose row uids can't be enumerated (see the DESIGN-8 known limit). Not a redesign if
  needed later.

### Performance targets

📋 Not measured yet; a headless benchmark should check these (see
[housekeeping](#unused-api-dead-code-and-housekeeping)). Release build, desktop CPU.

| Scenario | Target |
|----------|--------|
| Scrolling a 1M-row `VariantTable`, 20 columns | < 2 ms view time per frame |
| Live model appending 1000 rows/s and evicting from the front, stick to bottom | < 1 ms per frame, no O(row count) work per frame (DESIGN-11) |
| Local sort of 1M rows by one column | < 150 ms |
| Row-builder `CellUi` (DESIGN-10) | One model lookup per visible row per frame |

---

## Crate layout

| Path | Purpose |
|------|---------|
| `tabular_core/` | egui-independent core: `TableModel` trait, `Revision`, `Capabilities`, `ColumnInfo`, `ModelError`, `RowPosition`, `ColumnUid`, `RowUid` (u64), `CellCoord`, `CellMetadata`, `CellLevel`, `Rgb`, `WrapMode`, `CsvImporterConfig`. Re-exports `rvariant::{Variant, VariantTy}`. |
| `tabular_derive/` | `#[derive(TabularRow)]` proc macro: turns `Vec<Row>` into a read-only `<Row>Table` model. |
| `src/table_view.rs` | `TableView::show()`, `TableViewOptions`, `TableViewOutput`, `TableEvent`: model sync by revision, command application, header and body layout (own cell layout, no egui_extras), column DnD and resize handles. |
| `src/table_view/layout.rs` | `RowLayout` (anchor-based vertical scrolling, row order, row heights by `RowUid`) and `ColumnWidths` (by column). Unit-tested. |
| `src/table_view/scroll_bar.rs` | Row-proportional vertical scroll bar. |
| `src/table_view/interaction.rs` | Keyboard handling, copy, paste (+ paste modal), selection moves. |
| `src/table_view/state.rs` | `State` (incl. the edit buffer, command queue, pending events) and `SelectedRange` (selection + editing coord). |
| `src/table_view/tool_column.rs` | Left "tool" column: row numbers and context menus. |
| `src/table_view/config.rs` | `TableViewConfig` (serializable view settings). |
| `src/cell_ui.rs` | `CellUi` trait (row builder `show_row`, `show_cell`, `text`, `begin_edit`, `show_editor`, `header_ui`), `RowCells`, `EditorResponse`, `VariantCellUi`, built-in viewer and editor. |
| `src/commands.rs` | `TableCommand` (view-originated changes) and `apply`, which returns the inverse. Unit-tested. |
| `src/backends/variant.rs` | `VariantTable`: in-memory `HashMap<CellCoord, Variant>` model, `ColumnDef`. |
| `src/importers/` | `TabularImporter` (file picker + CSV options UI), `CsvImporter`, `RequiredColumns` (name/synonym mapping). |
| `src/util.rs` | `base_26` column names, encoding detection, cell text for copy/export, CSV export. |
| `demos/simple`, `demos/derive_row`, `demos/csv_xls_import` | Example apps. |
| `tests/ui/` | Integration test target `ui` of the root crate: headless UI tests (egui_kittest): selection, editing, keyboard, column drag & resize, paste, model contract (custom models, row builder, events). `fixture.rs` holds the harness helpers, generic over model and cell UI. Known bugs have `#[ignore = "<ID>: ..."]` repro tests. See AGENTS.md, "UI tests". |
| `tests/Cargo.toml`, `tests/src/` | Workspace package `tests`: derive macro tests (values, format, tuple structs, visibility). |

`TableView::show(ui, &mut model, &mut cell_ui, &mut config)` works with any `M: TableModel` and
`C: CellUi<M>`. The model owns data and column info and bumps revision counters. The view owns
visual column order, column widths, the row display order with the scroll anchor and row heights,
selection, the edit buffer, and paste state. It changes the model only through queued
`TableCommand`s, applied outside of drawing.

---

## Feature inventory

### Core data model (`TableModel`)

| Feature | Status | Notes |
|---------|--------|-------|
| Generic egui-free `TableModel` trait (rows/cols by uid, natural order only) | ✅ done | [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui), step 1. The view does all visual ordering. |
| `get` / `set` cells as `Variant` | ✅ done | Both optional. Copy and export fall back to `CellUi::text` without `get`. |
| Create row (append or at a position) / create column | ✅ done | `RowPosition`. `VariantTable` names new columns `A`, `B`, ... via `base_26`. |
| Remove rows / remove columns | 🚧 partial | Model methods, capabilities and commands exist (used as undo inverses); no UI yet (roadmap step 5). |
| Row / column skipping (strike-through, excluded from export) | 🐛 buggy | Skipped columns are still exported ([EXPORT-3](#export-3)). |
| Cell metadata: semantic level (background), corner triangle, multiple tooltips, wrap mode | ✅ done | `TableModel::metadata`, drawn by the view; `CellLevel::{Info, Warning, Error, Changed, Custom}` map to theme colors. `VariantTable::set_metadata(coord, meta, merge)`. |
| Change notification by revision counters (`Revision`) | ✅ done | Never consumed: any number of views and app code compare with their own last-seen copy. Replaced the flag system (FLAGS-1..5). |
| Capabilities (`Capabilities`) | ✅ done | The view only offers what the model allows; `TableViewOptions::read_only` on top. |
| Read-only tables | ✅ done | `VariantTable::set_read_only` and `TableViewOptions::read_only`. Skipping stays allowed. |
| Remote/lazy models | 💡 idea | Server-side sorting has a planned hook (`sorted_rows`, [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui), "Row order"), added with sorting. |
| Column "used" vs "available" | ✅ done | `ColumnInfo::is_used`, informational (shown in the header hover). The view shows all columns. |
| Undo / redo | 🚧 partial | Every `TableCommand::apply` returns its inverse (unit-tested); no undo stack or UI yet (roadmap step 5). |
| Sorting | ⬜ stub | `is_sortable`, "Sort ascending/descending" menu items exist but do nothing. Planned in the view ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui)); UI and stability policy in [DESIGN-13](#design-13-sort-and-filter-stability-and-ui). |
| Filtering | 📋 planned | Planned in the view ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui)); UI in [DESIGN-13](#design-13-sort-and-filter-stability-and-ui). |
| Typed column type (`VariantTy`, or none for custom columns) | ✅ done | `ColumnInfo::ty: Option<VariantTy>` + `type_label`. Picks the editor for empty cells. |
| Rename columns | 📋 planned | Model method, capability and command come with the UI (roadmap step 5). |
| Move rows (drag in the tool column) | 💡 idea | Needs a model-side order (`move_rows`), since the view's row order is otherwise natural or sorted. |
| Hierarchical rows (expand a frame into decoded signals, group by) | 💡 idea | U3. Would be a view-side row-order stage after filter/sort; tree depth drawn in the tool column. |
| Row uid contract (stable for a row's life, never reused) | ✅ done | Documented on `TableModel`; `RowUid` is `u64`. `VariantTable::clear` no longer restarts uids at 0. Derived tables use row indices (documented). |
| Batched value access for copy/export | ✅ done | `TableModel::row_values` with a per-cell default; copy and export call it once per row ([DESIGN-10](#design-10-row-builder-for-cell-ui)). |

### `VariantTable` and the built-in cell UI (`VariantCellUi`)

| Feature | Status | Notes |
|---------|--------|-------|
| In-memory storage, column defaults applied on row insert, `ColumnDef` | ✅ done | |
| Values converted to the column type on `set` and insert (paste, import) | ✅ done | Text is parsed; blank text in a non-text column becomes `Empty`. Values that don't convert are kept and get a `Warning` level + tooltip (PASTE-5). |
| `turn_column_into(ty)` with conversion errors shown as warning cells + tooltip | ✅ done | Code-only API, no UI. |
| Viewers: Str, StrList, Bool (check mark), Enum, numbers, others via `Display` | ✅ done | Built-in `show_value`, the default of `CellUi::show_cell`. |
| Editors: Str (TextEdit), Bool, Enum (ComboBox), U32/U64/I32/I64 (DragValue) | 🚧 partial | Built-in `edit_value`, the default of `CellUi::show_editor`; Bool and Enum commit on change. No editor for F32/F64 or other `Number` widths ([EDIT-8](#edit-8)). |
| Column mapping choices (combo box above columns) | ✅ done | Choices in `TableViewOptions::column_mapping_choices`; selection stored in `TableViewConfig::column_mapped_to`, keyed by `ColumnUid`; `TableEvent::ColumnMappingChanged`. |
| Date, SI values, currency viewers/editors | 📋 planned | |
| Numbers edited as parsed text instead of `DragValue` | 📋 planned | `DragValue` in a grid changes values on an accidental drag ([EDIT-11](#edit-11)). Keep `DragValue` as an opt-in. |
| Searchable enum editor | 📋 planned | ComboBox with a filter box for long enum lists. |
| Multi-line text editing (Alt+Enter inserts a newline) | 📋 planned | Needs the editor overlay ([DESIGN-12](#design-12-editor-overlay)). |
| Change a column's type from the UI | 💡 idea | `turn_column_into` is code-only; a header menu "Convert to…" would expose it. |

### Live data and custom models

Use case U3 ([Goals](#goals-use-cases-and-targets)). Design in
[DESIGN-11](#design-11-live-data-and-models-without-variant-values).

| Feature | Status | Notes |
|---------|--------|-------|
| Custom cell UI with any egui widget | ✅ done | `CellUi`; cells get `&mut M` for interactive widgets. |
| Row builder: one model lookup per row, cells filled by column ([DESIGN-10](#design-10-row-builder-for-cell-ui)) | ✅ done | `CellUi::show_row` + `RowCells::{cell, wants, level, tooltip}`. Tested: one lookup per visible row per frame; cells land in their columns after a move. |
| Models without `Variant` values (`get` optional), per-column degradation | 🚧 partial | Rendering, selection, copy (via `CellUi::text`) work; such cells aren't editable. Header/menu hints for disabled features come with sort (DESIGN-11). |
| Text for custom cells (`CellUi::text`) for copy/export | ✅ done | Find comes with DESIGN-13. |
| Cheap append + front eviction (ring buffer traces) | 🚧 partial | Every row-set change collects all uids into a new `Vec` and compares prefixes (`RowLayout::sync`), so even appends are O(row count); appends then index only the new rows. Front eviction re-indexes everything. A trace that appends every frame pays O(row count) per frame. Planned `rows_log` ([DESIGN-11](#design-11-live-data-and-models-without-variant-values)). |
| Stick to bottom | 🚧 partial | See Table view; needs to be an option and to survive eviction. |
| Scroll position kept while rows are evicted above | ✅ done | The anchor is a `RowUid`; if the anchored row itself is evicted the view stays at its index. |
| Sort/filter policy for constantly changing values (live re-sort vs. on demand) | 📋 planned | [DESIGN-13](#design-13-sort-and-filter-stability-and-ui). |
| Highlight changed values (flash that fades) | 📋 planned | `CellLevel::Changed` + fade in the view, or per-cell "changed at" in metadata. |
| Pause / freeze display while data keeps coming | 💡 idea | View-side snapshot of `visible_rows` plus "N new rows" badge; app keeps receiving. |
| Live data demo (simulated CAN bus trace + register map) | 📋 planned | `demos/live`; also the benchmark scenario. |
| 64-bit `RowUid` | ✅ done | A 1 kHz bus would overflow `u32` in about 50 days, 8 kHz in under a week, and uids must never be reused. |

### Table view

| Feature | Status | Notes |
|---------|--------|-------|
| Virtualized rendering (only visible rows) | ✅ done | Rows are laid out outwards from a scroll anchor; cost is O(visible rows) ([DESIGN-9](#design-9-anchor-based-layout-without-egui_extras)). |
| Scroll position anchored to a row | ✅ done | Inserting/removing rows above keeps the view on the same row. |
| Mouse wheel and vertical scroll bar | ✅ done | Bar sits right after the last column (or at the right edge if columns are wider). Proportional to rows, not pixels. Wheel delta passes to a parent scroll area at the ends. No touch drag-to-scroll ([VIEW-8](#view-8)). |
| Horizontal scrolling | ✅ done | egui `ScrollArea::horizontal`. |
| Heterogeneous row heights | 🐛 buggy | `TableViewConfig::use_heterogeneous_row_heights` (checkbox in the `simple` demo). Measured every frame and cached by `RowUid`; editing makes rows jump ([EDIT-5](#edit-5)). |
| Resizable columns | ✅ done | Drag the right edge of a column; double-click it to return to auto width. Widths are keyed by `ColumnUid` but not persisted ([DND-5](#dnd-5)). |
| Auto-sized columns | ✅ done | Grow to fit the widest header/cell seen, up to 400 px; never shrink on their own. Double-click on the edge re-fits to the header and visible rows (may shrink). Width changes are eased. |
| Column header: name, type, hover info (required/synonyms/used) | ✅ done | |
| Column header context menu | 🚧 partial | "Skip" and "Add column" work (offered per capabilities). "Sort", "Hide" are stubs ([VIEW-5](#view-5)). |
| Column drag & drop reorder | 🐛 buggy | Swaps instead of moves; selection and order persistence broken ([DND-*](#column-drag--drop)). Reports `TableEvent::ColumnsReordered`. |
| Tool column (row numbers, row context menu: append, skip) | ✅ done | `TableViewOptions::tool_column` hides it. |
| Tool column header menu: Export CSV, Append row, Clear | 🐛 buggy | "Clear" has no confirmation ([VIEW-6](#view-6)). |
| "No columns" state with "Create column" button | ✅ done | Offered only if the model can create columns. |
| Multiple tables in one parent `Ui` | ✅ done | All view ids derive from `TableViewOptions::id_salt` (salted with the parent `Ui`'s id). |
| Several views over one model | ✅ done | Each view tracks the model's `Revision` itself (FLAGS-1). Tested. |
| View output: `TableViewOutput { response, events }` | ✅ done | `TableEvent::{SelectionChanged, CellCommitted, EditCancelled, CommandFailed, ColumnMappingChanged, ColumnsReordered, RowsCreated, Message}`. |
| Cell background colors / corner triangles / tooltips | ✅ done | |
| Selection: single cell, rectangle, whole row, select all | 🐛 buggy | See [SEL-*](#selection-and-keyboard). |
| Scroll selection into view on keyboard navigation | 🚧 partial | Vertical only (arrow keys). No horizontal reveal yet. |
| Page Up / Page Down / Home / End | 📋 planned | Trivial with the anchor: move it by what fit on screen. |
| Ctrl+Arrow (jump to the data edge), Ctrl+Home / Ctrl+End | 📋 planned | Excel/Sheets convention. Edge = last non-empty cell before an empty one, or the table edge. |
| Shift+Tab, Shift+Enter move backwards; Enter after commit moves down; Tab wraps at row end | 📋 planned | Excel convention. Today Enter stays on the cell and Tab stops at the last column. |
| Column selection (click header, Ctrl+Space) and Shift+Space row selection | 📋 planned | `Selection.kind` gets `Columns` (DESIGN-8 amendment). Header click today starts a drag only. |
| Drag-select cells with the mouse | 📋 planned | Only Shift+click extends today. On touch, body drag scrolls instead ([VIEW-8](#view-8)). |
| Selected rows/columns highlighted in headers and row numbers | 📋 planned | [DESIGN-14](#design-14-look-and-feel). |
| Active cell drawn separately from the selected range | 📋 planned | [DESIGN-14](#design-14-look-and-feel); folds in [VIEW-7](#view-7). |
| Cell context menu (copy, cut, paste, clear, insert/delete row) | 📋 planned | Only the tool column and headers have menus. |
| Programmatic control (`set_selection`, `scroll_to`, `start_edit`, `cancel_edit`, `focus`) | 📋 planned | Roadmap step 2, with the uid-based selection. Needed for "jump to error", "select imported row". |
| Find (Ctrl+F): highlight matches, next/previous | 📋 planned | Distinct from filtering; uses `get()`/`CellUi::text()`. [DESIGN-13](#design-13-sort-and-filter-stability-and-ui). |
| Hide columns and a column chooser to show them again | 📋 planned | `hidden_columns` is planned in config, but without a chooser hidden columns can't be restored. Chooser lives in the tool-column header menu. |
| Frozen (pinned) left columns; tool column stays while scrolling sideways | 📋 planned | Listed under "Not done yet" in DESIGN-9. Pinned columns are a `TableViewConfig` list. |
| Rename column (double-click header), delete rows, insert row above/below | 📋 planned | Remove and insert exist in the model and as commands; rename and the UI come in roadmap step 5. |
| Stick-to-bottom for live data | 🚧 partial | Always on: if the end of the table is in view, appended rows keep it there. Not configurable (`TableViewOptions::stick_to_bottom` planned). |
| Long text ends with "…" and shows the full text on hover | 📋 planned | Text is clipped at the column edge today. [DESIGN-14](#design-14-look-and-feel). |
| Alignment by column type (numbers right-aligned, tabular digits) | 📋 planned | Everything is left-aligned. Needs the typed column type. |
| Density (padding, row height) and grid line settings | 📋 planned | `TableStyle`, [DESIGN-14](#design-14-look-and-feel). |
| Semantic cell colors (info/warning/error/changed) that follow the theme | ✅ done | `CellLevel`, mapped to `Visuals` colors by the view (VIEW-11). `Custom(Rgb)` doesn't pick text color by contrast yet (DESIGN-14). |
| Selection status bar (count, sum, average) | 📋 planned | Optional footer. Count only without numeric values. |
| Empty, loading and error states | 🚧 partial | "No columns" and "Add row" states exist, per capabilities. No loading state for lazy models (`CellState::Loading`, U5). |
| User feedback for refused or failed actions | ✅ done | `TableEvent::Message { level, text }` (paste without selection, paste the table can't take) and `TableEvent::CommandFailed`; the app shows them (VIEW-10). |
| Striped rows, hover highlight | ✅ done | Odd rows use `faint_bg_color`, the hovered row `widgets.hovered.bg_fill`. |
| Scroll to a newly appended row | ✅ done | `N`, "Append row" in the tool column menus and the "Add row" button. |
| Visual state persistence (`TableViewConfig` is serde) | 🚧 partial | Column order and widths are not persisted ([DND-5](#dnd-5)). |
| Custom column header UI (`CellUi::header_ui`) | ✅ done | |
| Per-column render config | 📋 planned | Widths become per-column view state ([DESIGN-4](#design-4-column-order-and-widths-as-persisted-view-state)). |
| Column order kept when the model's column set changes | ✅ done | Known columns keep the user's order, new ones are appended (`sync_model`). Not persisted yet ([DND-5](#dnd-5)). |

### Editing

| Feature | Status | Notes |
|---------|--------|-------|
| Click selected cell to edit; `E` to edit | ✅ done | `E` is replaced by F2 in step 2 ([EDIT-9](#edit-9)). |
| Enter commits, Escape cancels, Tab commits and edits next cell | 🐛 buggy | Global Enter ([EDIT-6](#edit-6)). The view owns the edit buffer and commits through a `TableCommand::Set`. |
| Clicking another cell or a row number commits | ✅ done | EDIT-1. |
| Editors commit on change (Bool, Enum) | ✅ done | `EditorResponse::commit`. |
| Commit on focus loss / click outside | 📋 planned | [EDIT-3](#edit-3). |
| Start editing by typing / Enter / F2 / double-click | 📋 planned | [EDIT-9](#edit-9). |
| Delete / Backspace clears the selected cells | 📋 planned | Pushes one `Set { Empty }` per cell as one undoable command. Respects capabilities and skipped/read-only columns. |
| Cut (Ctrl+X) | 📋 planned | Copy + clear; via `Event::Cut`. |
| Ctrl+Enter writes the typed value into every selected cell | 📋 planned | Excel convention. |
| Fill down / right (Ctrl+D / Ctrl+R) | 📋 planned | |
| Fill handle (drag the selection corner) | 💡 idea | Glide/Excel. Copy only, no series detection. |
| Editor drawn above the cell, rows never change height | 📋 planned | [DESIGN-12](#design-12-editor-overlay); fixes [EDIT-5](#edit-5) structurally. |
| Inline validation error (editor stays open with the error) | 📋 planned | DESIGN-8 "Edit lifecycle". |

### Clipboard

| Feature | Status | Notes |
|---------|--------|-------|
| Copy selection as TSV (Ctrl+C) | 🐛 buggy | Uses `ctrl` not `command` ([SEL-5](#sel-5)). One `row_values` lookup per row; `CellUi::text` where the model has no value. |
| Paste TSV block into selection | 🐛 buggy | See [PASTE-*](#paste). |
| Paste into empty table creates columns and rows | ✅ done | |
| Paste dialog for size mismatch (create rows, repeat fill, create columns) | 🐛 buggy | [PASTE-1](#paste-1) – [PASTE-4](#paste-4). |
| Paste "overflow" mode | 📋 planned | Commented-out button in `handle_paste_continue`. |
| Paste a 1×1 value, or a block that tiles the selection evenly, without a dialog | 📋 planned | Excel/Sheets fill the selection silently; the dialog is for real size mismatches only. |
| Copy raw values (round-trip) rather than displayed text | 📋 planned | Decide per column: `get()` value via a canonical `to_string`, not the viewer's formatting (Qt's display vs. edit role). Custom cells use `CellUi::text()`. |
| Copy with headers | 📋 planned | Option or a context menu entry. |
| Report cells that failed to parse on paste | 🚧 partial | `VariantTable` marks them (`Warning` level + tooltip). A summary `TableEvent::Message` is still missing. |
| Rich clipboard (HTML table) | 💡 idea | egui only exposes text today. |

### Import / export

| Feature | Status | Notes |
|---------|--------|-------|
| `TabularImporter` UI: file picker, reload, separator, header row, skip N rows | ✅ done | |
| CSV import with auto separator detection | ✅ done | Counts `,` `\t` `;` in the first MiB. |
| Encoding detection (chardetng) | 🐛 buggy | Never reads any bytes ([IMPORT-1](#import-1)). |
| Required columns mapped by name/synonym (case-insensitive) | ✅ done | Header names are not trimmed ([IMPORT-4](#import-4)). Values are converted by `VariantTable`; ones that don't convert are highlighted. |
| `TabularImporter::show(config, ui) -> TableViewOutput`, `table()`/`table_mut()` | ✅ done | View options via `importer.table_view.options_mut()`. |
| CSV without header row | 🐛 buggy | Last column dropped ([IMPORT-2](#import-2)). |
| Ragged CSV (rows wider than header) | 🐛 buggy | Extra cells land in wrong columns ([IMPORT-3](#import-3)). |
| Preview mode (`set_max_lines`) | ✅ done | |
| XLS/XLSX import | 📋 planned | Demo is named `csv_xls_import` but only CSV exists. |
| Export CSV | 🐛 buggy | See [EXPORT-*](#export). |
| Export XLS/XLSX | 💡 idea | |

### Derive macro (`#[derive(TabularRow)]`)

| Feature | Status | Notes |
|---------|--------|-------|
| Generates `<vis> struct <Row>Table` (a `TableModel`) with `new`, `data`, `data_mut`, read-only | ✅ done | Same visibility as the row struct. No egui in the generated code. Row uids are indices. |
| Column names from field names (Sentence case; index for tuple structs), Rust type shown in header | ✅ done | |
| Values: `Into<Variant>` when the field type has it, `Debug` text otherwise | ✅ done | Autoref dispatch in `egui_tabular::__derive`. |
| `#[format = "..."]` per field | ✅ done | The value is the formatted text. |
| Copy / CSV export from derived tables | ✅ done | `get()` and a one-lookup `row_values()` are generated. |
| Column value types (`ColumnInfo::ty`) for derived tables | 📋 planned | Columns have `ty: None`, so sorting (step 2) needs a type for them. |

### Accessibility, internationalization and platforms

| Feature | Status | Notes |
|---------|--------|-------|
| AccessKit grid semantics (grid/row/cell roles, row and column index, selected state, cell value as label) | 📋 planned | Cells have role `Unknown` and no label today. Also makes empty cells reachable in UI tests without `click_at`. |
| Keyboard-only operation (no mouse needed) | 🚧 partial | Needs focus-based input (SEL-4) and keyboard access to menus (Shift+F10 / Menu key). |
| Translatable UI strings | 💡 idea | "Append row (N)", paste dialog, menus are hard-coded English. A `TableStrings` struct in `TableViewOptions`. |
| Right-to-left layout | 💡 idea | Not planned unless asked for. |
| wasm | 🚧 partial | Builds are untested; `rfd` and the blocking save dialog don't work there ([EXPORT-4](#export-4), DESIGN-7). No web demo (README TODO). |
| Touch | 🐛 buggy | No drag-to-scroll ([VIEW-8](#view-8)); resize handles are thin. |
| macOS shortcuts (Cmd) | 🐛 buggy | [SEL-5](#sel-5). |

---

## Keyboard and mouse reference

Actual behavior at the baseline commit (including quirks). Keyboard handling is active only while
the **mouse pointer is over the table**, not based on focus ([SEL-4](#sel-4)).

| Input | Context | Behavior |
|-------|---------|----------|
| Click cell | — | Select cell. Commits the edit if another cell was being edited. |
| Click selected cell | Not read-only | Enter edit mode. |
| Shift+click cell | — | Grow selection bounding box (cannot shrink, [SEL-3](#sel-3)). |
| Click / Shift+click tool column | — | Select row / extend row selection. Commits an in-progress edit. |
| Right-click tool column / header | — | Context menus (see feature inventory). |
| Drag column header | — | Swap with drop target ([DND-1](#dnd-1)). |
| Drag column right edge | — | Resize the column (full table height is the handle). |
| Double-click column right edge | — | Return the column to auto width and re-fit it to the header and visible rows, eased (tool column: default width). |
| Mouse wheel | Pointer over table | Scroll rows; Shift+wheel scrolls horizontally. |
| Drag / click scroll bar | — | Drag the thumb, or click the track to jump there. |
| Arrows (+Shift) | Not editing | Move (grow) selection and scroll the moved edge into view. Panics with 0 rows ([SEL-2](#sel-2)). |
| `E` | Not editing, editable, single cell | Start editing. |
| `N` | Not editing, rows can be created | Append row and scroll to it. Also fires while typing in other widgets ([SEL-4](#sel-4)). |
| Ctrl+A / Cmd+A | Not editing | Select all. |
| Ctrl+C | Not editing | Copy TSV. Cmd+C on macOS doesn't work ([SEL-5](#sel-5)). |
| Ctrl/Cmd+V | Not editing, editable | Paste. Without a selection: `TableEvent::Message`. |
| Enter | Editing | Commit. Also fires if Enter was pressed elsewhere ([EDIT-6](#edit-6)). Bool and Enum editors commit on change. |
| Escape | Editing | Cancel edit. |
| Escape | Not editing | Clear selection. |
| Tab | Editing | Commit, move right, edit. |

### Planned input (target behavior)

Follows Excel/Sheets conventions unless noted. Move rows to the table above as they land. Shortcuts
run only while the table is active (focus-based, DESIGN-8).

| Input | Context | Behavior |
|-------|---------|----------|
| Enter / F2 / double-click / typing a character | Not editing | Start editing (typing seeds a text editor with the character). Replaces `E`. |
| Enter / Shift+Enter | Editing | Commit and move down / up. |
| Tab / Shift+Tab | Editing or not | Commit and move right / left; wraps to the next / previous row. |
| Ctrl+Enter | Editing, multi-cell selection | Write the value into every selected cell. |
| Alt+Enter | Editing a text cell | Insert a newline (Excel convention; Shift+Enter is "move up"). |
| Delete / Backspace | Not editing | Clear selected cells. |
| Ctrl/Cmd+X | Not editing | Cut. |
| Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z (Ctrl+Y) | Not editing | Undo / redo. |
| Ctrl+D / Ctrl+R | Not editing | Fill down / right. |
| Ctrl+Arrow, Ctrl+Home / Ctrl+End | Not editing | Jump to the data edge / table corners (+Shift extends). |
| Home / End, Page Up / Page Down | Not editing | Row start / end; move by one screen (+Shift extends). |
| Ctrl+Space / Shift+Space | Not editing | Select column / row. |
| Ctrl/Cmd+F | Table active | Find. |
| Click header | — | Select column. Shift/Ctrl+click extends. Sorting moves to a sort indicator in the header (DESIGN-13). |
| Double-click header | Columns renamable | Rename column. |
| Drag over cells | Desktop | Select range. On touch: scroll ([VIEW-8](#view-8)). |
| Right-click cell | — | Cell context menu. |
| Shift+F10 / Menu key | Not editing | Context menu for the active cell. |

---

## Known bugs

### Cell editing

Since roadmap step 1 the view owns the edit buffer (`State::edit`); the backend-side buffer and
the bugs it caused (EDIT-1, EDIT-2, EDIT-4) are gone. The rest is the edit lifecycle of step 2
([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui), "Edit lifecycle").

#### EDIT-3
**Clicking outside the table, or Tab-ing focus away, leaves the editor open but unfocused.** — *major*
- Where: `show_body`.
- Cause: the editor response's `lost_focus()` is never checked. Keystrokes go nowhere until the user clicks back into the cell. With the pointer outside the table, Tab is handled by egui focus traversal instead of the table.
- Fix: commit (or cancel, configurable) on `lost_focus()`.

#### EDIT-5
**Editor is taller than the row: rows jump or the editor is clipped.** — *major (visual)*
- Where: `TableViewConfig::default` (`minimum_row_height = 15.0`), `show_body` row heights.
- Cause: TextEdit/DragValue are about 20px plus frame. With heterogeneous heights the row grows when the editor opens (rows below shift) and shrinks back after commit. Since DESIGN-9 there is no frame of overflow any more: rows below are placed after the measured row. With fixed heights the editor is clipped.
- Fix: default the minimum row height to `ui.spacing().interact_size.y`, and/or use frameless editors sized to the row.

#### EDIT-6
**Enter commit is global.** — *minor*
- Where: `show_body`.
- Cause: `cell.input(|i| i.key_pressed(Key::Enter))` is checked regardless of which widget had focus.
- Fix: commit on `resp.lost_focus() && enter`. (Bool and Enum commit on change since step 1, `EditorResponse::commit`.)

#### EDIT-8
**No editor for F32/F64 and other `Number` widths.** — *minor*
- Where: `cell_ui::edit_value`. Shows "Editor is not implemented for …".

#### EDIT-9
**Missing edit entry points.** — *minor*
- No Enter/F2/double-click/type-to-edit. (The dead arrow-key commit was removed in step 1, the empty double-click branch with DESIGN-9.)
- Test (ignored, fails): `editing::f2_starts_editing`.

#### EDIT-12
**An edit survives the removal of its row or column.** — *minor*
- Where: `State::end_stale_edit`, `VariantTable::set`.
- If the edited row is removed (e.g. "Clear" from the tool column menu while editing), the editing coord stays in the selection and the buffer is kept. A later commit writes a cell for a row that no longer exists: `VariantTable::set` only checks the column, so an orphan cell is stored.
- Fix: cancel the edit when its row or column disappears (DESIGN-8 "Edit lifecycle", step 2); `VariantTable::set` should return `NotFound` for unknown rows.
- Found by code reading.

#### EDIT-11
**Integer editors are `DragValue`s: a small drag changes the value.** — *minor (UX)*
- Where: `cell_ui::edit_value` (U32/U64/I32/I64).
- Effect: clicking into the editor and moving the mouse slightly while pressed changes the number. Grids (Excel, AG Grid, Glide) edit numbers as text and parse on commit.
- Fix: a parsing `TextEdit` with validation in `VariantCellUi`; `DragValue` as an opt-in per column.

### Column drag & drop

#### DND-1
**Drop swaps columns instead of moving.** — *major (UX)*
- Where: `TableView::swap_columns` (`table_view.rs:376`).
- Dropping A on D in `A B C D` yields `D B C A`. Expected `B C D A` (or insert-before semantics with an insertion marker).
- Test (ignored, fails): `columns::drag_header_moves_column`.

#### DND-3
**`SelectedRange::swap_col` is a no-op when the selection is in the drop-target column.** — *major*
- Where: `state.rs:122-133`.
- Cause: the first `if` sets `col_start = col2`, then the second `if self.col_start == col2` immediately sets it back to `col1`. Needs `else if`. `swap_columns` passes the drop target as `col1`, so only a selection in the target column is affected; a selection in the dragged column follows it (`columns::selection_follows_dragged_column`). The selection (and an in-progress edit) ends up on the wrong column.
- Test (ignored, fails): `columns::selection_follows_drop_target_column`.

#### DND-4
**Drop-target highlight hides the header text and highlights the source column.** — *minor (visual)*
- Where: `table_view.rs:188-208`.
- Painted after the content at 50% alpha over the top 66% of the cell. Prefer a thin insertion line on the target edge, and skip it when hovering the dragged column itself.

#### DND-5
**User column order is not persisted.** — *major*
- Since step 1 the order survives column set changes (`sync_model` keeps known columns in place), but it is not stored in `TableViewConfig`, so it is lost on restart.

#### DND-6
**Grab cursor only over the label.** — *minor*
- `on_hover_cursor(Grab)` is set on the name label (`table_view.rs:155`), but the whole header cell is draggable.

### Selection and keyboard

#### SEL-2
**Arrow keys panic with zero rows.** — *crash (debug)*
- Where: `move_down`/`move_right` compute `count - 1` (`state.rs:190`, `:210`).
- Repro: select a cell, use tool header menu → Clear (if it doesn't panic first, see VIEW-1), press ↓.
- Cause: the selection is never validated or cleared when the row/column set changes.
- Test (ignored, fails): `keyboard::arrows_with_zero_rows_dont_panic`.

#### SEL-3
**Shift-extend has no anchor.** — *minor (UX)*
- `stretch_to`, and `move_*` with `expand`, only grow the bounding box. You can't shrink a selection with Shift+arrows or Shift+click.
- Test (ignored, fails): `selection::shift_click_can_shrink_selection`.

#### SEL-4
**Keyboard and paste handling depend on mouse hover, not focus.** — *major*
- Where: `TableView::show` (`pointer_over_table`).
- Effect: with the pointer over the table, typing `n` in another TextEdit appends rows, `e` starts editing, pasting into another field also pastes into the table, and Ctrl+C copies the table. Moving the mouse away disables table navigation.
- Fix: track table focus (focusable table response or a "table active" flag set on click). Also skip handling when `ctx.wants_keyboard_input()` and the focused widget isn't ours.
- Test (ignored, fails): `keyboard::keyboard_works_without_pointer_over_table`.

#### SEL-5
**Copy uses `modifiers.ctrl`.** — *minor*
- `interaction.rs:15`. Cmd+C on macOS doesn't work (egui issue #4065). Prefer handling `Event::Copy`.
- Test (ignored, fails): `selection::cmd_c_copies_on_mac`.

### Paste

#### PASTE-1
**Row-length check is wrong.** — *major*
- Where: `handle_paste` (`interaction.rs:115-123`).
- `fold` of consecutive differences equals `first_len - last_len`, so ragged middles are reported "equal". `pasting_block_width` takes the max of the last pair only, not the whole block.

#### PASTE-2
**Trailing newline from spreadsheets adds a bogus row.** — *major*
- `text.split('\n')` (`interaction.rs:104`). Excel/LibreOffice always end with `\n`, so every such paste is "N+1 rows with holes" and opens the dialog.
- Quoted cells containing tabs/newlines are split incorrectly. `trim()` also strips meaningful whitespace.
- Fix: parse with `csv::ReaderBuilder` (`\t` delimiter, flexible) and drop a trailing empty record.
- Test (ignored, fails): `paste::paste_with_trailing_newline`.

#### PASTE-4
**Paste dialog state not reset.** — *minor*
- The code resets `create_cols_on_paste` (`interaction.rs:152`), but the checkbox is bound to `create_adhoc_cols_on_paste`, so the previous choice persists. `create_cols_on_paste` is otherwise unused.

### Table view / layout

#### VIEW-5
**Column context menu stubs.** — *minor*
- "Sort ascending/descending" and "Hide" do nothing.

#### VIEW-6
**"Clear" has no confirmation.** — *minor*
- TODO in `tool_column.rs`. A commented-out modal exists in `interaction.rs` (`handle_clear_request`).

#### VIEW-8
**No touch drag-to-scroll.** — *minor*
- Since DESIGN-9 the body is not an egui `ScrollArea`, so dragging the body on a touch screen doesn't scroll. Cells sense clicks only, so a body drag could be mapped to `RowLayout::scroll_by` (or used for drag-selection on desktop and drag-scroll on touch).

#### VIEW-7
**Selection styling uses `warn_fg_color`.** — *minor (visual)*
- Selection fill and borders use the warning color rather than `visuals.selection`. Vertical selection borders are commented out, so multi-column selections have no side edges.

### Import

#### IMPORT-1
**Encoding detection reads nothing.** — *major*
- Where: `util::detect_encoding` (`util.rs:23-27`).
- `Vec::with_capacity` has length 0, so `rdr.read(&mut buf)` returns 0 immediately and chardetng guesses with no data.
- **Caution when fixing:** once it actually reads, `CsvImporter::load` must `seek(0)` again before building the decoder; today it only works because nothing is consumed.

#### IMPORT-2
**No-header CSV drops the last column.** — *data-loss*
- `for col_idx in 0..max_col_idx` (`csv.rs:143`) should be `..=max_col_idx`. Cells for the last column are stored but no column is created for them.

#### IMPORT-3
**Rows wider than the header write into wrong columns.** — *data-loss*
- `.unwrap_or(ColumnUid(csv_idx))` (`csv.rs:123`). Required columns occupy uids `0..n`, so extra cells collide with them (the reader is `flexible(true)`).

#### IMPORT-4
**Header names are not trimmed.** — *minor*
- `" Name"` doesn't match required column `Name`.

#### IMPORT-5
**Separator auto-detect is naive.** — *minor*
- Counts raw bytes including those inside quoted fields. Consider per-line consistency scoring.

### Export

#### EXPORT-1
**Panics on write errors.** — *crash*
- `write_record(..).unwrap()` (`util.rs:59`, `:69`). File-create errors are silently ignored. Should return/report an error.

#### EXPORT-3
**Exports skipped columns.** — *minor*
- Only skipped rows are filtered (`un_skipped_rows`).

#### EXPORT-4
**Blocking file dialog in the UI path.** — *minor*
- `rfd::FileDialog::save_file()` blocks the frame and doesn't work on wasm.

### Documentation

#### DOC-4
**README shortcut list is stale.** — *minor*
- Lists `E` (dropped in favor of F2 by DESIGN-8) and is maintained by hand. Should follow the keyboard reference in this file.

#### DOC-5
**Public API is barely documented.** — *minor*
- `src/table_view.rs` has 17 doc comments, `tabular_core/src/lib.rs` none. No crate-level docs or examples in rustdoc. Target: every public item documented and `#![warn(missing_docs)]` once DESIGN-8 lands.

---

## Design issues and planned rework

### DESIGN-1: Flags system

> Superseded by [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui). Kept for history.

Replace `PersistentFlags` / `OneShotFlags` (FLAGS-1 – FLAGS-5) with:

```rust
// tabular_core — the backend only bumps counters; any number of observers compare against
// their own last-seen value. Nothing is consumed, there is no delay buffer, nothing to clear.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Revision { pub columns: u64, pub rows: u64, pub cells: u64 }

#[derive(Copy, Clone, Default, Debug)]
pub struct Capabilities {
    pub read_only: bool, pub clearable: bool, pub skip_rows: bool, pub skip_cols: bool,
    pub create_rows: bool, pub create_cols: bool, pub get_variant: bool,
}

pub trait TableBackend {
    fn revision(&self) -> Revision;          // replaces all one-shot flag methods
    fn capabilities(&self) -> Capabilities;  // replaces PersistentFlags + is_clearable()
    // loading / uncommitted / collision status as a separate `fn status()` when remote backends exist
}
```

- `TableView` stores `last_seen: Revision` and rebuilds column info / row order on change.
  Safety net: also resync when the view's row count differs from `row_count()`, so a backend that
  forgets to bump a counter shows stale data instead of panicking.
- App code keeps its own `last_seen` the same way.
- View-originated events move to the return value of `show()`:

  ```rust
  pub struct TableViewOutput { pub response: Response, pub events: Vec<TableEvent> }
  pub enum TableEvent {
      SelectionChanged(Vec<RowUid>), CellCommitted(CellCoord), EditCancelled(CellCoord),
      ColumnMappingChanged(ColumnUid), ColumnsReordered, RowsCreated(Vec<RowUid>), ...
  }
  ```
- The derive macro then needs no flag fields at all.
- Breaking change: bundle with DESIGN-3 and the unused-API cleanup.

### DESIGN-2: Edit lifecycle

> Superseded by [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui): the view owns the edit buffer, so the backend has no `begin_edit`/`end_edit`.

Fixes EDIT-1 – EDIT-3, EDIT-6.

- Single owner of "is editing": the view. Add a private `fn end_edit(&mut self, table, commit: bool)`
  used by **every** exit path: click elsewhere, header or tool-column click, Escape, Enter, Tab,
  arrows, focus loss, row/column set change, `clear()`.
- Make the lifecycle explicit in `TableFrontend`:
  `begin_edit(coord)` → `show_cell_editor(coord, ui) -> EditorResponse` → `end_edit(coord, commit)`.
  The view guarantees exactly one `end_edit` per `begin_edit`. The backend no longer infers "first
  pass" from a stale buffer; `begin_edit` is the moment to load the value and request focus.
- `EditorResponse { response, commit_requested: bool }` lets discrete editors (Bool, Enum) commit on change.

### DESIGN-3: `TableBackend::col_uid` conflicts with view-owned column order

> Superseded by [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui), which removes both `col_uid` and `row_uid` from the trait.

The view owns visual column order (`columns_ordered`, drag & drop). The backend's
`col_uid(VisualColIdx)` is a second, conflicting ordering (root of EDIT-4). Remove it from the
trait; all visual→uid lookups go through view state.

### DESIGN-4: Column order and widths as persisted view state

- Store `column_order: Vec<ColumnUid>` and `column_widths: HashMap<ColumnUid, f32>` in
  `TableViewConfig`.
- On column set change, keep known columns in user order and append new ones.
- Use move (insert) semantics for DnD (DND-1, DND-5). Widths are already keyed by `ColumnUid`
  in view state (`ColumnWidths`, DESIGN-9); this step persists them.

### DESIGN-5: Focus-based input

> Absorbed into [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui) ("Focus and input").

Track table focus instead of pointer hover (SEL-4). Only handle shortcuts and paste when the table is
focused and no foreign widget wants keyboard input.

### DESIGN-6: Builder-style `show`

> Superseded by [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui) (`TableViewOptions`).

`show(table, config, max_height, ui, id)` is growing. Consider
`TableView::new(id).max_height(..).show_tool_column(..).show(ui, table, config)`, with `id`
salting all view ids (VIEW-3, already fixed by DESIGN-9).

### DESIGN-7: Optional heavy dependencies

`rfd` (GTK/portal, file dialogs) is a hard dependency of a widget crate. Put import/export UI behind
a feature (e.g. `file-dialogs`), and expose export as `fn write_csv(table, impl Write) -> Result`.

### DESIGN-8: Core contract rework (`TableModel` + `CellUi`)

**Status:** accepted 2026-10-01; implementation is roadmap steps 1–2. **Breaking.** It supersedes
DESIGN-1, DESIGN-2, DESIGN-3 and DESIGN-6, and absorbs DESIGN-5. Every known downstream user will
be ported, so there are no compatibility shims.

**Amended 2026-10-01 (gap review):** optional `get` and per-column degradation for models without
values (DESIGN-11), typed column type, more capabilities and commands, row builder `CellUi::show_row`
(DESIGN-10), `CellUi::text`, column selections, programmatic control, the row uid contract,
`TableEvent::Message`, and undo inverses from the first implementation. The code blocks below
include the amendments.

**Step 1 implemented 2026-10-01.** Where the code differs from the text below:

- Added only what is breaking or used now. Defaulted methods that nothing calls yet come with the
  step that uses them: `rows_log` (step 4), `sorted_rows` (sorting), `rename_column` and
  `Capabilities::rename_columns` (step 5). Same for `TableViewOptions::{stick_to_bottom,
  sort_policy, style}`, `CellMetadata::{loading, changed}` and the programmatic control API (step 2).
- `TableCommand`: no `SetMany`; `Batch(Vec<TableCommand>)` instead, which is also the form of
  compound inverses. `Paste` carries the resolved target (`rows`, `create_rows`, `columns`,
  `create_columns`, `block`, `repeat`) instead of an anchor and a mode; the paste dialog still
  computes it (PASTE-1, PASTE-2, PASTE-4 remain). `CommandFailed` boxes the command.
- `ModelError::NotFound` was added. `Capabilities` has `NONE`, `ALL` and `read_only()`;
  `ColumnInfo` has builder methods and `type_text()`.
- The view already owns the edit buffer (`State::edit`, part of step 2): `begin_edit` →
  `show_editor` → `TableCommand::Set` on commit. Selection is still index-based (`SelectedRange`).
- Queued commands are applied twice per frame: after input handling (so the body shows the result,
  e.g. Tab after a commit) and after drawing.
- `CellUi` has default `show_cell`/`show_editor` (the built-in viewer and editor), so
  `VariantCellUi` is an empty impl, and custom UIs get editing of `Variant` values for free.

#### Principles

1. **The model is data only.** `TableModel` lives in `tabular_core` and has no egui dependency.
2. **The view owns all presentation state:** column order and widths, row order (sort/filter),
   selection, the edit buffer, focus, and the paste dialog. Nothing the user sees is ordered by the
   model.
3. **The view never mutates the model while rendering.** Every view-originated change is pushed to a
   command queue, and the queue is applied once per frame, after the body has rendered. This prevents
   VIEW-1-style desyncs by construction (VIEW-1 itself was fixed by DESIGN-9) and is the hook for undo/redo.
   **The one exception** is custom cell UI: `CellUi` gets `&mut M` so a cell can host a live
   widget (e.g. a register write button, U3). Such writes bypass the queue and undo, and must not
   change the row or column set during rendering; a model that needs that queues it internally.
4. **Change detection uses revision counters that are never consumed.** Any number of views and app
   code can each compare against their own last-seen value.
5. **Values are optional.** A model may render purely through a custom `CellUi` and return no
   `Variant`s. Features that need values degrade per column (see
   [Goals](#goals-use-cases-and-targets), "Feature availability").
6. **Row uids are stable identities.** A `RowUid` names the same row for its whole life and is never
   reused for another row within the model's life (selection, scroll anchor, height cache and undo
   key on it). A reload that creates new rows gives them new uids, so selection and scroll position
   are dropped; a model that wants them kept across reloads keeps the uids.

#### Core types (`tabular_core`)

```rust
/// Bumped by the model on change. Compare with your last-seen copy; never reset.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Revision {
    pub columns: u64, // column set, names, types, ColumnInfo changes
    pub rows: u64,    // row set (insert, remove, clear, reload)
    pub cells: u64,   // any cell value
    pub skips: u64,   // row/column skip state; separate so preprocessing that skips doesn't loop
}

/// What the model supports. The view only issues commands that are allowed here.
#[derive(Copy, Clone, Default, Debug)]
pub struct Capabilities {
    pub edit_cells: bool,
    pub create_rows: bool,
    pub insert_rows: bool,     // at a position, not only append
    pub remove_rows: bool,
    pub create_columns: bool,
    pub remove_columns: bool,
    pub rename_columns: bool,
    pub clear: bool,
    pub skip_rows: bool,
    pub skip_columns: bool,
}

/// Replaces BackendColumn (renamed; `is_skipped` is the only source of truth for column skip).
pub struct ColumnInfo {
    pub name: String,
    pub synonyms: Vec<String>,
    /// Value type of the column. Picks the editor for empty cells, alignment and comparison.
    /// None = custom column without values (DESIGN-11): not sortable, not editable by VariantCellUi.
    pub ty: Option<VariantTy>,
    /// Shown in the header; defaults to `ty`'s name. Free text, e.g. "u8 (CAN DLC)".
    pub type_label: Option<String>,
    pub is_sortable: bool,
    pub is_required: bool,
    pub is_used: bool,
    pub is_skipped: bool,
}

pub enum ModelError { Unsupported, TypeMismatch { expected: VariantTy }, Other(String) }

/// Where a new row goes.
pub enum RowPosition { Append, Before(RowUid), After(RowUid) }

pub trait TableModel {
    fn revision(&self) -> Revision;
    fn capabilities(&self) -> Capabilities;

    /// Natural column order. The view starts from this and then applies the user's order.
    fn columns(&self) -> impl Iterator<Item = ColumnUid>;
    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo>;
    /// Natural row order (insertion / file order). Never sorted or filtered by the view's state.
    fn rows(&self) -> impl Iterator<Item = RowUid>;
    fn row_count(&self) -> usize { self.rows().count() } // override when O(1)
    /// Optional counters that let the view update appends and front evictions in O(changed)
    /// instead of rebuilding (live data, DESIGN-11). None = rebuild on every `revision.rows` change.
    fn rows_log(&self) -> Option<RowsLog> { None }

    /// `Cow` so that computed models (derive macro) can return owned values and stored
    /// models can return references (sorting 1M rows must not clone every string).
    /// Optional: models that render only through a custom CellUi return None (DESIGN-11).
    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> { None }
    /// Batched form for copy/export/sort: one lookup per row (DESIGN-10). Default calls `get`.
    fn row_values(&self, row: RowUid, cols: &[ColumnUid], out: &mut Vec<Option<Variant>>) { /* get() per col */ }
    fn metadata(&self, coord: CellCoord) -> Option<Cow<'_, CellMetadata>> { None }
    fn is_row_skipped(&self, row: RowUid) -> bool { false }

    // Mutations. Defaults return Err(Unsupported); the view checks capabilities() first.
    fn set(&mut self, coord: CellCoord, value: Variant) -> Result<(), ModelError>;
    fn create_row(&mut self, at: RowPosition, values: Vec<(ColumnUid, Variant)>) -> Result<RowUid, ModelError>;
    fn remove_rows(&mut self, rows: &[RowUid]) -> Result<(), ModelError>;
    fn create_column(&mut self) -> Result<ColumnUid, ModelError>;
    fn remove_columns(&mut self, cols: &[ColumnUid]) -> Result<(), ModelError>;
    fn rename_column(&mut self, col: ColumnUid, name: String) -> Result<(), ModelError>;
    fn clear(&mut self) -> Result<(), ModelError>;
    fn skip_rows(&mut self, rows: &[RowUid], skipped: bool) -> Result<(), ModelError>;
    fn skip_column(&mut self, col: ColumnUid, skipped: bool) -> Result<(), ModelError>;

    /// Optional delegated ordering, see "Row order". None = the view sorts locally.
    fn sorted_rows(&self, keys: &[SortKey]) -> Option<Vec<RowUid>> { None }
}
```

`CellMetadata` replaces `color`/`corner: Option<Rgb>` with a semantic `level: Option<CellLevel>`
(`Info`, `Warning`, `Error`, `Changed`, `Custom(Rgb)`) mapped to the theme by the view (VIEW-11,
DESIGN-14). A `loading: bool` marks cells of lazy models (U5) that the view draws as placeholders.

Removed from the contract: `PersistentFlags`, `OneShotFlags` and the five flag methods,
`col_uid(VisualColIdx)`, `row_uid(VisualRowIdx)`, `VisualRowIdx`/`VisualColIdx` (they become
view-internal), `available_columns`/`used_columns`, `is_clearable`, `is_col_skipped`,
`un_skip_all_*` (use the slice forms), `commit_cell_edit`, `column_mapping_choices` (moves to view
options), and `set_metadata` (becomes inherent on `VariantTable`; the trait only reads metadata).
`un_skipped_rows()` stays as a provided method.

#### Cell UI (`egui_tabular`)

```rust
pub trait CellUi<M: TableModel> {
    /// View mode for one visible row. Look the row up once, then fill cells by column
    /// (DESIGN-10). The default calls `show_cell` for each cell the view wants.
    fn show_row(&mut self, model: &mut M, row: RowUid, cells: &mut RowCells<'_>) { /* ... */ }
    /// View mode for one cell. `&mut M` because cells may host live interactive widgets
    /// (e.g. hardware state polled through the model). Implement this or `show_row`.
    fn show_cell(&mut self, model: &mut M, coord: CellCoord, ui: &mut Ui) {}
    /// Plain text for copy, export and find when `model.get()` has no value (DESIGN-11).
    fn text(&self, model: &M, coord: CellCoord) -> Option<String> { None }
    /// Initial edit value; None = this cell is not editable.
    fn begin_edit(&mut self, model: &M, coord: CellCoord) -> Option<Variant> {
        model.get(coord).map(Cow::into_owned)
    }
    /// Edits the view-owned buffer. Never touches the model.
    fn show_editor(&mut self, model: &M, coord: CellCoord, value: &mut Variant, ui: &mut Ui)
        -> EditorResponse;
    /// Extra header content (replaces TableFrontend::custom_column_ui).
    fn header_ui(&mut self, model: &mut M, col: ColumnUid, ui: &mut Ui) {}
}

pub struct EditorResponse { pub response: Response, pub commit: bool } // commit: discrete editors

/// Default implementation for any model: renders and edits every Variant type.
pub struct VariantCellUi;
impl<M: TableModel> CellUi<M> for VariantCellUi { /* ... */ }
```

- The model and the cell UI are **separate values**, so `show(&mut model, &mut cell_ui)` borrows
  cleanly. A model with custom rendering uses a small companion type:
  `struct BanksUi; impl CellUi<Banks> for BanksUi`.
- The **view** paints background color, corner, tooltips (from `model.metadata()`) and the skipped
  strike-through. `cell_color`/`cell_tooltips`/`cell_corner` leave the frontend, so custom cell UIs
  get them for free.
- The view requests focus on the editor's first frame, and handles Enter/Escape/Tab/lost focus
  uniformly, so `CellUi` implementors don't reimplement the lifecycle. The editor is drawn in an
  overlay above the cell ([DESIGN-12](#design-12-editor-overlay)).

#### View

```rust
pub struct TableViewOptions {
    pub id_salt: Id,              // salts every view id, so several tables can share a Ui
    pub max_height: Option<f32>,
    pub tool_column: bool,
    pub read_only: bool,          // view-level, on top of model capabilities
    pub stick_to_bottom: bool,
    pub column_mapping_choices: Vec<String>,
    pub sort_policy: SortPolicy,  // DESIGN-13
    pub style: TableStyle,        // DESIGN-14
}

impl TableView {
    pub fn new(options: TableViewOptions) -> Self;
    pub fn options_mut(&mut self) -> &mut TableViewOptions;
    pub fn show<M: TableModel>(
        &mut self, ui: &mut Ui, model: &mut M, cell_ui: &mut impl CellUi<M>,
        config: &mut TableViewConfig,
    ) -> TableViewOutput;
    pub fn selection(&self) -> Option<&Selection>;
    pub fn visible_rows(&self) -> &[RowUid];
    pub fn visible_columns(&self) -> &[ColumnUid];

    // Programmatic control (applied at the start of the next `show`).
    pub fn set_selection(&mut self, selection: Option<Selection>);
    pub fn scroll_to(&mut self, row: RowUid, align: Option<Align>); // None = minimal reveal
    pub fn scroll_to_column(&mut self, col: ColumnUid);
    pub fn start_edit(&mut self, coord: CellCoord);
    pub fn cancel_edit(&mut self);
    pub fn request_focus(&mut self);
}

/// User preferences; serde, owned and persisted by the app.
pub struct TableViewConfig {
    pub minimum_row_height: Option<f32>, // None = ui.spacing().interact_size.y (EDIT-5)
    pub heterogeneous_row_heights: bool,
    pub column_order: Vec<ColumnUid>,             // DESIGN-4
    pub column_widths: HashMap<ColumnUid, f32>,   // DESIGN-4
    pub hidden_columns: HashSet<ColumnUid>,
    pub sort: Vec<SortKey>,
    pub column_mapped_to: HashMap<ColumnUid, String>,
}

pub struct TableViewOutput { pub response: Response, pub events: Vec<TableEvent> }

pub enum TableEvent {
    SelectionChanged { rows: Vec<RowUid> },
    CellCommitted(CellCoord),
    EditCancelled(CellCoord),
    CommandFailed { command: TableCommand, error: ModelError },
    ColumnMappingChanged(ColumnUid),
    ColumnsReordered,
    SortChanged,
    RowsCreated(Vec<RowUid>),
    /// Feedback for the user ("Nothing selected to paste into", "3 cells failed to parse").
    Message { level: CellLevel, text: String }, // VIEW-10
}
```

**Command queue.** Commands are view-level intents, applied in one place after rendering:

```rust
pub enum TableCommand {
    Set { coord: CellCoord, value: Variant },
    Paste { anchor: (RowUid, ColumnUid), block: Vec<Vec<String>>, mode: PasteMode },
    SetMany(Vec<(CellCoord, Variant)>), // Delete/clear, Ctrl+Enter, fill: one undo step
    CreateRows { at: RowPosition, count: usize },
    CreateColumn,
    RemoveRows(Vec<RowUid>),
    RemoveColumns(Vec<ColumnUid>),
    RenameColumn { col: ColumnUid, name: String },
    Clear,
    SkipRows { rows: Vec<RowUid>, skipped: bool },
    SkipColumn { col: ColumnUid, skipped: bool },
}
```

`Paste` is one compound command because it may create columns and then write into them. Its apply
step calls `create_column`, `create_row` and `set` in sequence.

**Undo from the start.** `apply` returns the inverse command (`Set` returns the old value,
`CreateRows` returns `RemoveRows`, ...) from the first implementation in step 1, even before an undo
UI exists. Retrofitting inverses later means revisiting every command. `RemoveRows`/`RemoveColumns`/
`Clear` inverses need the removed data, so their inverse is only available when the model can
re-insert with the same uids; otherwise they clear the undo stack (and the UI says so). Possible later
extension: a `defer_commands` option returns the queue in `TableViewOutput` instead of applying it,
for apps that route edits to a server.

**Row order.**

- The view holds `visible_rows: Vec<RowUid>`. It is rebuilt when `revision.rows` changes, when
  `config.sort` or the filter changes, or when `revision.cells` changes while a sort or filter is
  active and `SortPolicy` allows it (edits made in this view never re-sort,
  [DESIGN-13](#design-13-sort-and-filter-stability-and-ui)). The pipeline is: `model.rows()` → filter → sort (stable). Sorting uses `model.get()` and
  `rvariant::Variant::sort_cmp` (rvariant 0.3; the derived `Ord` compares by variant first, so
  `U32(5)` vs `I64(3)` would be wrong). Check that numbers compare by value, strings
  case-insensitively, and empty values sort last; wrap it if not.
- `visible_rows` is `RowLayout`'s row order ([DESIGN-9](#design-9-anchor-based-layout-without-egui_extras)).
  Row heights are cached there by `RowUid`, and the scroll anchor follows its row through re-sorts.
- **Delegated sorting hook:** if `model.sorted_rows(&config.sort)` returns `Some`, the view uses
  that order instead of sorting locally (e.g. a model backed by SQL runs `ORDER BY`). This is one
  optional trait method. The view's structure doesn't change, because it still holds a `Vec<RowUid>`.
- **Known limit:** the view materializes all row uids (4 bytes per row; 1M rows ≈ 4 MB, local sort
  in the order of 100 ms). Tables that can't enumerate their uids (unbounded or paginated remote
  data) are out of scope. Supporting them later would turn `visible_rows` into an enum
  (`Local(Vec)` / model-driven paging) inside the view's row-order module, plus one more optional
  trait method. It would not need a redesign.

**Selection.** `Selection { anchor: (RowUid, ColumnUid), cursor: (RowUid, ColumnUid), kind: Cells | Rows | Columns }`.
`Columns` covers all visible rows of the anchor..cursor columns (header click, Ctrl+Space); copy,
paste, clear and hide work on it.
It is resolved to a visual rectangle each frame through uid→index maps that are built along with
`visible_rows`/`visible_columns`. If the anchor or cursor disappears, the selection is clamped, or
dropped if the table is empty. Shift-extend moves the cursor only (SEL-3). Column moves need no
selection fix-up (DND-3). Row selection covers exactly the visible columns (SEL-1).

**Edit lifecycle.** The view owns the edit buffer:
`editing: Option<EditState { coord, value: Variant, first_frame: bool, error: Option<ModelError> }>`.

- **Enter edit mode:** click on the selected cell, double-click, Enter, F2, or typing a character
  (a `Str` editor is seeded with that character). The `E` shortcut is dropped in favor of F2. Entry
  calls `cell_ui.begin_edit`.
- **Exit:** every exit goes through one `finish_edit(Commit | Cancel)`.
  - **Commit:** Enter, Tab (and continue on the next visible cell), click elsewhere, focus loss, or
    `EditorResponse::commit`. Pushes `TableCommand::Set`. If `set` fails, the editor reopens with
    the error shown as a tooltip.
  - **Cancel:** Escape, or the edited row/column disappearing.

  Because the buffer lives in the view, where the pointer is doesn't matter (EDIT-1, EDIT-2, EDIT-3,
  EDIT-6).

**Focus and input** (was DESIGN-5). The table has a focusable response and is *active* while it or
its editor has focus. Shortcuts run only when the table is active. Copy and paste use
`Event::Copy`/`Event::Paste` (SEL-4, SEL-5).

#### Implementations after the change

- **`VariantBackend` → `VariantTable`:** implements `TableModel` only and is rendered with
  `VariantCellUi`. `insert_column` takes a `ColumnDef` struct instead of 7 positional arguments.
  `set` coerces the value to the column type (PASTE-5). Conversion errors surface through
  `metadata()`. `set_read_only` sets the capabilities (BACKEND-1).
- **`#[derive(TabularRow)]`:** generates `<vis> struct <Row>Table` (DERIVE-2) implementing
  `TableModel` with `get()`. A field becomes `Variant` via `From` where rvariant has an impl,
  otherwise the `#[format]`/`Debug` string. All capabilities are false. No egui in the generated code
  (DERIVE-3), and it is rendered with `VariantCellUi`. Copy/export work (DERIVE-4).
- **`TabularImporter`:** `show` returns `TableViewOutput`. Apps detect data changes with
  `importer.table().revision()`.

#### Migration (old → new)

| Old | New |
|-----|-----|
| `impl TableBackend + TableFrontend for T` | `impl TableModel for T` + `impl CellUi<T> for TUi` (or use `VariantCellUi`) |
| `one_shot_flags().rows_selected` | `TableEvent::SelectionChanged` in `show()` output, or `view.selection()` |
| `one_shot_flags().any_changed()` | Compare `model.revision()` with a stored copy |
| `one_shot_flags_internal_mut().row_set_updated = true` | Bump `revision.rows` |
| `persistent_flags().is_read_only` | `capabilities()` / `TableViewOptions::read_only` |
| `row_uid(VisualRowIdx(i))` for iteration | `model.rows()` (natural order) or `view.visible_rows()` (on-screen order) |
| `view.show(&mut t, &mut cfg, max_h, ui, id)` | `view.show(ui, &mut t, &mut VariantCellUi, &mut cfg)`; `id`/`max_h` move to `TableViewOptions` |
| `TableBackend::poll` | Inherent method on the model, called by the app |
| `backend.set_mapping_choices(..)` | `view.options_mut().column_mapping_choices` |
| `TableFrontend::show_cell_view` that looks up the row per cell | `CellUi::show_row`: one lookup, then `cells.cell(col, \|ui\| ..)` (DESIGN-10) |
| `cell_color` / `cell_corner` returning `Color32` | `CellMetadata::level` (`CellLevel`), or `cells.level(col, ..)` from `show_row` |
| `BackendColumn::ty: String` | `ColumnInfo::ty: Option<VariantTy>` + `type_label` |
| `VariantBackend::new([(name, ty, default)])` | `VariantTable::new([ColumnDef::new(name, ty).default(v)])` |
| `insert_column(uid, name, synonyms, ty, default, required, used)` | `insert_column(uid, ColumnDef::new(..).synonyms(..).required(..).used(..))` |
| `TableView::new()` | `TableView::new(TableViewOptions::default())` |
| `importer.show(config, max_h, ui, id)`, `importer.backend()` | `importer.show(config, ui) -> TableViewOutput`, `importer.table()`; options via `importer.table_view.options_mut()` |
| `#[derive(TabularRow)] struct Row` → `RowTabularBackend` | `RowTable` (same visibility as `Row`) |
| `set_metadata(coord, CellMetadata::new().color(Rgb::ORANGE))` | `CellMetadata::new().level(CellLevel::Warning)` (`CellLevel::Custom(rgb)` for a fixed color) |

### DESIGN-9: Anchor-based layout without egui_extras

**Status:** accepted and implemented 2026-10-01. Fixes VIEW-1, VIEW-2, VIEW-3, DND-2.

egui_extras' `TableBuilder` needed the total content height up front (a row count plus either one
row height or a height per visual row, walked every frame), kept column widths by position, and
didn't expose what the view needed (per-uid widths, scroll position, stable ids). The view now lays
out the table itself.

- **Scroll anchor.** The vertical position is `(RowUid, offset into that row)`, not a pixel offset
  from the top. Each frame `RowLayout::normalize` applies scrolling and clamps, then the body lays
  out rows downwards from the anchor until the viewport is full. Cost is O(visible rows), and the
  total pixel height of the table is never needed.
- **Row heights** are measured every frame for visible rows and cached by `RowUid`; unmeasured rows
  are assumed to be `minimum_row_height`. Rows below a measured row are placed after it in the same
  frame, so there is no one-frame overflow. Backgrounds are painted into shape placeholders after
  the row height is known.
- **Stable scrolling.** Inserting or removing rows above, and (later) re-sorting or filtering, keep
  the anchored row on screen. If the anchor row disappears, the view stays at its index.
- **Stick to bottom.** If the last row was in view, appended rows keep the view at the end.
- **Reveal.** `RowLayout::reveal(idx)` scrolls minimally to bring a row fully into view; arrow keys
  use it.
- **Scroll bar** is drawn by the view and works in rows: thumb length = rows in view / row count,
  position = fractional index of the top row. With very uneven row heights the thumb speed varies
  slightly. Mouse wheel delta is consumed only when the table actually scrolled.
- **Columns.** `ColumnWidths` keyed by column (`ColumnKey::Tool` / `Data(ColumnUid)`), so widths
  follow DnD moves. Auto columns grow to the widest content seen (cells are measured with up to
  400 px), user resizing fixes the width, double-click on the edge resets to auto. Horizontal
  scrolling still uses egui `ScrollArea::horizontal`; the header is outside the vertical scroll, so
  it is always visible.
- **Cells** are child `Ui`s with a click sense registered below their contents (like egui_extras),
  laid out top-down, clipped to their column (and to the row in uniform-height mode).
- **Pixel alignment.** All layout coordinates (scroll offset, row and header heights, column widths
  and x positions) are rounded with `round_ui()`. Otherwise egui debug builds paint "Unaligned"
  markers on every cell.
- **Column width easing.** `ColumnWidths` holds the target width; the shown width eases towards it
  with `animate_value_with_time` for auto-sizing changes only (not for user drags or the first
  measurement). A re-fit (double-click) keeps the old width until the new one is measured.
- **Row set changes** take an incremental path when rows were only appended, so pressing `N` in a
  10k-row table doesn't rebuild the uid index. The view still collects every uid from the model
  into a new `Vec` first, so the change itself is O(row count); DESIGN-11 removes that for live data.

**Fits DESIGN-8:** `RowLayout::rows` is the view-owned row order. Sorting/filtering (step 2) only
replaces how that `Vec<RowUid>` is built. Paged or unbounded models (the "known limit" in DESIGN-8)
would replace the `Vec` with model-driven next/prev stepping from the anchor; the layout loop
already only walks outwards from the anchor.

**Not done yet:** horizontal reveal, Page Up/Down/Home/End, touch drag-to-scroll ([VIEW-8](#view-8)),
persisting widths ([DESIGN-4](#design-4-column-order-and-widths-as-persisted-view-state)), a sticky
tool column during horizontal scroll.

### DESIGN-10: Row builder for cell UI

**Status:** implemented 2026-10-01 in roadmap step 1. Differences: `RowCells::columns()` returns
`&'a [ColumnUid]` (not tied to the `RowCells` borrow, so `cell()` can be called while iterating)
instead of an `Rc` handle. Until the editor overlay (DESIGN-12) the edited cell is not given to
`show_row` (`wants` is false); the view draws the editor into it.

**Problem.** A custom frontend gets one call per cell (`show_cell_view(coord)`). When the row lives
in an in-memory database, a decoded frame or any struct that is costly to find, every cell repeats
the lookup: 10 visible columns = 10 queries per row per frame. Caching in the frontend is fragile
(invalidation, borrowing the model). egui_extras' `TableRow::col` avoids the repeat lookup, but it
is positional, so it breaks with user column order and hidden columns.

**Design.** The view calls `CellUi::show_row` once per visible row and hands it a `RowCells`
builder. The implementation looks the row up once and fills cells **by `ColumnUid`, in any order**.
The view places each cell in its column, whatever the visual order.

```rust
/// One per visible row, created by the view. Holds the child `Ui`s of the row's visible cells.
pub struct RowCells<'a> { /* view-private */ }

impl RowCells<'_> {
    pub fn row(&self) -> RowUid;
    /// Visible columns of this row in visual order. A cheap shared handle (`Rc<[ColumnUid]>`),
    /// so `cell()` can be called while iterating.
    pub fn columns(&self) -> VisibleColumns;
    /// True if the column is shown now (not hidden, not scrolled out horizontally). Use it to skip
    /// expensive work, e.g. decoding a signal for a column that isn't on screen.
    pub fn wants(&self, col: ColumnUid) -> bool;
    /// Fill one cell. Any order. A column that isn't shown is a no-op and `add` isn't called.
    /// Filling the same column twice appends to the cell.
    pub fn cell<R>(&mut self, col: ColumnUid, add: impl FnOnce(&mut Ui) -> R) -> Option<R>;
    /// Per-cell presentation from the same lookup, overriding `model.metadata()` for this frame.
    pub fn level(&mut self, col: ColumnUid, level: CellLevel);
    pub fn tooltip(&mut self, col: ColumnUid, text: impl Into<String>);
}

// Default: per-cell, so simple implementations only write `show_cell`.
fn show_row(&mut self, model: &mut M, row: RowUid, cells: &mut RowCells<'_>) {
    for col in cells.columns().iter() {
        cells.cell(*col, |ui| self.show_cell(model, CellCoord { row_uid: row, col_uid: *col }, ui));
    }
}
```

Example, a CAN trace:

```rust
impl CellUi<Trace> for TraceUi {
    fn show_row(&mut self, trace: &mut Trace, row: RowUid, cells: &mut RowCells<'_>) {
        let Some(frame) = trace.frame(row) else { return };   // the only lookup
        cells.cell(COL_TIME, |ui| ui.label(format!("{:.6}", frame.t)));
        cells.cell(COL_ID, |ui| ui.monospace(format!("{:03X}", frame.id)));
        if frame.dlc > 8 { cells.level(COL_DLC, CellLevel::Error); }
        if cells.wants(COL_SIGNALS) {
            let signals = self.dbc.decode(frame);            // only when on screen
            cells.cell(COL_SIGNALS, |ui| signals.ui(ui));
        }
    }
}
```

- **Unfilled cells** are drawn empty: background, selection, metadata and click sense still work,
  because the view creates the cell `Ui`s and their click sense before calling `show_row`. Row
  height measurement and placeholder backgrounds are unchanged (DESIGN-9).
- **Editing** doesn't need a special case: the editor is an overlay (DESIGN-12), the cell under it
  is still rendered.
- **Borrowing:** `frame` borrows the model immutably. A cell that needs `&mut M` (interactive
  widget writing to the device) copies what it needs out of `frame` first, or records an action
  that the implementation applies after the last `cell()` call.
- **Model side:** the same repeat-lookup problem exists for copy, export and sort, which call
  `get()` per cell. `TableModel::row_values(row, cols, out)` is the batched form (default: `get` per
  column). Copy/export call it once per row. Sort keeps calling `get` (one column).
- **Metadata:** the view calls `model.metadata()` per visible cell. A model with a costly lookup
  returns `None` there and sets level/tooltip from `show_row` instead.
- **Derive macro:** generates `row_values` that reads each struct once.
- **Tests:** a UI test with a counting model checks one lookup per visible row per frame, and
  that cells land in the right columns after a column move and with a hidden column.

### DESIGN-11: Live data and models without `Variant` values

**Status:** proposed 2026-10-01; contract parts in roadmap step 1, the rest in step 4. Use case U3.

**Goal.** Bus traces (CAN, I2C, USART, SPI), register maps and decoded signals, backed by the app's
own structs, updated many times a second. The model may have no `Variant` values at all. Features
that need values degrade per column (see the availability table in
[Goals](#goals-use-cases-and-targets)); everything that doesn't need them works fully.

**Values are optional.**

- `TableModel::get` defaults to `None`. `ColumnInfo::ty: None` marks a custom column: no sort, no
  built-in editor, no paste or clear; header and context menu disable those entries instead of
  hiding them, with a hover text saying why.
- `CellUi::text(model, coord)` gives a string for copy, export and find when there's no value. The
  derive macro and `VariantCellUi` don't need it.
- Paste into custom columns is refused with a `TableEvent::Message`.

**Cheap row-set updates.** A trace appends rows every frame and evicts the oldest from the front.
Today that costs O(row count) per frame (DESIGN-9 note).

```rust
/// Monotonic counters. The view keeps the last-seen copy and derives what changed.
pub struct RowsLog {
    pub evicted_front: u64, // total rows ever removed from the front of the natural order
    pub appended: u64,      // total rows ever appended at the end
    pub other: u64,         // bumped by any other row-set change (insert, remove, clear, reload)
}
```

- If `other` didn't change, the view removes `evicted_front - last.evicted_front` rows from the
  front and asks for the last `appended - last.appended` uids (`rows()` is double-ended or a new
  `rows_from_end(n)` method). Otherwise it rebuilds as today.
- `RowLayout` keeps rows in a `VecDeque` with an index offset, so front removal is O(evicted). The
  height cache drops evicted uids.
- With a sort or filter active, appended rows are filtered and, under `SortPolicy::Live`, inserted
  by binary search; evicted rows are removed from the sorted order (O(n), acceptable) or the order
  is rebuilt.

**Row uids.** `RowUid` becomes `u64`. Uids are never reused (DESIGN-8 principle 6); a 1 kHz bus
overflows `u32` in about 50 days, an 8 kHz bus in about 6.

**Scrolling.** Stick to bottom (`TableViewOptions::stick_to_bottom`) keeps the newest row in view
while rows are evicted. When scrolled up, the anchor keeps its row until that row is evicted, then
stays at index 0. Wheel scrolling never fights with stick to bottom: scrolling up turns it off until
the end is reached again.

**Repaint.** The view can't know that a bus thread received data. The app calls
`ctx.request_repaint()` from the receiving side (document this in the live demo). The view only
requests repaints for its own animations.

**Change highlight.** `CellMetadata::changed: Option<u64>`: the `revision.cells` value at which the
cell last changed. The view records when it first saw each revision and fades a `Changed` level
over `TableStyle::change_flash`. The model stays time-free and egui-free.

**Interactive cells.** `CellUi` gets `&mut M` (the DESIGN-8 principle 3 exception), so a cell can
host a button or a drag value that writes a register. Such writes are not undoable and are not
commands.

**Pause (idea).** A view-side "freeze" keeps the current `visible_rows` snapshot while the model
keeps growing, with a "1 234 new rows" badge to resume.

**Demo.** `demos/live`: a simulated CAN bus thread (random ids, a few periodic frames with
changing payloads), a trace table with stick to bottom and eviction at 100k rows, and a register
map table with a writable column. It doubles as the performance benchmark scenario.

### DESIGN-12: Editor overlay

**Status:** proposed 2026-10-01; roadmap step 2 (with the view-owned edit buffer). Fixes EDIT-5.

Draw the editor in a layer above the table (an `egui::Area` anchored to the cell rect), as Excel and
Glide Data Grid do, instead of inside the cell's `Ui`.

- The row height never changes when editing starts or ends, so heterogeneous rows don't jump and
  fixed-height rows don't clip (EDIT-5).
- The overlay is at least the cell's size and may grow right and down (long text, multi-line with
  Alt+Enter), limited to the table's visible area.
- Editors are frameless with the same padding as cell text, so text doesn't shift when editing
  starts.
- It follows the cell when scrolling. If the cell scrolls out of view, editing continues and the
  next keystroke reveals the cell again.
- A click outside the overlay commits (DESIGN-8 edit lifecycle). ComboBox popups opened from the
  overlay are above it, so they work as today.
- Tests find the editor as today (`TextInput` node); add one that checks row heights don't change
  when editing starts.

### DESIGN-13: Sort and filter: stability and UI

**Status:** proposed 2026-10-01; policy in roadmap step 2, UI in step 5.

**Stability policy.** Re-sorting or re-filtering immediately after a value changes makes rows jump
away under the user: edit a cell in a sorted column and the row vanishes. AG Grid, Google Sheets
and Airtable keep the row in place until the sort is applied again.

```rust
pub enum SortPolicy {
    /// Default. Edits made in this view never move or hide rows; changes from outside the view
    /// (revision.cells bumped without a view command) re-sort and re-filter.
    OnExternalChange,
    /// Re-sort/filter on every change, including this view's edits (live "top N" views, U3).
    /// The scroll anchor stays at its index instead of following its row.
    Live,
    /// Only when the user or app re-applies. The header shows an "out of date" marker with a
    /// refresh action.
    OnDemand,
}
```

- A row that no longer matches the filter after an edit stays visible, marked, until the filter is
  re-applied.
- The view tells its own edits from outside changes by remembering the `revision.cells` it expects
  after applying its commands.

**Sort UI.** A sort indicator at the right edge of each sortable header: click cycles ascending,
descending, none; Shift+click adds a secondary key (shows 1, 2, ...). The header body selects the
column (keyboard table). Context menu: Sort ascending / descending / Clear sort. Columns without
values have the indicator disabled.

**Find and filter share one bar.** Ctrl/Cmd+F opens a find bar above the table: matches are
highlighted, Enter/Shift+Enter jump to the next/previous match and reveal it horizontally and
vertically. A "Only matching rows" toggle turns the same text into a quick filter. Matching uses
`get()` (canonical text) or `CellUi::text()`.

**Per-column filters (later).** A filter popup in the header by column type: text contains, number
range, enum set, empty/non-empty. Custom filter UI through `CellUi::filter_ui(col, ui, &mut
ColumnFilter)` (README promises filtering "based on custom user ui").

**Feedback.** The tool column shows the natural row number, not the visual index, so filtered-out
rows show as gaps (as in Excel). The status bar shows "1 234 of 10 000 rows".

### DESIGN-14: Look and feel

**Status:** proposed 2026-10-01; roadmap step 7. Fixes VIEW-7, VIEW-11 and decides DOC-2.

All colors come from the current `egui::Visuals` each frame, so theme switches apply immediately.
`TableStyle` in `TableViewOptions` overrides the defaults:

```rust
pub struct TableStyle {
    pub density: Density,          // Compact | Normal | Comfortable: padding + minimum row height
    pub grid_lines: GridLines,     // None | Horizontal | All
    pub striped: bool,             // today always on
    pub show_column_types: bool,   // type label under the header name
    pub truncate_text: bool,       // "…" + full text on hover (default true)
    pub change_flash: f32,         // seconds, DESIGN-11
}
```

- **Selection** (VIEW-7): range fill is `visuals.selection.bg_fill` at low alpha; the active cell
  gets a 2 px `visuals.selection.stroke` border and no fill; the range gets a 1 px outline on all
  four sides. Header cells of selected columns and row numbers of selected rows are highlighted.
- **Semantic levels** (VIEW-11): `CellLevel::{Info, Warning, Error, Changed}` map to a low-alpha
  background of `hyperlink_color`, `warn_fg_color`, `error_fg_color` and the selection color; text
  keeps the normal color. Corner triangles use the full color. `Custom(Rgb)` picks black or white
  text by contrast.
- **Text overflow:** single-line cells truncate with "…" and show the full text on hover, only
  when truncated. `WrapMode` in metadata stays as a per-cell override.
- **Alignment** by column type: numbers right with tabular (fixed-width) digits, booleans
  centered, text left. Per-column override in `TableViewConfig`.
- **Skipped rows/columns** (DOC-2): one look for both, drawn by the view: weak text color and
  strike-through. The diagonal cross goes away (it hides the content).
- **Headers:** a distinct background, the name in strong text, the type label in small weak text
  (optional), the sort indicator (DESIGN-13).
- **Tests:** these are visual, so a few pixel snapshots (light and dark: a selection, the levels, a
  truncated cell), masked to the area under test (see AGENTS.md).

---

## Unused API, dead code and housekeeping

- **Never called by the view:** `TableBackend::{reload, poll, commit_all, commit_immediately,
  use_column, on_highlight_cell}` and `TableFrontend::{column_render_config, on_cell_view_response}`
  were removed in step 0. Implementors that overrode `poll` keep
  it as an inherent method.
- **Dead modules:** `src/cell.rs`, `src/filter.rs`, `src/sort.rs`, `src/table_view/{interface,
  widgets,util}.rs` were deleted in step 0.
- **Missing derives:** `Debug` on `VisualRowIdx`, `VisualColIdx`, `CellCoord`; `Ord`/serde on
  `RowUid`; `Default` for `TableView`. Added in step 0.
- `CellMetadata` builder methods restated every field. Simplified in step 0.
- **`Cargo.toml`:** `log = "0"` was too loose, and the workspace declared
  `tabular_core`/`tabular_derive` at `0.1` while the root crate used `0.2`. Both fixed in step 0;
  the root crate now uses the workspace entries. `rvariant` is still a path dependency to
  `../rvariant`, so a sibling checkout is required to build.
- `.idea/` was committed. Untracked and ignored in step 0.
- `demos/simple` has placeholder hyperlinks (`"Abc"`, `"Def"`, `github.com/...`).
- README "Keyboard shortcuts" and "Features" lists should be derived from this file (DOC-4).
- **No CI.** There is no `.github/`. Planned: `cargo fmt --check`, clippy with `-D warnings`, tests,
  a check that every `--ignored` UI test still fails, a `wasm32-unknown-unknown` build of the crate,
  an MSRV build, and `just dry-publish`.
- **`rvariant` path dependency** blocks CI and publishing, not only local builds. Use a git or
  crates.io dependency before CI and before the 0.3.0 release.
- **No benchmarks.** Add a headless benchmark (egui_kittest frames, release build) for the
  [performance targets](#goals-use-cases-and-targets): scrolling 1M rows, live append + eviction,
  1M-row sort, and a lookup counter for the row builder.
- **Property tests** (proptest) for `RowLayout::{sync, normalize, reveal}`, selection clamping after
  row/column set changes, and the paste parser: the underflow bugs (SEL-2, PASTE-3) are this kind.
- **Pixel snapshots** for DESIGN-14 only, light and dark, small and masked.
- **API docs** (DOC-5): `#![warn(missing_docs)]` after DESIGN-8; `cargo semver-checks` in CI after
  0.3.0 is released.

---

## Roadmap

Suggested order; update as items land. Breaking changes are preferred whenever they improve the
design (all users will be ported), so the API redesign comes **first**: most crash and data-loss bugs are fixed by it
structurally, so patching them in the old code first would be wasted work.

0. ✅ **Housekeeping** — done: dead modules deleted, never-called trait stubs removed, missing
   derives, `Cargo.toml` versions, `.idea/` untracked.
1. ✅ **Core contract** ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui) with its
   amendments, see "Step 1 implemented" there): `Revision`, `Capabilities`, `TableModel` (optional
   `get`, typed `ColumnInfo::ty`, `row_values`, 64-bit `RowUid`), `CellUi` with the row builder
   ([DESIGN-10](#design-10-row-builder-for-cell-ui)) and `text()`, `VariantCellUi`, command queue
   whose `apply` returns inverse commands, `TableViewOptions`, `TableViewOutput`/`TableEvent`.
   Ported `VariantBackend` → `VariantTable`, the derive macro, the importer and the three demos.
   Fixed FLAGS-1..5, EDIT-4, BACKEND-1, BACKEND-2, DERIVE-1..5, PASTE-5, and along the way EDIT-1,
   EDIT-2, EDIT-7, SEL-1, PASTE-3, VIEW-4, VIEW-10, VIEW-11, EXPORT-2, DOC-1..3.
2. 🚧 **View state rewrite:** uid selection with anchor/cursor (cells, rows, columns), the editor
   overlay ([DESIGN-12](#design-12-editor-overlay)), the full edit lifecycle (`finish_edit`, cancel
   when the row disappears), focus-based input, programmatic control (`set_selection`,
   `scroll_to`, ...), view-owned row order (sorting) built into `RowLayout` with `SortPolicy`
   ([DESIGN-13](#design-13-sort-and-filter-stability-and-ui)). Fixes EDIT-3, EDIT-5, EDIT-6,
   EDIT-9, EDIT-12, SEL-2..5, VIEW-5, DND-3. Already landed ahead of this step: the anchor-based
   layout ([DESIGN-9](#design-9-anchor-based-layout-without-egui_extras)), the view-owned edit
   buffer and `TableViewOptions` (step 1).
3. 📋 **Column order and widths** (DESIGN-4): DND-1, DND-4, DND-5, DND-6; persist `ColumnWidths`;
   hidden columns with a column chooser; frozen columns and a sticky tool column.
4. 📋 **Live data** ([DESIGN-11](#design-11-live-data-and-models-without-variant-values)): `rows_log`
   fast path with a `VecDeque` row order, configurable stick to bottom that survives eviction,
   per-column degradation for models without values, change highlight, `demos/live` (simulated CAN
   trace + register map) as the benchmark scenario.
5. 📋 **Spreadsheet essentials:** Delete/Backspace clear, cut, undo/redo UI, Ctrl+Arrow, Home/End,
   Page Up/Down, Shift+Tab/Shift+Enter, Enter moves down, drag-select, horizontal reveal, cell
   context menu, Ctrl+Enter, fill down/right, sort UI and find bar with quick filter (DESIGN-13),
   rename/remove columns, insert/delete rows, numbers edited as text (EDIT-11), confirmation for
   Clear (VIEW-6).
6. 📋 **Paste/export/import:** paste via the `csv` crate (PASTE-1..4), paste fill without a dialog,
   copy raw values and copy with headers, `write_csv(model, order, impl Write) -> Result`
   (EXPORT-1, EXPORT-3, EXPORT-4), `rfd` behind a feature (DESIGN-7), IMPORT-1..5. Independent of steps 1–5; can
   land any time.
7. 📋 **Look and feel and accessibility** ([DESIGN-14](#design-14-look-and-feel)): `TableStyle`,
   selection drawing (VIEW-7), contrast for `CellLevel::Custom`, truncation, alignment, header highlight,
   status bar; AccessKit grid semantics; touch drag-to-scroll (VIEW-8).
8. 📋 **More features:** per-column filters, XLSX import, more editors (EDIT-8, dates, searchable
   enum), multi-line text, translatable strings, pause for live views, hierarchical rows (idea).

**Independent, any time — do first:** CI (needs the `rvariant` dependency fixed), then benchmarks
and property tests (see [housekeeping](#unused-api-dead-code-and-housekeeping)). Small fixes:
IMPORT-2, IMPORT-3, DOC-4. DOC-5 can start now that step 1 has landed.

---

## Fixed issues

Move entries here when fixed (keep the ID, add the commit hash and a one-line note).

| ID | Fixed in | Note |
|----|----------|------|
| VIEW-1 | `bf0d2d8` | Body iterates the view's row order, resynced after the header and on row count change; no `unwrap`. (DESIGN-9) |
| VIEW-2 | `bf0d2d8` | Row heights cached by `RowUid`. (DESIGN-9) |
| VIEW-3 | `bf0d2d8` | No egui_extras state; all ids derive from the `id` passed to `show`. (DESIGN-9) |
| VIEW-9 | `bf0d2d8` | `N` blinked when the view was at the end: the post-header sync moved the anchor to the end (stick to bottom) and the body rendered it unclamped, i.e. empty, for one frame. Now normalized after every sync. (DESIGN-9) |
| EDIT-10 | `0205e1e` | Clicking on a cell's text didn't select or edit it, only the empty part of the cell did: labels are selectable by default and sense clicks above the cell. Cells and headers now disable `selectable_labels`, which also covers custom cell UI. Found by the egui_kittest suite (`selection::click_selects_cell`). |
| DND-2 | `bf0d2d8` | Column widths keyed by `ColumnUid`. (DESIGN-9) |
| FLAGS-1 | `80bd7d2` | One-shot flags replaced by `Revision` counters that are never consumed; every view and app code compares its own copy. Test: `model::every_view_sees_model_changes`. (DESIGN-8) |
| FLAGS-2 | `80bd7d2` | View events (`rows_selected`, `column_mapping_changed`) moved to `TableViewOutput::events`. (DESIGN-8) |
| FLAGS-3 | `80bd7d2` | Flag system removed. (DESIGN-8) |
| FLAGS-4 | `80bd7d2` | One source each: `capabilities()`, `ColumnInfo::is_skipped`. (DESIGN-8) |
| FLAGS-5 | `80bd7d2` | No flag methods; a model implements `revision()` and `capabilities()`. (DESIGN-8) |
| EDIT-1 | `80bd7d2` | A row number click commits the edit; the edit buffer is the view's. Test: `editing::row_number_click_commits_edit`. |
| EDIT-2 | `80bd7d2` | No backend edit buffer to go stale; the view drops its buffer when the cell is no longer edited. Test: `editing::escape_away_from_table_cancels_edit`. |
| EDIT-4 | `80bd7d2` | `E` and Tab take the cell from the view's row and column order; `col_uid`/`row_uid` are gone from the model. Tests: `editing::{e,tab}_edits_visual_column_after_reorder`. (DESIGN-8) |
| EDIT-7 | `80bd7d2` | Bool values are drawn as a check mark that doesn't take the click. Test: `model::bool_cell_click_selects_the_cell`. |
| SEL-1 | `80bd7d2` | Row selections end at the last column. Test: `selection::row_selection_copies_exactly_the_row`. |
| PASTE-3 | `80bd7d2` | Column count to create uses `saturating_sub`. |
| PASTE-5 | `80bd7d2` | `VariantTable` converts values to the column type; failures are kept and marked. Test: `commands::tests::paste_converts_to_column_type`. |
| VIEW-4 | `80bd7d2` | "Add row", "Create column", "Append row", "Clear", "Skip" follow the model's capabilities and `TableViewOptions::read_only`. Test: `model::read_only_table_offers_no_add_row_or_create_column`. |
| VIEW-10 | `80bd7d2` | `TableEvent::Message` instead of a log line. Test: `model::paste_without_selection_tells_the_user`. |
| VIEW-11 | `80bd7d2` | `CellLevel` instead of `Rgb`, mapped to the theme's colors. |
| BACKEND-1 | `80bd7d2` | `VariantTable::set_read_only` sets the capabilities. Test: `editing::read_only_table_does_not_edit`. |
| BACKEND-2 | `80bd7d2` | `col_uid` removed from the model. (DESIGN-8) |
| EXPORT-2 | `80bd7d2` | Export uses the view's column order. |
| DERIVE-1 | `80bd7d2` | The `#[format]` string literal is used as is; anything else is a compile error. Test: `tests` package. |
| DERIVE-2 | `80bd7d2` | `<Row>Table` has the row struct's visibility. |
| DERIVE-3 | `80bd7d2` | The generated code has no egui. |
| DERIVE-4 | `80bd7d2` | `get()` and `row_values()` are generated; copy and export work. |
| DERIVE-5 | `80bd7d2` | Tuple fields are accessed as `row.0`. |
| DOC-1 | `80bd7d2` | README links `TableModel` and `CellUi`. |
| DOC-2 | `80bd7d2` | README says skipped cells are crossed out. |
| DOC-3 | `80bd7d2` | README says the view keeps one id per row. |

---

## Maintaining this file

- **Adding a feature:** add or update its row in the feature inventory with the right status. If it
  adds or changes keyboard/mouse behavior, update the reference table.
- **Fixing a bug:** remove the entry from [Known bugs](#known-bugs), add a row to
  [Fixed issues](#fixed-issues) with the commit hash, and update any inventory row or roadmap item
  that linked to it.
- **Finding a bug:** add it under the right area with the next free ID for that prefix, using the
  format: bold one-line summary + severity; Where / Symptom / Cause / Fix bullets as known.
- **New prefixes:** add a heading under Known bugs.
- Keep claims verifiable: say how a bug was confirmed (code reading, repro, test).
- **UI tests:** a known bug that can be reproduced through the UI gets a test in `tests/ui/` that
  asserts the intended behavior, marked `#[ignore = "<ID>: summary"]`, and the bug entry links it
  with a "Test" bullet. Fixing the bug means removing the `ignore`. Check with
  `cargo test --test ui -- --ignored` that every ignored test still fails; one that passes is either
  fixed or a broken repro.
