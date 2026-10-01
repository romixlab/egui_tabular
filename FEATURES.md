# egui_tabular — Features, Known Issues and Planned Work

This file is the **single source of truth** for what the crate does, how well it does it, what is
broken and what is planned. README.md is the public pitch; this file is the engineering record.

- Every change that adds, removes, fixes or alters a feature **must update this file in the same
  commit** (see [Maintaining this file](#maintaining-this-file)).
- Issue IDs (`EDIT-1`, `DND-3`, ...) are stable. Never renumber or reuse an ID; mark it fixed instead.
- File/line references were taken at commit `29fddab` and will drift; the function name is the
  durable anchor.

Last full review: 2026-10-01 (baseline `29fddab`, egui 0.36, egui_extras 0.36.1).

---

## Contents

1. [Status legend](#status-legend)
2. [Crate layout](#crate-layout)
3. [Feature inventory](#feature-inventory)
4. [Keyboard and mouse reference](#keyboard-and-mouse-reference)
5. [Known bugs](#known-bugs)
6. [Design issues and planned rework](#design-issues-and-planned-rework)
7. [Unused API, dead code and housekeeping](#unused-api-dead-code-and-housekeeping)
8. [Roadmap](#roadmap)
9. [Fixed issues](#fixed-issues)
10. [Maintaining this file](#maintaining-this-file)

---

## Status legend

| Status    | Meaning                                                                    |
|-----------|----------------------------------------------------------------------------|
| `done`    | Implemented and believed correct.                                          |
| `buggy`   | Implemented, but with known bugs (linked by ID).                           |
| `partial` | Some of it works; the gaps are described.                                  |
| `stub`    | UI or API exists but does nothing.                                         |
| `planned` | Not implemented; intended.                                                 |
| `idea`    | Not implemented; worth considering, not committed to.                      |

Severity for bugs: **crash** (panic), **data-loss** (user edits or data silently lost/corrupted),
**major** (feature visibly broken), **minor** (cosmetic or edge case).

---

## Crate layout

| Path | Purpose |
|------|---------|
| `tabular_core/` | egui-independent core types: `TableBackend` trait, `ColumnUid`, `RowUid`, `CellCoord`, `BackendColumn`, `PersistentFlags`, `OneShotFlags`, `CellMetadata`, `Rgb`, `WrapMode`, `CsvImporterConfig`. Re-exports `rvariant::{Variant, VariantTy}`. |
| `tabular_derive/` | `#[derive(TabularRow)]` proc macro: turns `Vec<Row>` into a read-only backend + frontend. |
| `src/table_view.rs` | `TableView::show()`: header, body rendering, column DnD, row heights. |
| `src/table_view/interaction.rs` | Keyboard handling, copy, paste (+ paste modal), selection moves. |
| `src/table_view/state.rs` | `State` and `SelectedRange` (selection + editing coord). |
| `src/table_view/tool_column.rs` | Left "tool" column: row numbers and context menus. |
| `src/table_view/config.rs` | `TableViewConfig` (serializable view settings). |
| `src/frontend.rs` | `TableFrontend` trait: cell view/editor UI, colors, tooltips, corners. |
| `src/backends/variant.rs` | `VariantBackend`: in-memory `HashMap<CellCoord, Variant>` backend + frontend. |
| `src/importers/` | `TabularImporter` (file picker + CSV options UI), `CsvImporter`, `RequiredColumns` (name/synonym mapping). |
| `src/util.rs` | `base_26` column names, encoding detection, CSV export. |
| `demos/simple`, `demos/derive_row`, `demos/csv_xls_import` | Example apps. |
| `tests/` | Compile test for the derive macro. |

The view is generic over `T: TableBackend + TableFrontend`. The backend owns data, column info and
(today) the in-progress edit buffer. The view owns visual column order, selection, row heights and
paste state.

---

## Feature inventory

### Core data model / backend trait

| Feature | Status | Notes |
|---------|--------|-------|
| Generic `TableBackend` trait (rows/cols by uid, visual index → uid mapping) | `done` | See [DESIGN-3](#design-3-tablebackendcol_uid-conflicts-with-view-owned-column-order) for `col_uid`. |
| `get` / `set` cells as `Variant` | `done` | Optional; copy and CSV export depend on `get`. |
| Create row / create column | `done` | `VariantBackend` names new columns `A`, `B`, ... via `base_26`. |
| Row / column skipping (strike-through, excluded from export) | `buggy` | Skipped columns are still exported ([EXPORT-3](#export-3)). |
| Cell metadata: background color, corner triangle, multiple tooltips, wrap mode | `done` | `set_metadata(coord, meta, merge)`. |
| Change/flag notification (`PersistentFlags`, `OneShotFlags`) | `buggy` | Error-prone; see [DESIGN-1](#design-1-flags-system), [FLAGS-*](#flags-and-change-notification). |
| Read-only tables | `buggy` | `VariantBackend::set_read_only` has no effect ([BACKEND-1](#backend-1)); several buttons ignore read-only ([VIEW-4](#view-4)). |
| Remote/lazy backends (`reload`, `poll`, `commit_all`, `commit_immediately`) | `stub` | Trait methods exist; the view never calls them. |
| Column "used" vs "available" | `partial` | `used_columns()` exists, but `VariantBackend` doesn't override it and `use_column` is a no-op. |
| Undo / redo | `planned` | |
| Sorting | `stub` | `is_sortable`, "Sort ascending/descending" menu items exist but do nothing; `src/sort.rs` is not compiled. |
| Filtering | `planned` | `src/filter.rs` is not compiled. |

### `VariantBackend`

| Feature | Status | Notes |
|---------|--------|-------|
| In-memory storage, column defaults applied on row insert | `done` | |
| `turn_column_into(ty)` with conversion errors shown as orange cells + tooltip | `done` | Code-only API, no UI. |
| Viewers: Str, StrList, Bool, Enum, numbers, others via `Display` | `buggy` | Bool viewer is a live checkbox ([EDIT-7](#edit-7)). |
| Editors: Str (TextEdit), Bool, Enum (ComboBox), U32/U64/I32/I64 (DragValue) | `partial` | No editor for F32/F64 or other `Number` widths ([EDIT-8](#edit-8)). Enum doesn't commit on selection ([EDIT-6](#edit-6)). |
| Column mapping choices (combo box above columns) | `done` | Stored in `TableViewConfig::column_mapped_to`, keyed by `ColumnUid`. |
| Date, SI values, currency viewers/editors | `planned` | |

### Table view

| Feature | Status | Notes |
|---------|--------|-------|
| Virtualized rendering (only visible rows) | `done` | Via `egui_extras::TableBuilder`. |
| Heterogeneous row heights | `buggy` | Heights only resync on `row_set_updated` flag ([VIEW-1](#view-1)); editing causes jumps ([EDIT-5](#edit-5)). |
| Resizable columns | `buggy` | Widths are positional and don't follow reordered columns ([DND-2](#dnd-2)). |
| Column header: name, type, hover info (required/synonyms/used) | `done` | |
| Column header context menu | `partial` | Only "Skip" works. "Sort", "Hide" are stubs; "Add column" is only shown for sortable columns ([VIEW-5](#view-5)). |
| Column drag & drop reorder | `buggy` | Swaps instead of moves; widths, selection and order persistence broken ([DND-*](#column-drag--drop)). |
| Tool column (row numbers, row context menu: append, skip) | `done` | |
| Tool column header menu: Export CSV, Append row, Clear | `buggy` | "Clear" panics ([VIEW-1](#view-1)), no confirmation ([VIEW-6](#view-6)). |
| "No columns" state with "Create column" button | `buggy` | Ignores read-only / creation support ([VIEW-4](#view-4)). |
| Multiple tables in one parent `Ui` | `buggy` | egui_extras state collides ([VIEW-3](#view-3)). |
| Cell background colors / corner triangles / tooltips | `done` | |
| Selection: single cell, rectangle, whole row, select all | `buggy` | See [SEL-*](#selection-and-keyboard). |
| Scroll selection into view on keyboard navigation | `planned` | |
| Stick-to-bottom for live data | `partial` | `stick_to_bottom(true)` is hard-coded, not configurable. |
| Visual state persistence (`TableViewConfig` is serde) | `partial` | Column order and widths are not persisted ([DND-5](#dnd-5)). |
| Custom column header UI (`TableFrontend::custom_column_ui`) | `done` | |
| Per-column render config (`TableFrontend::column_render_config`) | `stub` | Never called; `Column::auto()` is hard-coded. |

### Editing

| Feature | Status | Notes |
|---------|--------|-------|
| Click selected cell to edit; `E` to edit | `buggy` | See [EDIT-*](#cell-editing). |
| Enter commits, Escape cancels, Tab commits and edits next cell | `buggy` | [EDIT-1](#edit-1) – [EDIT-4](#edit-4), [EDIT-6](#edit-6). |
| Commit on focus loss / click outside | `planned` | [EDIT-3](#edit-3). |
| Start editing by typing / Enter / F2 / double-click | `planned` | [EDIT-9](#edit-9). |

### Clipboard

| Feature | Status | Notes |
|---------|--------|-------|
| Copy selection as TSV (Ctrl+C) | `buggy` | Row selections add trailing tabs ([SEL-1](#sel-1)); uses `ctrl` not `command` ([SEL-5](#sel-5)). |
| Paste TSV block into selection | `buggy` | See [PASTE-*](#paste). |
| Paste into empty table creates columns and rows | `done` | |
| Paste dialog for size mismatch (create rows, repeat fill, create columns) | `buggy` | [PASTE-1](#paste-1) – [PASTE-4](#paste-4). |
| Paste "overflow" mode | `planned` | Commented-out button in `handle_paste_continue`. |

### Import / export

| Feature | Status | Notes |
|---------|--------|-------|
| `TabularImporter` UI: file picker, reload, separator, header row, skip N rows | `done` | |
| CSV import with auto separator detection | `done` | Counts `,` `\t` `;` in the first MiB. |
| Encoding detection (chardetng) | `buggy` | Never reads any bytes ([IMPORT-1](#import-1)). |
| Required columns mapped by name/synonym (case-insensitive) | `done` | Header names are not trimmed ([IMPORT-4](#import-4)). |
| CSV without header row | `buggy` | Last column dropped ([IMPORT-2](#import-2)). |
| Ragged CSV (rows wider than header) | `buggy` | Extra cells land in wrong columns ([IMPORT-3](#import-3)). |
| Preview mode (`set_max_lines`) | `done` | |
| XLS/XLSX import | `planned` | Demo is named `csv_xls_import` but only CSV exists. |
| Export CSV | `buggy` | See [EXPORT-*](#export). |
| Export XLS/XLSX | `idea` | |

### Derive macro (`#[derive(TabularRow)]`)

| Feature | Status | Notes |
|---------|--------|-------|
| Generates `<Row>TabularBackend` with `new(Vec<Row>)`, read-only | `buggy` | Struct is private ([DERIVE-2](#derive-2)). |
| Column names from field names (Sentence case), type shown in header | `done` | |
| `#[format = "..."]` per field | `buggy` | Output wrapped in literal quotes ([DERIVE-1](#derive-1)). |
| Copy / CSV export from derived tables | `planned` | `get()` not generated ([DERIVE-4](#derive-4)). |

---

## Keyboard and mouse reference

Actual behavior at the baseline commit (including quirks). Keyboard handling is active only while
the **mouse pointer is over the table**, not based on focus ([SEL-4](#sel-4)).

| Input | Context | Behavior |
|-------|---------|----------|
| Click cell | — | Select cell. Commits the edit if another cell was being edited. |
| Click selected cell | Not read-only | Enter edit mode. |
| Shift+click cell | — | Grow selection bounding box (cannot shrink, [SEL-3](#sel-3)). |
| Click / Shift+click tool column | — | Select row / extend row selection. Drops an in-progress edit ([EDIT-1](#edit-1)). |
| Right-click tool column / header | — | Context menus (see feature inventory). |
| Drag column header | — | Swap with drop target ([DND-1](#dnd-1)). |
| Arrows (+Shift) | Not editing | Move (grow) selection. Panics with 0 rows ([SEL-2](#sel-2)). |
| `E` | Not editing, not read-only, single cell | Start editing. Wrong column after reorder ([EDIT-4](#edit-4)). |
| `N` | Not editing, not read-only | Append row. Also fires while typing in other widgets ([SEL-4](#sel-4)). |
| Ctrl+A / Cmd+A | Not editing | Select all. |
| Ctrl+C | Not editing | Copy TSV. Cmd+C on macOS doesn't work ([SEL-5](#sel-5)). |
| Ctrl/Cmd+V | Not editing, not read-only | Paste. |
| Enter | Editing | Commit. Also fires if Enter was pressed elsewhere ([EDIT-6](#edit-6)). |
| Escape | Editing | Cancel edit. Doesn't clear the backend buffer if the pointer is outside the table ([EDIT-2](#edit-2)). |
| Escape | Not editing | Clear selection. |
| Tab | Editing | Commit, move right, edit. Wrong column after reorder ([EDIT-4](#edit-4)). |

---

## Known bugs

### Cell editing

The root cause of most of these is that edit state has **two owners**: the view's
`SelectedRange.editing: Option<CellCoord>` and the backend's `VariantBackend.cell_edit:
Option<(CellCoord, Variant)>`. Many exit paths update one but not the other. See
[DESIGN-2](#design-2-edit-lifecycle) for the fix.

#### EDIT-1
**Leaving edit mode via tool column or header click drops the edit without commit or cancel.** — *data-loss*
- Where: `show_body`, tool column click handler (`table_view.rs:446-455`); `VariantBackend::show_cell_editor` (`variant.rs:445-512`).
- Symptom: the edit is lost. Re-editing the same cell later shows the abandoned text, and the editor never gets focus.
- Cause: `*r = SelectedRange::single_row(..)` replaces the selection and drops `editing`. The backend keeps the stale `cell_edit`. The next `show_cell_editor` for that coord sees `prev_coord == coord`, treats it as "not first pass", reuses the stale value and skips `request_focus()`.
- Fix: route every exit through one `end_edit(commit)` ([DESIGN-2](#design-2-edit-lifecycle)).

#### EDIT-2
**Escape with the pointer outside the table doesn't cancel in the backend.** — *major*
- Where: `show_body` Escape branch (`table_view.rs:547`) vs `handle_key_input_when_editing` (`interaction.rs:80-85`).
- Cause: the `show_body` branch only sets `editing = None`; only the interaction handler calls `cancel_edit()`, and that handler runs only when the pointer is over the table. Same stale-buffer symptoms as EDIT-1.

#### EDIT-3
**Clicking outside the table, or Tab-ing focus away, leaves the editor open but unfocused.** — *major*
- Where: `show_body`.
- Cause: the editor response's `lost_focus()` is never checked. Keystrokes go nowhere until the user clicks back into the cell. With the pointer outside the table, Tab is handled by egui focus traversal instead of the table.
- Fix: commit (or cancel, configurable) on `lost_focus()`.

#### EDIT-4
**Tab and `E` compute the edit coord from backend column order, not visual order.** — *data-loss*
- Where: `handle_key_input_when_editing` (`interaction.rs:73`), `handle_selection_moves` (`interaction.rs:348`).
- Cause: they call `data.col_uid(VisualColIdx(col))`, which indexes the backend's column list. The view renders `state.columns_ordered`, which differs after a column drag. For `VariantBackend`, `col_uid` also counts unused columns. For derived backends it always returns `None`, so Tab/E never work there.
- Effect: the editor is drawn at the right visual cell (rendering matches positionally), but the stored coord is wrong. `VariantBackend::commit_cell_edit(coord)` requires `last_edited_coord == coord`, so the **edit is silently discarded**.
- Fix: use `self.state.columns_ordered[col_idx]`; remove `TableBackend::col_uid` ([DESIGN-3](#design-3-tablebackendcol_uid-conflicts-with-view-owned-column-order)).

#### EDIT-5
**Editor is taller than the row: rows jump or the editor is clipped.** — *major (visual)*
- Where: `TableViewConfig::default` (`minimum_row_height = 15.0`), `show_body` row-height feedback.
- Cause: TextEdit/DragValue are about 20px plus frame. With heterogeneous heights the row grows one frame late (one frame of overflow, then every row below shifts) and shrinks back after commit. With fixed heights the editor is clipped.
- Fix: default the minimum row height to `ui.spacing().interact_size.y`, and/or use frameless editors sized to the row.

#### EDIT-6
**Enter commit is global; discrete editors don't commit on change.** — *minor*
- Where: `show_body` (`table_view.rs:544`).
- Cause: `ui.input(|i| i.key_pressed(Key::Enter))` is checked regardless of which widget had focus. Choosing an item in the Enum ComboBox doesn't commit; the user must also press Enter.
- Fix: commit on `resp.lost_focus() && enter`, and on `changed()` for Bool and Enum.

#### EDIT-7
**Bool cells render a live checkbox in view mode.** — *minor*
- Where: `VariantBackend::show_cell_view` (`variant.rs:401-404`).
- Cause: toggles a local copy, so it flickers and does nothing. It also swallows the click, so the cell isn't selected.
- Fix: draw a disabled checkbox or a glyph.

#### EDIT-8
**No editor for F32/F64 and other `Number` widths.** — *minor*
- Where: `VariantBackend::show_cell_editor` (`variant.rs:489-507`). Shows "Editor is not implemented for …".

#### EDIT-9
**Dead/missing edit entry points.** — *minor*
- `handle_selection_moves` commits an edit on arrow keys, but it only runs when not editing (dead code, `interaction.rs:322`).
- `resp.double_clicked_by(..) {}` is an empty branch (`table_view.rs:580`).
- No Enter/F2/type-to-edit.

### Column drag & drop

#### DND-1
**Drop swaps columns instead of moving.** — *major (UX)*
- Where: `TableView::swap_columns` (`table_view.rs:376`).
- Dropping A on D in `A B C D` yields `D B C A`. Expected `B C D A` (or insert-before semantics with an insertion marker).

#### DND-2
**Column widths don't follow reordered columns.** — *major (visual)*
- Cause: egui_extras stores `TableState::column_widths` by position (`egui_extras-0.36.1/src/table.rs:578`). After a reorder each column inherits its neighbour's width.
- Fix: keep per-`ColumnUid` widths in view state/config and feed them as `Column::initial`, or reset the egui_extras table state (`Table::reset`) for the affected columns after a move.

#### DND-3
**`SelectedRange::swap_col` is a no-op.** — *major*
- Where: `state.rs:122-133`.
- Cause: the first `if` sets `col_start = col2`, then the second `if self.col_start == col2` immediately sets it back to `col1`. Needs `else if`. The selection (and an in-progress edit) ends up on the wrong column.

#### DND-4
**Drop-target highlight hides the header text and highlights the source column.** — *minor (visual)*
- Where: `table_view.rs:188-208`.
- Painted after the content at 50% alpha over the top 66% of the cell. Prefer a thin insertion line on the target edge, and skip it when hovering the dragged column itself.

#### DND-5
**User column order is not preserved.** — *major*
- Where: `check_col_set_updated` (`table_view.rs:293`).
- `columns_ordered` is rebuilt and sorted by uid on every `columns_reset` (any `insert_column`, CSV reload). Order is also not stored in `TableViewConfig`.

#### DND-6
**Grab cursor only over the label.** — *minor*
- `on_hover_cursor(Grab)` is set on the name label (`table_view.rs:155`), but the whole header cell is draggable.

### Selection and keyboard

#### SEL-1
**Row selection is one column too wide.** — *minor*
- Where: `SelectedRange::single_row` and `stretch_multi_row` set `col_end = col_count` (`state.rs:73`, `:161`).
- Effect: Ctrl+C on a row selection appends an extra `\t` per line; paste size matching is off by one.

#### SEL-2
**Arrow keys panic with zero rows.** — *crash (debug)*
- Where: `move_down`/`move_right` compute `count - 1` (`state.rs:190`, `:210`).
- Repro: select a cell, use tool header menu → Clear (if it doesn't panic first, see VIEW-1), press ↓.
- Cause: the selection is never validated or cleared when the row/column set changes.

#### SEL-3
**Shift-extend has no anchor.** — *minor (UX)*
- `stretch_to`, and `move_*` with `expand`, only grow the bounding box. You can't shrink a selection with Shift+arrows or Shift+click.

#### SEL-4
**Keyboard and paste handling depend on mouse hover, not focus.** — *major*
- Where: `TableView::show` (`pointer_over_table`).
- Effect: with the pointer over the table, typing `n` in another TextEdit appends rows, `e` starts editing, pasting into another field also pastes into the table, and Ctrl+C copies the table. Moving the mouse away disables table navigation.
- Fix: track table focus (focusable table response or a "table active" flag set on click). Also skip handling when `ctx.wants_keyboard_input()` and the focused widget isn't ours.

#### SEL-5
**Copy uses `modifiers.ctrl`.** — *minor*
- `interaction.rs:15`. Cmd+C on macOS doesn't work (egui issue #4065). Prefer handling `Event::Copy`.

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

#### PASTE-3
**"Create columns" can panic.** — *crash*
- `about_to_paste_rows[0].len() - selected_range.width()` underflows when the first row is narrower than the selection (`interaction.rs:261`).

#### PASTE-4
**Paste dialog state not reset.** — *minor*
- The code resets `create_cols_on_paste` (`interaction.rs:152`), but the checkbox is bound to `create_adhoc_cols_on_paste`, so the previous choice persists. `create_cols_on_paste` is otherwise unused.

#### PASTE-5
**Pasted values are always `Variant::Str`.** — *major*
- Typed columns (e.g. U32) receive strings; `VariantBackend::set` doesn't convert. Fix: convert using the column type in the backend (`set` should coerce or report failure via metadata).

### Table view / layout

#### VIEW-1
**"Clear" from the tool column menu panics.** — *crash*
- Where: `show_body` (`table_view.rs:430`, `row_uid(..).unwrap()`).
- Cause: `clear()` runs inside the header closure. The body then iterates the stale `row_heights` (heterogeneous mode) and `row_uid` returns `None`. More generally `row_heights` only resizes on the `row_set_updated` flag, so any backend that forgets the flag can panic here or hide rows.
- Fix: resync when `row_heights.len() != row_count()`; never `unwrap` `row_uid`.

#### VIEW-2
**Row heights are indexed by visual row.** — *minor*
- Once sorting/filtering exists, heights won't follow rows. Key by `RowUid` or reset on visible-order change.

#### VIEW-3
**Two `TableView`s in the same parent `Ui` share egui_extras state.** — *major*
- `TableBuilder::new(ui)` has no `.id_salt(id)` (`table_view.rs:82`), so column widths and scroll state collide.

#### VIEW-4
**Buttons ignore capabilities.** — *minor*
- "Add row" under an empty table and "Create column" in the no-columns state are shown regardless of `is_read_only` or whether `create_row`/`create_column` are supported.

#### VIEW-5
**Column context menu stubs.** — *minor*
- "Sort ascending/descending" and "Hide" do nothing. "Add column" is nested inside `if col.is_sortable` (`table_view.rs:333-344`).

#### VIEW-6
**"Clear" has no confirmation.** — *minor*
- TODO in `tool_column.rs`. A commented-out modal exists in `interaction.rs` (`handle_clear_request`).

#### VIEW-7
**Selection styling uses `warn_fg_color`.** — *minor (visual)*
- Selection fill and borders use the warning color rather than `visuals.selection`. Vertical selection borders are commented out, so multi-column selections have no side edges.

### Flags and change notification

See [DESIGN-1](#design-1-flags-system) for the planned replacement.

#### FLAGS-1
**One-shot flags are consumed by the view.** — *major*
- `show()` archives and zeroes them. A second consumer (a second `TableView`, or app code reading after another view) never sees them, which leads to stale columns and the VIEW-1 panic. If the view isn't shown (collapsed tab), the delayed copy never updates.

#### FLAGS-2
**View events are written into backend flags.** — *design*
- `rows_selected` and `column_mapping_changed` are UI events stored in the backend's `OneShotFlags`.

#### FLAGS-3
**Flags that are never set or never read.** — *minor*
- `first_pass` (never set, although documented as "set once data backend is created"), `visible_row_vec_updated`, `reloaded` (only set by paste/CSV), `cleared` (set by the derive macro but not by `VariantBackend::clear`).
- In `PersistentFlags`, `is_reload_*`, `column_info_present`, `row_set_present`, `cells_loading`, `have_all_cells`, `have_uncommitted_data`, `have_collisions` are never read.

#### FLAGS-4
**Duplicated sources of truth.** — *major*
- `TableBackend::is_clearable()` vs `PersistentFlags::is_clearable`: the view uses the method, the derive macro overrides the method but leaves the flag `true`.
- `VariantBackend.read_only` vs `PersistentFlags::is_read_only` (see BACKEND-1).
- `BackendColumn::is_skipped` vs `is_col_skipped()`.

#### FLAGS-5
**Boilerplate burden.** — *design*
- Every backend must implement five flag methods (`one_shot_flags`, `one_shot_flags_internal`, `one_shot_flags_internal_mut`, `one_shot_flags_archive`, `persistent_flags`) with prescribed bodies and two `OneShotFlags` fields.

### Backend

#### BACKEND-1
**`VariantBackend::set_read_only` has no effect.** — *major*
- It writes `self.read_only`, which nothing reads; the view checks `persistent_flags().is_read_only`.

#### BACKEND-2
**`VariantBackend::col_uid` indexes all columns.** — *minor*
- Includes unused ones, so it is inconsistent with what the view shows (feeds EDIT-4).

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

#### EXPORT-2
**Uses backend column order, not view order.** — *minor*

#### EXPORT-3
**Exports skipped columns.** — *minor*
- Only skipped rows are filtered (`un_skipped_rows`).

#### EXPORT-4
**Blocking file dialog in the UI path.** — *minor*
- `rfd::FileDialog::save_file()` blocks the frame and doesn't work on wasm.

### Derive macro

#### DERIVE-1
**`#[format = "..."]` output includes literal quotes.** — *major*
- Where: `tabular_row.rs:158`. `tokens.to_string()` of the literal yields `"\"0x{:08x}\""`.
- Verified: expansion is `format!("\"0x{:08x}\"", x)`, so cells render as `"0x00001000"`.
- Fix: match `Expr::Lit(LitStr)` and emit the `LitStr` directly; emit a compile error for anything else.

#### DERIVE-2
**Generated backend struct is private.** — *major*
- `struct <Row>TabularBackend` has no visibility. Should inherit the row struct's visibility.

#### DERIVE-3
**Generated code requires a direct `egui` dependency.** — *minor*
- It references `egui::Ui` / `egui::Id`. Re-export `egui` from `egui_tabular` and use `egui_tabular::egui::…`.

#### DERIVE-4
**`get()` and `col_uid()` not generated.** — *minor*
- Ctrl+C and CSV export do nothing for derived tables; Tab/E never work.

#### DERIVE-5
**Tuple structs generate invalid field access.** — *minor*
- Unnamed fields generate `row._0` instead of `row.0`.

---

## Design issues and planned rework

### DESIGN-1: Flags system

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

- `TableView` stores `last_seen: Revision` and rebuilds column info / row heights on change.
  Safety net: also resync when `row_heights.len() != row_count()`, so a backend that forgets to
  bump a counter shows stale data instead of panicking.
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

The view owns visual column order (`columns_ordered`, drag & drop). The backend's
`col_uid(VisualColIdx)` is a second, conflicting ordering (root of EDIT-4). Remove it from the
trait; all visual→uid lookups go through view state.

### DESIGN-4: Column order and widths as persisted view state

- Store `column_order: Vec<ColumnUid>` and `column_widths: HashMap<ColumnUid, f32>` in
  `TableViewConfig`.
- On column set change, keep known columns in user order and append new ones.
- Use move (insert) semantics for DnD (DND-1, DND-2, DND-5).

### DESIGN-5: Focus-based input

Track table focus instead of pointer hover (SEL-4). Only handle shortcuts and paste when the table is
focused and no foreign widget wants keyboard input.

### DESIGN-6: Builder-style `show`

`show(table, config, max_height, ui, id)` is growing. Consider
`TableView::new(id).max_height(..).show_tool_column(..).show(ui, table, config)`, with `id` passed
to `TableBuilder::id_salt` (VIEW-3).

### DESIGN-7: Optional heavy dependencies

`rfd` (GTK/portal, file dialogs) is a hard dependency of a widget crate. Put import/export UI behind
a feature (e.g. `file-dialogs`), and expose export as `fn write_csv(table, impl Write) -> Result`.

---

## Unused API, dead code and housekeeping

- **Never called by the view:**
  - `TableFrontend`: `column_render_config`, `on_cell_view_response`.
  - `TableBackend`: `on_highlight_cell`, `commit_all`, `commit_immediately`, `reload`, `poll`, `use_column`.
- **Not compiled (not in any `mod` tree):** `src/table_view/interface.rs`, `src/table_view/widgets.rs`,
  `src/table_view/util.rs`, `src/cell.rs`, `src/filter.rs`, `src/sort.rs`. They reference fields that
  no longer exist. Delete or port.
- **Missing derives:** `VisualRowIdx`, `VisualColIdx`, `CellCoord` lack `Debug`. `RowUid` lacks
  `Ord`/serde. `TableView` lacks `Default` (clippy `new_without_default`).
- `CellMetadata` builder methods restate every field; use `Self { color: Some(rgb), ..self }`.
- **`Cargo.toml`:**
  - `log = "0"` is too loose.
  - Workspace declares `tabular_core`/`tabular_derive` at `0.1`, while the root crate depends on
    `0.2`; the workspace entries are unused.
  - `rvariant` is a path dependency to `../rvariant` (sibling checkout required to build).
- `.idea/` is committed.
- `demos/simple` has placeholder hyperlinks (`"Abc"`, `"Def"`, `github.com/...`).
- README "Keyboard shortcuts" and "Features" lists should be derived from this file.

---

## Roadmap

Suggested order; update as items land.

1. **Crashes and silent data loss (small, local fixes):** VIEW-1, SEL-2, PASTE-3, EXPORT-1, EDIT-4,
   DND-3, IMPORT-2, IMPORT-3, DERIVE-1.
2. **Edit lifecycle** (DESIGN-2): EDIT-1, EDIT-2, EDIT-3, EDIT-5, EDIT-6, EDIT-7.
3. **Column DnD** (DESIGN-4): DND-1, DND-2, DND-4, DND-5, DND-6.
4. **Input and paste:** SEL-4/DESIGN-5, PASTE-1, PASTE-2, PASTE-4, PASTE-5, SEL-1, SEL-3, SEL-5,
   IMPORT-1.
5. **Breaking API rework:** DESIGN-1 (flags → `Revision` + `TableViewOutput`), DESIGN-3, unused API
   removal, DESIGN-6, DESIGN-7, DERIVE-2 – DERIVE-5.
6. **Features:** sorting, filtering, undo/redo, scroll-into-view, XLSX import, more editors.

---

## Fixed issues

Move entries here when fixed (keep the ID, add the commit hash and a one-line note).

| ID | Fixed in | Note |
|----|----------|------|
| — | — | — |

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
