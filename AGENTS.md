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

- `tabular_core/` — egui-free core: `TableBackend` trait, uid/coord types, flags, cell metadata.
- `tabular_derive/` — `#[derive(TabularRow)]` proc macro.
- `src/` — the `egui_tabular` crate: `TableView` (`src/table_view*`), `TableFrontend`
  (`src/frontend.rs`), `VariantBackend` (`src/backends/variant.rs`), CSV import (`src/importers/`),
  utilities (`src/util.rs`).
- `demos/simple`, `demos/derive_row`, `demos/csv_xls_import` — runnable examples.
- `tests/` — derive macro compile test.

FEATURES.md has a more detailed layout table.

`rvariant` is a path dependency on `../rvariant`; a sibling checkout is required to build.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo run -p simple            # main interactive demo (10k rows, editable)
cargo run -p derive_row        # derive macro demo
cargo run -p csv_xls_import    # importer demo
just dry-publish               # publish check (excludes demos/tests)
```

UI behavior (editing, drag & drop, selection) can only be confirmed by running a demo. When you
can't run the GUI, say so, and describe the manual check in the PR.

## Conventions and pitfalls

- Match the surrounding code style; run `cargo fmt`.
- The egui version is listed in the README compatibility table. egui_extras is no longer used: the
  view lays out rows and columns itself ([DESIGN-9](FEATURES.md#design-9-anchor-based-layout-without-egui_extras)).
- **A breaking redesign is in progress: [DESIGN-8](FEATURES.md#design-8-core-contract-rework-tablemodel--cellui).**
  New code should move toward it, not extend the old `TableBackend`/`TableFrontend` API or the
  flag system. Don't patch bugs that DESIGN-8 removes structurally (see the roadmap).
- Until DESIGN-8 lands, these pitfalls apply to the old code:
  - The view owns visual column order (`State::columns_ordered`). Don't map visual indices to
    columns through the backend (`TableBackend::col_uid`), which is the cause of EDIT-4.
  - Editing state has two owners (view `SelectedRange.editing` and backend edit buffer). Any code
    path that leaves edit mode must commit or cancel in the backend too.
  - The view's row order (`RowLayout`) is rebuilt on `OneShotFlags::row_set_updated` or a row
    count change. Never `unwrap()` `row_uid()`.
- Avoid usize underflow in selection math (`count - 1` with empty tables).
