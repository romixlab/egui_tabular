# egui_tabular — Features, Known Issues and Planned Work

This file is the **single source of truth** for what the crate does, how well it does it, what is
broken and what is planned. README.md is the public pitch; this file is the engineering record.

- Every change that adds, removes, fixes or alters a feature **must update this file in the same
  commit** (see [Maintaining this file](#maintaining-this-file)).
- Issue IDs (`EDIT-1`, `DND-3`, ...) are stable. Never renumber or reuse an ID; mark it fixed instead.
- File/line references were taken at commit `29fddab` and will drift; the function name is the
  durable anchor.

Last full review: 2026-10-01 (baseline `29fddab`, egui 0.36, egui_extras 0.36.1). The breaking
redesign in [DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui) is in progress.
egui_extras was dropped in [DESIGN-9](#design-9-anchor-based-layout-without-egui_extras).

> **Branch `v0.3.0` is under heavy development.** Breaking changes are not only accepted but
> preferred whenever they lead to a better design: no compatibility shims, deprecation paths or
> adapters for the old API. Every known downstream user will be ported.

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

## Crate layout

| Path | Purpose |
|------|---------|
| `tabular_core/` | egui-independent core types: `TableBackend` trait, `ColumnUid`, `RowUid`, `CellCoord`, `BackendColumn`, `PersistentFlags`, `OneShotFlags`, `CellMetadata`, `Rgb`, `WrapMode`, `CsvImporterConfig`. Re-exports `rvariant::{Variant, VariantTy}`. |
| `tabular_derive/` | `#[derive(TabularRow)]` proc macro: turns `Vec<Row>` into a read-only backend + frontend. |
| `src/table_view.rs` | `TableView::show()`: header and body layout (own cell layout, no egui_extras), column DnD and resize handles. |
| `src/table_view/layout.rs` | `RowLayout` (anchor-based vertical scrolling, row order, row heights by `RowUid`) and `ColumnWidths` (by column). Unit-tested. |
| `src/table_view/scroll_bar.rs` | Row-proportional vertical scroll bar. |
| `src/table_view/interaction.rs` | Keyboard handling, copy, paste (+ paste modal), selection moves. |
| `src/table_view/state.rs` | `State` and `SelectedRange` (selection + editing coord). |
| `src/table_view/tool_column.rs` | Left "tool" column: row numbers and context menus. |
| `src/table_view/config.rs` | `TableViewConfig` (serializable view settings). |
| `src/frontend.rs` | `TableFrontend` trait: cell view/editor UI, colors, tooltips, corners. |
| `src/backends/variant.rs` | `VariantBackend`: in-memory `HashMap<CellCoord, Variant>` backend + frontend. |
| `src/importers/` | `TabularImporter` (file picker + CSV options UI), `CsvImporter`, `RequiredColumns` (name/synonym mapping). |
| `src/util.rs` | `base_26` column names, encoding detection, CSV export. |
| `demos/simple`, `demos/derive_row`, `demos/csv_xls_import` | Example apps. |
| `tests/ui/` | Integration test target `ui` of the root crate: headless UI tests (egui_kittest): selection, editing, keyboard, column drag & resize, paste. `fixture.rs` holds the harness helpers. Known bugs have `#[ignore = "<ID>: ..."]` repro tests. See AGENTS.md, "UI tests". |
| `tests/Cargo.toml`, `tests/src/` | Workspace package `tests`: compile test for the derive macro. |

The view is generic over `T: TableBackend + TableFrontend`. The backend owns data, column info and
(today) the in-progress edit buffer. The view owns visual column order, column widths, the row
display order with the scroll anchor and row heights, selection, and paste state.

---

## Feature inventory

### Core data model / backend trait

| Feature | Status | Notes |
|---------|--------|-------|
| Generic `TableBackend` trait (rows/cols by uid, visual index → uid mapping) | ✅ done | See [DESIGN-3](#design-3-tablebackendcol_uid-conflicts-with-view-owned-column-order) for `col_uid`. |
| `get` / `set` cells as `Variant` | ✅ done | Optional; copy and CSV export depend on `get`. |
| Create row / create column | ✅ done | `VariantBackend` names new columns `A`, `B`, ... via `base_26`. |
| Row / column skipping (strike-through, excluded from export) | 🐛 buggy | Skipped columns are still exported ([EXPORT-3](#export-3)). |
| Cell metadata: background color, corner triangle, multiple tooltips, wrap mode | ✅ done | `set_metadata(coord, meta, merge)`. |
| Change/flag notification (`PersistentFlags`, `OneShotFlags`) | 🐛 buggy | Error-prone; see [DESIGN-1](#design-1-flags-system), [FLAGS-*](#flags-and-change-notification). |
| Read-only tables | 🐛 buggy | `VariantBackend::set_read_only` has no effect ([BACKEND-1](#backend-1)); several buttons ignore read-only ([VIEW-4](#view-4)). |
| Remote/lazy backends | 💡 idea | The never-called `reload`, `poll`, `commit_all`, `commit_immediately` stubs were removed in step 0. Server-side sorting has a planned hook ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui), "Row order"). |
| Column "used" vs "available" | 🚧 partial | `used_columns()` exists, but `VariantBackend` doesn't override it. `use_column` (a no-op) was removed in step 0. DESIGN-8 folds this into `ColumnInfo::is_used`. |
| Undo / redo | 📋 planned | |
| Sorting | ⬜ stub | `is_sortable`, "Sort ascending/descending" menu items exist but do nothing. Planned in the view ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui)). |
| Filtering | 📋 planned | Planned in the view ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui)). |

### `VariantBackend`

| Feature | Status | Notes |
|---------|--------|-------|
| In-memory storage, column defaults applied on row insert | ✅ done | |
| `turn_column_into(ty)` with conversion errors shown as orange cells + tooltip | ✅ done | Code-only API, no UI. |
| Viewers: Str, StrList, Bool, Enum, numbers, others via `Display` | 🐛 buggy | Bool viewer is a live checkbox ([EDIT-7](#edit-7)). |
| Editors: Str (TextEdit), Bool, Enum (ComboBox), U32/U64/I32/I64 (DragValue) | 🚧 partial | No editor for F32/F64 or other `Number` widths ([EDIT-8](#edit-8)). Enum doesn't commit on selection ([EDIT-6](#edit-6)). |
| Column mapping choices (combo box above columns) | ✅ done | Stored in `TableViewConfig::column_mapped_to`, keyed by `ColumnUid`. |
| Date, SI values, currency viewers/editors | 📋 planned | |

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
| Column header context menu | 🚧 partial | Only "Skip" works. "Sort", "Hide" are stubs; "Add column" is only shown for sortable columns ([VIEW-5](#view-5)). |
| Column drag & drop reorder | 🐛 buggy | Swaps instead of moves; widths, selection and order persistence broken ([DND-*](#column-drag--drop)). |
| Tool column (row numbers, row context menu: append, skip) | ✅ done | |
| Tool column header menu: Export CSV, Append row, Clear | 🐛 buggy | "Clear" has no confirmation ([VIEW-6](#view-6)). |
| "No columns" state with "Create column" button | 🐛 buggy | Ignores read-only / creation support ([VIEW-4](#view-4)). |
| Multiple tables in one parent `Ui` | ✅ done | All view ids derive from the `id` passed to `show`. |
| Cell background colors / corner triangles / tooltips | ✅ done | |
| Selection: single cell, rectangle, whole row, select all | 🐛 buggy | See [SEL-*](#selection-and-keyboard). |
| Scroll selection into view on keyboard navigation | 🚧 partial | Vertical only (arrow keys). No horizontal reveal yet. |
| Page Up / Page Down / Home / End | 📋 planned | Trivial with the anchor: move it by what fit on screen. |
| Stick-to-bottom for live data | 🚧 partial | Always on: if the end of the table is in view, appended rows keep it there. Not configurable. |
| Scroll to a newly appended row | ✅ done | `N`, "Append row" in the tool column menus and the "Add row" button. |
| Visual state persistence (`TableViewConfig` is serde) | 🚧 partial | Column order and widths are not persisted ([DND-5](#dnd-5)). |
| Custom column header UI (`TableFrontend::custom_column_ui`) | ✅ done | |
| Per-column render config | 📋 planned | The never-called `TableFrontend::column_render_config` was removed in step 0. Widths become per-column view state ([DESIGN-4](#design-4-column-order-and-widths-as-persisted-view-state)). |

### Editing

| Feature | Status | Notes |
|---------|--------|-------|
| Click selected cell to edit; `E` to edit | 🐛 buggy | See [EDIT-*](#cell-editing). |
| Enter commits, Escape cancels, Tab commits and edits next cell | 🐛 buggy | [EDIT-1](#edit-1) – [EDIT-4](#edit-4), [EDIT-6](#edit-6). |
| Commit on focus loss / click outside | 📋 planned | [EDIT-3](#edit-3). |
| Start editing by typing / Enter / F2 / double-click | 📋 planned | [EDIT-9](#edit-9). |

### Clipboard

| Feature | Status | Notes |
|---------|--------|-------|
| Copy selection as TSV (Ctrl+C) | 🐛 buggy | Row selections add trailing tabs ([SEL-1](#sel-1)); uses `ctrl` not `command` ([SEL-5](#sel-5)). |
| Paste TSV block into selection | 🐛 buggy | See [PASTE-*](#paste). |
| Paste into empty table creates columns and rows | ✅ done | |
| Paste dialog for size mismatch (create rows, repeat fill, create columns) | 🐛 buggy | [PASTE-1](#paste-1) – [PASTE-4](#paste-4). |
| Paste "overflow" mode | 📋 planned | Commented-out button in `handle_paste_continue`. |

### Import / export

| Feature | Status | Notes |
|---------|--------|-------|
| `TabularImporter` UI: file picker, reload, separator, header row, skip N rows | ✅ done | |
| CSV import with auto separator detection | ✅ done | Counts `,` `\t` `;` in the first MiB. |
| Encoding detection (chardetng) | 🐛 buggy | Never reads any bytes ([IMPORT-1](#import-1)). |
| Required columns mapped by name/synonym (case-insensitive) | ✅ done | Header names are not trimmed ([IMPORT-4](#import-4)). |
| CSV without header row | 🐛 buggy | Last column dropped ([IMPORT-2](#import-2)). |
| Ragged CSV (rows wider than header) | 🐛 buggy | Extra cells land in wrong columns ([IMPORT-3](#import-3)). |
| Preview mode (`set_max_lines`) | ✅ done | |
| XLS/XLSX import | 📋 planned | Demo is named `csv_xls_import` but only CSV exists. |
| Export CSV | 🐛 buggy | See [EXPORT-*](#export). |
| Export XLS/XLSX | 💡 idea | |

### Derive macro (`#[derive(TabularRow)]`)

| Feature | Status | Notes |
|---------|--------|-------|
| Generates `<Row>TabularBackend` with `new(Vec<Row>)`, read-only | 🐛 buggy | Struct is private ([DERIVE-2](#derive-2)). |
| Column names from field names (Sentence case), type shown in header | ✅ done | |
| `#[format = "..."]` per field | 🐛 buggy | Output wrapped in literal quotes ([DERIVE-1](#derive-1)). |
| Copy / CSV export from derived tables | 📋 planned | `get()` not generated ([DERIVE-4](#derive-4)). |

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
| Drag column right edge | — | Resize the column (full table height is the handle). |
| Double-click column right edge | — | Return the column to auto width and re-fit it to the header and visible rows, eased (tool column: default width). |
| Mouse wheel | Pointer over table | Scroll rows; Shift+wheel scrolls horizontally. |
| Drag / click scroll bar | — | Drag the thumb, or click the track to jump there. |
| Arrows (+Shift) | Not editing | Move (grow) selection and scroll the moved edge into view. Panics with 0 rows ([SEL-2](#sel-2)). |
| `E` | Not editing, not read-only, single cell | Start editing. Wrong column after reorder ([EDIT-4](#edit-4)). |
| `N` | Not editing, not read-only | Append row and scroll to it. Also fires while typing in other widgets ([SEL-4](#sel-4)). |
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
- Test (ignored, fails): `editing::row_number_click_commits_edit`.

#### EDIT-2
**Escape with the pointer outside the table doesn't cancel in the backend.** — *major*
- Where: `show_body` Escape branch (`table_view.rs:547`) vs `handle_key_input_when_editing` (`interaction.rs:80-85`).
- Cause: the `show_body` branch only sets `editing = None`; only the interaction handler calls `cancel_edit()`, and that handler runs only when the pointer is over the table. Same stale-buffer symptoms as EDIT-1.
- Test (ignored, fails): `editing::escape_away_from_table_cancels_in_backend` (the re-opened editor shows the abandoned text).

#### EDIT-3
**Clicking outside the table, or Tab-ing focus away, leaves the editor open but unfocused.** — *major*
- Where: `show_body`.
- Cause: the editor response's `lost_focus()` is never checked. Keystrokes go nowhere until the user clicks back into the cell. With the pointer outside the table, Tab is handled by egui focus traversal instead of the table.
- Fix: commit (or cancel, configurable) on `lost_focus()`.

#### EDIT-4
**Tab and `E` compute the edit coord from backend column order, not visual order.** — *data-loss*
- Where: `handle_key_input_when_editing` (`interaction.rs:73`), `handle_selection_moves` (`interaction.rs:348`).
- Cause: they call `data.col_uid(VisualColIdx(col))`, which indexes the backend's column list. The view renders `state.columns_ordered`, which differs after a column drag. For `VariantBackend`, `col_uid` also counts unused columns. For derived backends it always returns `None`, so Tab/E never work there.
- Effect: the editor is drawn at the right visual cell (rendering matches positionally), but the stored coord is wrong. `VariantBackend::commit_cell_edit(coord)` requires `last_edited_coord == coord`, so the **edit is silently discarded**. Enter still commits correctly (it uses the rendered coord); the loss happens when the edit is left by clicking another cell or by Tab, which commit the stored coord.
- Fix: use `self.state.columns_ordered[col_idx]`; remove `TableBackend::col_uid` ([DESIGN-3](#design-3-tablebackendcol_uid-conflicts-with-view-owned-column-order)).
- Test (ignored, fails): `editing::e_edits_visual_column_after_reorder`, `editing::tab_edits_visual_column_after_reorder`.

#### EDIT-5
**Editor is taller than the row: rows jump or the editor is clipped.** — *major (visual)*
- Where: `TableViewConfig::default` (`minimum_row_height = 15.0`), `show_body` row heights.
- Cause: TextEdit/DragValue are about 20px plus frame. With heterogeneous heights the row grows when the editor opens (rows below shift) and shrinks back after commit. Since DESIGN-9 there is no frame of overflow any more: rows below are placed after the measured row. With fixed heights the editor is clipped.
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
- Test (ignored, fails): `editing::f2_starts_editing`.

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
- Test (ignored, fails): `selection::row_selection_copies_exactly_the_row`.

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

#### VIEW-4
**Buttons ignore capabilities.** — *minor*
- "Add row" under an empty table and "Create column" in the no-columns state are shown regardless of `is_read_only` or whether `create_row`/`create_column` are supported.

#### VIEW-5
**Column context menu stubs.** — *minor*
- "Sort ascending/descending" and "Hide" do nothing. "Add column" is nested inside `if col.is_sortable` (`table_view.rs:333-344`).

#### VIEW-6
**"Clear" has no confirmation.** — *minor*
- TODO in `tool_column.rs`. A commented-out modal exists in `interaction.rs` (`handle_clear_request`).

#### VIEW-8
**No touch drag-to-scroll.** — *minor*
- Since DESIGN-9 the body is not an egui `ScrollArea`, so dragging the body on a touch screen doesn't scroll. Cells sense clicks only, so a body drag could be mapped to `RowLayout::scroll_by` (or used for drag-selection on desktop and drag-scroll on touch).

#### VIEW-7
**Selection styling uses `warn_fg_color`.** — *minor (visual)*
- Selection fill and borders use the warning color rather than `visuals.selection`. Vertical selection borders are commented out, so multi-column selections have no side edges.

### Flags and change notification

See [DESIGN-1](#design-1-flags-system) for the planned replacement.

#### FLAGS-1
**One-shot flags are consumed by the view.** — *major*
- `show()` archives and zeroes them. A second consumer (a second `TableView`, or app code reading after another view) never sees them, which led to stale columns and the VIEW-1 panic (fixed by DESIGN-9). If the view isn't shown (collapsed tab), the delayed copy never updates.

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
- Test (ignored, fails): `editing::read_only_table_does_not_edit`.

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

#### Principles

1. **The model is data only.** `TableModel` lives in `tabular_core` and has no egui dependency.
2. **The view owns all presentation state:** column order and widths, row order (sort/filter),
   selection, the edit buffer, focus, and the paste dialog. Nothing the user sees is ordered by the
   model.
3. **The view never mutates the model while rendering.** Every view-originated change is pushed to a
   command queue, and the queue is applied once per frame, after the body has rendered. This prevents
   VIEW-1-style desyncs by construction (VIEW-1 itself was fixed by DESIGN-9) and is the hook for undo/redo.
4. **Change detection uses revision counters that are never consumed.** Any number of views and app
   code can each compare against their own last-seen value.

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
    pub remove_rows: bool,
    pub create_columns: bool,
    pub clear: bool,
    pub skip_rows: bool,
    pub skip_columns: bool,
}

/// Replaces BackendColumn (renamed; `is_skipped` is the only source of truth for column skip).
pub struct ColumnInfo {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: String,
    pub is_sortable: bool,
    pub is_required: bool,
    pub is_used: bool,
    pub is_skipped: bool,
}

pub enum ModelError { Unsupported, TypeMismatch { expected: VariantTy }, Other(String) }

pub trait TableModel {
    fn revision(&self) -> Revision;
    fn capabilities(&self) -> Capabilities;

    /// Natural column order. The view starts from this and then applies the user's order.
    fn columns(&self) -> impl Iterator<Item = ColumnUid>;
    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo>;
    /// Natural row order (insertion / file order). Never sorted or filtered by the view's state.
    fn rows(&self) -> impl Iterator<Item = RowUid>;
    fn row_count(&self) -> usize { self.rows().count() } // override when O(1)

    /// `Cow` so that computed models (derive macro) can return owned values and stored
    /// models can return references (sorting 1M rows must not clone every string).
    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>>;
    fn metadata(&self, coord: CellCoord) -> Option<Cow<'_, CellMetadata>> { None }
    fn is_row_skipped(&self, row: RowUid) -> bool { false }

    // Mutations. Defaults return Err(Unsupported); the view checks capabilities() first.
    fn set(&mut self, coord: CellCoord, value: Variant) -> Result<(), ModelError>;
    fn create_row(&mut self, values: Vec<(ColumnUid, Variant)>) -> Result<RowUid, ModelError>;
    fn remove_rows(&mut self, rows: &[RowUid]) -> Result<(), ModelError>;
    fn create_column(&mut self) -> Result<ColumnUid, ModelError>;
    fn clear(&mut self) -> Result<(), ModelError>;
    fn skip_rows(&mut self, rows: &[RowUid], skipped: bool) -> Result<(), ModelError>;
    fn skip_column(&mut self, col: ColumnUid, skipped: bool) -> Result<(), ModelError>;

    /// Optional delegated ordering, see "Row order". None = the view sorts locally.
    fn sorted_rows(&self, keys: &[SortKey]) -> Option<Vec<RowUid>> { None }
}
```

Removed from the contract: `PersistentFlags`, `OneShotFlags` and the five flag methods,
`col_uid(VisualColIdx)`, `row_uid(VisualRowIdx)`, `VisualRowIdx`/`VisualColIdx` (they become
view-internal), `available_columns`/`used_columns`, `is_clearable`, `is_col_skipped`,
`un_skip_all_*` (use the slice forms), `commit_cell_edit`, `column_mapping_choices` (moves to view
options), and `set_metadata` (becomes inherent on `VariantTable`; the trait only reads metadata).
`un_skipped_rows()` stays as a provided method.

#### Cell UI (`egui_tabular`)

```rust
pub trait CellUi<M: TableModel> {
    /// View mode. `&mut M` because cells may host live interactive widgets
    /// (e.g. hardware state polled through the model).
    fn show_cell(&mut self, model: &mut M, coord: CellCoord, ui: &mut Ui);
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
  uniformly, so `CellUi` implementors don't reimplement the lifecycle.

#### View

```rust
pub struct TableViewOptions {
    pub id_salt: Id,              // salts every view id, so several tables can share a Ui
    pub max_height: Option<f32>,
    pub tool_column: bool,
    pub read_only: bool,          // view-level, on top of model capabilities
    pub stick_to_bottom: bool,
    pub column_mapping_choices: Vec<String>,
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
}
```

**Command queue.** Commands are view-level intents, applied in one place after rendering:

```rust
pub enum TableCommand {
    Set { coord: CellCoord, value: Variant },
    Paste { anchor: (RowUid, ColumnUid), block: Vec<Vec<String>>, mode: PasteMode },
    CreateRows { count: usize },
    CreateColumn,
    RemoveRows(Vec<RowUid>),
    Clear,
    SkipRows { rows: Vec<RowUid>, skipped: bool },
    SkipColumn { col: ColumnUid, skipped: bool },
}
```

`Paste` is one compound command because it may create columns and then write into them. Its apply
step calls `create_column`, `create_row` and `set` in sequence. Possible later extensions (not in
step 1): `apply` returns the inverse operations (undo), and a `defer_commands` option returns the
queue in `TableViewOutput` instead of applying it, for apps that route edits to a server.

**Row order.**

- The view holds `visible_rows: Vec<RowUid>`. It is rebuilt when `revision.rows` changes, when
  `config.sort` or the filter changes, or when `revision.cells` changes while a sort or filter is
  active. The pipeline is: `model.rows()` → filter → sort (stable). Sorting uses `model.get()` and a
  crate-local Variant comparator, because `rvariant::Variant` doesn't implement `Ord`. Numbers
  compare by value, strings case-insensitively, and empty values sort last.
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

**Selection.** `Selection { anchor: (RowUid, ColumnUid), cursor: (RowUid, ColumnUid), kind: Cells | Rows }`.
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
  10k-row table doesn't rebuild the uid index.

**Fits DESIGN-8:** `RowLayout::rows` is the view-owned row order. Sorting/filtering (step 2) only
replaces how that `Vec<RowUid>` is built. Paged or unbounded models (the "known limit" in DESIGN-8)
would replace the `Vec` with model-driven next/prev stepping from the anchor; the layout loop
already only walks outwards from the anchor.

**Not done yet:** horizontal reveal, Page Up/Down/Home/End, touch drag-to-scroll ([VIEW-8](#view-8)),
persisting widths ([DESIGN-4](#design-4-column-order-and-widths-as-persisted-view-state)), a sticky
tool column during horizontal scroll.

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
- README "Keyboard shortcuts" and "Features" lists should be derived from this file.

---

## Roadmap

Suggested order; update as items land. Breaking changes are preferred whenever they improve the
design (all users will be ported), so the API redesign comes **first**: most crash and data-loss bugs are fixed by it
structurally, so patching them in the old code first would be wasted work.

0. ✅ **Housekeeping** — done: dead modules deleted, never-called trait stubs removed, missing
   derives, `Cargo.toml` versions, `.idea/` untracked.
1. 📋 **Core contract** ([DESIGN-8](#design-8-core-contract-rework-tablemodel--cellui)): `Revision`, `Capabilities`, `TableModel`, `CellUi` + `VariantCellUi`,
   command queue. Port `VariantBackend` → `VariantTable`, the derive macro, the importer and the
   three demos. Fixes FLAGS-1..5, EDIT-4, BACKEND-1, BACKEND-2, DERIVE-2..4, PASTE-5.
2. 🚧 **View state rewrite:** uid selection with anchor/cursor, view-owned edit buffer, focus-based
   input, `TableViewOptions`, view-owned row order (sorting) built into `RowLayout`. Fixes
   EDIT-1..3, EDIT-5..7, EDIT-9, SEL-1..5, VIEW-4, VIEW-5, DND-3. The anchor-based layout
   ([DESIGN-9](#design-9-anchor-based-layout-without-egui_extras)) landed ahead of this step.
3. 📋 **Column order and widths** (DESIGN-4): DND-1, DND-4, DND-5, DND-6; persist `ColumnWidths`.
4. 📋 **Paste/export/import:** paste via the `csv` crate (PASTE-1..4), `write_csv(model, order, impl
   Write) -> Result` (EXPORT-1..4), `rfd` behind a feature (DESIGN-7), IMPORT-1..5. Independent of
   steps 1–3; can land any time.
5. 📋 **Features:** filtering, undo/redo (inverse commands), horizontal scroll-into-view, Page Up/Down,
   touch drag-to-scroll (VIEW-8), XLSX import, more
   editors (EDIT-8), confirmation for Clear (VIEW-6), selection colors (VIEW-7).

Small fixes that are independent of the redesign and can go in at any point: DERIVE-1, DERIVE-5
(the macro is rewritten in step 1 anyway, so fold them in there), IMPORT-2, IMPORT-3.

---

## Fixed issues

Move entries here when fixed (keep the ID, add the commit hash and a one-line note).

| ID | Fixed in | Note |
|----|----------|------|
| VIEW-1 | `bf0d2d8` | Body iterates the view's row order, resynced after the header and on row count change; no `unwrap`. (DESIGN-9) |
| VIEW-2 | `bf0d2d8` | Row heights cached by `RowUid`. (DESIGN-9) |
| VIEW-3 | `bf0d2d8` | No egui_extras state; all ids derive from the `id` passed to `show`. (DESIGN-9) |
| VIEW-9 | `bf0d2d8` | `N` blinked when the view was at the end: the post-header sync moved the anchor to the end (stick to bottom) and the body rendered it unclamped, i.e. empty, for one frame. Now normalized after every sync. (DESIGN-9) |
| EDIT-10 | `0205e1e` | Clicking on a cell's text didn't select or edit it, only the empty part of the cell did: labels are selectable by default and sense clicks above the cell. Cells and headers now disable `selectable_labels`, which also covers custom `TableFrontend` UI. Found by the egui_kittest suite (`selection::click_selects_cell`). |
| DND-2 | `bf0d2d8` | Column widths keyed by `ColumnUid`. (DESIGN-9) |

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
