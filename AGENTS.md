# Working on egui_tabular

Guidance for AI agents and contributors. Read this before changing code.

## Branch `v0.3.0` is under heavy development

Breaking changes are not just accepted here, they are preferred whenever they lead to a better
design. Don't add compatibility shims, deprecation paths or adapters for the old API, and don't
keep an awkward structure only to avoid breaking callers. Every known downstream user will be
ported. If a cleaner design needs an API break, make the break and record it in FEATURES.md.

## FEATURES.md is the source of truth

[FEATURES.md](FEATURES.md) records every feature and its status, every known bug (with a stable ID
such as `EDIT-4` or `DND-3`), the planned design rework, and the roadmap.

- **Read the relevant sections before starting work.** A "new" bug is often already recorded, and
  the planned design (`DESIGN-*`) may change how a fix should be done.
- **Keep it updated in the same commit as the code change.** This is required, not optional:
  - Fixed a bug → move it to "Fixed issues" with the commit hash, and update inventory/roadmap rows that link to it.
  - Added or changed a feature → update its inventory row and status; update the keyboard/mouse table if input behavior changed.
  - Found a bug you aren't fixing → add it with the next free ID for its prefix.
  - Never renumber or reuse IDs.
- Reference bug IDs in commit messages (e.g. `fix(view): resync row heights on row count change (VIEW-1)`).
- README.md is the public summary; when a user-visible feature or shortcut changes, update it too,
  following FEATURES.md.

## Repository layout

Cargo workspace (edition 2024):

- `tabular_core/` — egui-free core: `TableModel` trait, `Revision`, `Capabilities`, `ColumnInfo`,
  uid/coord types, cell metadata.
- `tabular_derive/` — `#[derive(TabularRow)]` proc macro.
- `src/` — the `egui_tabular` crate: `TableView` (`src/table_view*`), `CellUi` + `VariantCellUi`
  (`src/cell_ui.rs`), `TableCommand` (`src/commands.rs`), `VariantTable`
  (`src/backends/variant.rs`), CSV import (`src/importers/`), utilities (`src/util.rs`).
- `demos/simple`, `demos/derive_row`, `demos/csv_xls_import` — runnable examples.
- `tests/ui/` — headless UI tests (egui_kittest), see [UI tests](#ui-tests).
- `tests/Cargo.toml` + `tests/src/` — the `tests` package: derive macro tests.

FEATURES.md has a more detailed layout table.

`rvariant` is a path dependency on `../rvariant`; a sibling checkout is required to build.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo test --test ui           # headless UI tests (~0.1 s, no GPU or display)
cargo test --test ui -- --ignored   # known-bug repros; all of them must fail
cargo run -p simple            # main interactive demo (10k rows, editable)
cargo run -p derive_row        # derive macro demo
cargo run -p csv_xls_import    # importer demo
just dry-publish               # publish check (excludes demos/tests)
```

UI behavior (selection, editing, keyboard, drag & drop, paste) is tested headlessly in `tests/ui/`.
Prefer adding a test there over a manual check. Only what the tests can't observe (visuals,
animation feel, real clipboard and window integration) still needs a demo run: when you can't run
the GUI, say so, and describe the manual check in the PR.

## UI tests

`tests/ui/` drives a real `TableView` + `VariantTable` (or any model, `Table::custom`) with
[egui_kittest](https://docs.rs/egui_kittest). The harness runs egui frames without a window,
injects input events and queries the AccessKit tree that egui builds every frame. Nothing is
rendered and no screenshots are taken.

### How it works

- **Finding things.** Widgets are found by their AccessKit label: cell values, header names and row
  numbers are `Label` nodes. `Table::grid(cols, rows)` fills every cell with its spreadsheet name
  ("A1", "B2", ...) and names columns "A", "B", ..., so every cell can be found by its text.
  `get_by_label` panics if there are zero **or several** matches, so keep texts unique. Cells
  themselves have no label or role yet (role `Unknown`); empty cells can only be reached by
  position (`Table::click_at`).
- **Clicking.** `Node::click()` sends a real pointer move, press and release at the node's center,
  so it goes through egui's normal hit testing, like a user's click. If a click on a label
  doesn't reach the cell, the label is eating it (EDIT-10). `click_accesskit()` bypasses hit
  testing; don't use it for table cells.
- **Frames and time.** Every queued event runs in its own frame, and each frame advances time by
  0.25 s. `Harness::run()` steps until nothing requests a repaint (eased widths, tooltips).
  Because of the 0.25 s step, two queued clicks are never a double click: `Table::double_click_at`
  puts both clicks into one frame's raw input. Drags need several pointer moves while pressed to
  pass egui's drag threshold (`Table::drag`).
- **Observing state.** Prefer what a user would see:
  - Selection: `Table::copy()` presses Ctrl+C and returns the copied TSV, which the fixture
    captures from the `CopyText` output command. `None` means nothing is selected.
  - Editing: `Table::editor()` is the `TextInput` node; `editor_text()` is its value.
  - Data: `Table::value(row, col)` reads the model; `has(text)` checks what is displayed.
  - Events: `t.h.state().events` collects every `TableEvent` the view reported.
  - Column order: `Table::header_order()` sorts header labels by x.

  Don't add accessors to `TableView` just for tests. The view's state is being rewritten
  (DESIGN-8), and black-box tests survive that.
- **Keyboard needs the pointer over the table (SEL-4).** Clicks leave the pointer where they
  clicked. `drop_at` and `pointer_away()` remove it, so call `hover(..)` before pressing keys
  after a drag. `CTRL` sets both `ctrl` and `command`, as egui-winit does on Linux/Windows.
- **Custom models.** `Table::custom(model, cell_ui)` runs the same helpers over any `TableModel`
  and `CellUi` (see `tests/ui/model.rs`). To customize the harness (OS, size), use
  `Table::grid_with(Harness::builder().with_os(..), ..)` or `Table::build`.

### Conventions

- Go through the helpers in `tests/ui/fixture.rs`; add a helper when you need a new interaction.
  When the API changes, only the fixture should need porting.
- One behavior per test, named after it (`enter_commits`, `drag_header_reorders_columns`). Put it
  in the module for its area: `selection`, `editing`, `keyboard`, `columns`, `paste`, `model`
  (the model contract, custom models and cell UIs, events).
- **Known bugs are executable.** A bug that can be reproduced through the UI gets a test asserting
  the *intended* behavior, marked `#[ignore = "EDIT-4: one-line summary"]`, and the bug entry in
  FEATURES.md gets a "Test" bullet pointing to it. Before committing, check with
  `cargo test --test ui -- --ignored` that each ignored test fails at the assertion that shows
  the bug, not earlier in setup. A repro that passes is wrong (or the bug is gone), and that's
  worth finding out: two first drafts in this suite passed and showed that DND-3 and EDIT-4 are
  narrower than first recorded.
- Fixing a bug: write or un-ignore its test first, see it fail, fix, see it pass, then update
  FEATURES.md.
- A new feature or input change gets a test, alongside the keyboard/mouse table update.
- Add pixel snapshots only for purely visual behavior that state assertions can't express. They
  need the `snapshot` + `wgpu` features of egui_kittest, a GPU or software renderer, and checked-in
  PNGs (`UPDATE_SNAPSHOTS=1` regenerates them), and they are sensitive to fonts and drivers. Keep
  them small and mask what isn't under test (`Harness::mask`).
- Debugging: `println!("{:#?}", t.h.root())` dumps the AccessKit tree. A failed `get_by_*` panics
  with the tree as well.

## Conventions and pitfalls

- Match the surrounding code style; run `cargo fmt`.
- The egui version is listed in the README compatibility table. egui_extras is no longer used: the
  view lays out rows and columns itself ([DESIGN-9](FEATURES.md#design-9-anchor-based-layout-without-egui_extras)).
- **A breaking redesign is in progress: [DESIGN-8](FEATURES.md#design-8-core-contract-rework-tablemodel--cellui).**
  The model contract (roadmap step 1) has landed; the view state rewrite (step 2) is next. Don't
  patch bugs that the next steps remove structurally (see the roadmap).
- The model is data only and egui-free (`tabular_core`). Everything the user sees is ordered and
  held by the view: column order (`State::columns_ordered`), row order (`RowLayout`), selection and
  the edit buffer (`State::edit`). Map visual indices through those, never through the model.
- The view never changes the model while drawing: queue a `TableCommand` in `State::commands`. The
  queue is applied after input handling and after the frame. Each command's `apply` returns its
  inverse (for undo); keep that true when adding commands. The only exception is custom `CellUi`
  widgets, which get `&mut M`.
- The view picks up model changes by comparing `TableModel::revision()` with the last seen one
  (`sync_model`). A model must bump the right counter on every change.
- Until step 2, selection is index-based (`SelectedRange`) and its editing coord must match
  `State::edit`: leave edit mode through `State::commit_edit` or by clearing the coord (the buffer
  is dropped at the end of the frame).
- Avoid usize underflow in selection math (`count - 1` with empty tables).
