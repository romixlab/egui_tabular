# Handoff: tab-row-spans (VIEW-16, P2529#25oct-1529)

## Done (commit 1c7a828)
- `tabular_core`: `RowSpan { first, rows }`, `TableModel::row_span(coord)` (default None), re-exported from `egui_tabular`.
- `src/table_view/spans.rs`: `span_at` (display rows of the covering span; only while `row_natural.is_none()`), `State::{span_at, unit_at, selection_is_unit, grow_selection_to_spans}`, `SpanCell` + `show_span_cells` (drawn after all rows, opaque panel fill + level/selection tint, outline, click selects span, RowClicked with first row, tooltips). Unit tests at the bottom.
- `show_body`: covered cells get a hidden Sense::hover child Ui and `shown: false`; a `SpanCell` is pushed at the span's first row or at the first row in view (`row_idx == start`), so no upward scan. Heights from `RowLayout::height_of(from, to, cap)` (cap = 2 x body height).
- `layout.rs`: `Reveal::Range`, `reveal_range`, `height_of`, unit tests.
- `interaction.rs`: arrows move a unit (single cell or exactly one span) onto the next cell/span; else old behavior; then `grow_selection_to_spans`. `show_table` also grows before key input (covers Shift+click from last frame).
- Tests: `tests/ui/spans.rs` (10 tests), fixture `scroll_to_row`. Demo `demos/row_spans` (+ justfile publish excludes). FEATURES.md (non-goal narrowed, inventory rows, keyboard table, DESIGN-9 notes, layout table), CHANGELOG [Unreleased], README.

## Blocker
- build-1 has no pkg-config and no wayland libs: rfd -> wayland-sys build script fails, nothing compiles natively. Told parent (needs root: `pacman -S --needed pkgconf wayland libxkbcommon`). Only `cargo check --target wasm32-unknown-unknown -p egui_tabular` was possible (2 pre-existing rfd FileDialog errors, my code typechecks).

## Next
1. Once libs are there: `cargo build --workspace`, `cargo test --test ui spans`, `cargo test -p egui_tabular --lib spans layout`, fix what fails (tests were never run!). Check each kittest asserts really observe the span (e.g. `span_starting_above_the_viewport...` relies on scroll_to_row(38) putting a10 off screen).
2. `cargo run -p row_spans` can't be checked here (no display): describe the manual check in the ready note.
3. Rebase onto main (sibling sessions landing: sort-filter-demo, date-cells, tab-pinned-cols touches show_body slots), `just test --changed`, `just lint`, `git rm HANDOFF.md`, `tpm work ready --note ...`, `tpm work say parent` in a few lines.

## Decisions
- Spans are in the natural row order; sorted/filtered views ignore them (documented on `row_span`).
- Span content doesn't grow row heights; it is clipped to the span rect.
- Whole-row selections are not grown to spans.
