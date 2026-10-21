# Handoff: tabular-sort (P2620#21oct-1114c)

## Done (branch s/21oct2026-tabular-sort)
- Cherry-picked origin/v0.3.0 (DESIGN-8 rework); reverted its AGENTS.md version-bump rule (main reverted it on purpose).
- SORT-1/FILTER-1/VIEW-12/VIEW-13 core (7ad7e2b): src/row_order.rs pipeline, TableViewConfig sort/filter/column_order/column_widths/hidden_columns, RowClicked etc.
- EDIT-13 date cells (from child date-cells, cherry-picked; DateRange folded into ColumnFilter::Range, 20f2dfd).
- SORT-2 header sort control + menu (690813d), pixel snapshot tests/snapshots/sort_controls.png.
- VIEW-14 hide + chooser, DND-1/DND-3 (from child hidden-columns-ui, cherry-picked 281998b).
- SEL-2 fixed as a side effect of the selection remap.

## Next
- Child sort-filter-demo (P2620#21oct-1126c, omarchy-m1, branch s/21oct2026-sort-filter-demo in ~/git/.wt/sort-filter-demo): when ready, fetch from omarchy-m1 and cherry-pick its commits here, run just test/lint.
- Then git rm HANDOFF.md, tpm work ready.

## Gotchas
- Children on omarchy-m1 fetch this branch with `git fetch gpd-omarchy:/tmp/wt/tabular-sort/egui_tabular s/21oct2026-tabular-sort`; their branches carry this branch's commits, so take only their own commits.
- omarchy-m1's ../rvariant is on v0.4.0 (needs si_dynamic 0.1): children point rvariant at a main worktree.
- Snapshot test needs a wgpu adapter (works on gpd-omarchy).
