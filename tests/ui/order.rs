//! Row order and saved view state: sort, filter, column order/widths/visibility from
//! `TableViewConfig`, row click events and programmatic selection (SORT-1, FILTER-1, VIEW-12,
//! VIEW-13).

use crate::fixture::{Table, col_name};
use egui::Key;
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    ColumnDef, ColumnFilter, ColumnUid, RowUid, SortKey, SortPolicy, TableEvent, TableModel,
    TableViewConfig, VariantTable,
};

const A: ColumnUid = ColumnUid(0);
const B: ColumnUid = ColumnUid(1);

/// Column A holds `names`, column B their 1-based natural position as text ("n1", "n2", ...).
fn named(names: &[&str]) -> Table {
    let mut table = VariantTable::new((0..2).map(|c| ColumnDef::new(col_name(c), VariantTy::Str)));
    for (i, name) in names.iter().enumerate() {
        table.insert_row([
            (A, Variant::Str(name.to_string())),
            (B, Variant::Str(format!("n{}", i + 1))),
        ]);
    }
    Table::custom(table, egui_tabular::VariantCellUi)
}

#[test]
fn config_sort_orders_rows() {
    let mut t = named(&["pear", "Apple", "fig"]);
    assert_eq!(
        t.top_down(&["pear", "Apple", "fig"]),
        ["pear", "Apple", "fig"]
    );
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    assert_eq!(
        t.top_down(&["pear", "Apple", "fig"]),
        ["Apple", "fig", "pear"]
    );
    t.config(|c| c.sort = vec![SortKey::descending(A)]);
    assert_eq!(
        t.top_down(&["pear", "Apple", "fig"]),
        ["pear", "fig", "Apple"]
    );
    t.config(|c| c.sort.clear());
    assert_eq!(
        t.top_down(&["pear", "Apple", "fig"]),
        ["pear", "Apple", "fig"]
    );
}

#[test]
fn row_numbers_stay_natural_when_sorted() {
    let mut t = named(&["b", "a"]);
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    // "a" was row 1 (0-based) in the model and still says so; the number sits on its row.
    let a_y = t.center("a").y;
    assert!((t.center("1").y - a_y).abs() < 1.0);
}

#[test]
fn quick_filter_hides_rows_and_counts() {
    let mut t = named(&["red apple", "green pear", "red cherry"]);
    t.config(|c| c.filter.quick = "RED".into());
    assert_eq!(
        t.top_down(&["red apple", "green pear", "red cherry"]),
        ["red apple", "red cherry"]
    );
    assert_eq!(t.h.state().view.row_counts(), (2, 3));
    // Terms match in any column: "n3" is in column B.
    t.config(|c| c.filter.quick = "red n3".into());
    assert_eq!(t.top_down(&["red apple", "red cherry"]), ["red cherry"]);
    t.config(|c| c.filter.quick.clear());
    assert_eq!(t.h.state().view.row_counts(), (3, 3));
}

#[test]
fn column_filter_one_of() {
    let mut t = named(&["open", "done", "blocked"]);
    t.config(|c| {
        c.filter.columns.insert(
            A,
            ColumnFilter::OneOf(["open".into(), "blocked".into()].into()),
        );
    });
    assert_eq!(
        t.top_down(&["open", "done", "blocked"]),
        ["open", "blocked"]
    );
}

#[test]
fn selection_follows_its_row_through_a_sort() {
    let mut t = named(&["c", "a", "b"]);
    t.click("c");
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    t.hover("a");
    assert_eq!(t.copy().as_deref(), Some("c"));
    assert_eq!(t.h.state().view.active_row(), Some(RowUid(0)));
}

#[test]
fn selection_of_a_filtered_out_row_is_dropped() {
    let mut t = named(&["c", "a"]);
    t.click("c");
    t.config(|c| c.filter.quick = "a".into());
    t.hover("a");
    assert_eq!(t.copy(), None);
}

#[test]
fn own_edit_keeps_the_row_in_place() {
    let mut t = named(&["a", "b", "c"]);
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    t.start_edit("a");
    t.type_replace("z");
    t.press(Key::Enter);
    // OnExternalChange: this view's edit doesn't move the row.
    assert_eq!(t.top_down(&["z", "b", "c"]), ["z", "b", "c"]);
    t.h.state_mut().view.refresh_rows();
    t.h.run();
    assert_eq!(t.top_down(&["z", "b", "c"]), ["b", "c", "z"]);
}

#[test]
fn live_policy_resorts_on_own_edit() {
    let mut t = named(&["a", "b", "c"]);
    t.h.state_mut().view.options_mut().row_order.policy = SortPolicy::Live;
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    t.start_edit("a");
    t.type_replace("z");
    t.press(Key::Enter);
    assert_eq!(t.top_down(&["z", "b", "c"]), ["b", "c", "z"]);
}

#[test]
fn external_change_resorts() {
    let mut t = named(&["a", "b", "c"]);
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    let row = t.rows[0];
    t.h.state_mut()
        .table
        .set((row, A).into(), Variant::Str("z".into()))
        .unwrap();
    t.h.run();
    assert_eq!(t.top_down(&["z", "b", "c"]), ["b", "c", "z"]);
}

#[test]
fn on_demand_policy_waits_for_refresh() {
    let mut t = named(&["a", "b"]);
    t.h.state_mut().view.options_mut().row_order.policy = SortPolicy::OnDemand;
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    let row = t.rows[0];
    t.h.state_mut()
        .table
        .set((row, A).into(), Variant::Str("z".into()))
        .unwrap();
    t.h.run();
    assert_eq!(t.top_down(&["z", "b"]), ["z", "b"]);
    assert!(t.h.state().view.is_row_order_stale());
    t.h.state_mut().view.refresh_rows();
    t.h.run();
    assert_eq!(t.top_down(&["z", "b"]), ["b", "z"]);
    assert!(!t.h.state().view.is_row_order_stale());
}

#[test]
fn row_click_reports_the_row() {
    let mut t = named(&["x", "y"]);
    t.click("y");
    assert!(t.saw(|e| *e
        == TableEvent::RowClicked {
            row: RowUid(1),
            col: Some(A)
        }));
    assert_eq!(t.h.state().view.active_row(), Some(RowUid(1)));
    assert_eq!(t.h.state().view.selected_rows(), [RowUid(1)]);
    t.click("0");
    assert!(t.saw(|e| *e
        == TableEvent::RowClicked {
            row: RowUid(0),
            col: None
        }));
    assert_eq!(t.h.state().view.active_row(), Some(RowUid(0)));
}

#[test]
fn row_double_click_is_reported() {
    let mut t = named(&["x", "y"]);
    let pos = t.center("y");
    t.double_click_at(pos);
    assert!(t.saw(|e| matches!(e, TableEvent::RowDoubleClicked { row: RowUid(1), .. })));
}

#[test]
fn select_row_selects_without_an_event() {
    let mut t = named(&["x", "y"]);
    t.h.state_mut().view.select_row(RowUid(1));
    t.h.run();
    assert_eq!(t.h.state().view.active_row(), Some(RowUid(1)));
    // The app made the change, so the view doesn't report it back.
    assert!(!t.saw(|e| matches!(e, TableEvent::SelectionChanged { .. })));
    t.h.state_mut().view.clear_selection();
    t.h.run();
    assert_eq!(t.h.state().view.active_row(), None);
}

#[test]
fn hidden_columns_are_not_shown() {
    let mut t = Table::grid(3, 1);
    t.config(|c| {
        c.hidden_columns.insert(B);
    });
    assert!(!t.has("B"));
    assert!(!t.has("B1"));
    t.hover("A1");
    t.ctrl(Key::A);
    assert_eq!(t.copy().as_deref(), Some("A1\tC1"));
}

#[test]
fn column_order_comes_from_config() {
    let mut t = Table::grid(3, 1);
    t.config(|c| c.column_order = vec![ColumnUid(2), ColumnUid(0)]);
    assert_eq!(t.header_order(), ["C", "A", "B"]);
}

#[test]
fn dragging_a_column_saves_the_order() {
    let mut t = Table::grid(3, 1);
    t.config(|c| {
        c.hidden_columns.insert(ColumnUid(2));
        c.column_order = vec![ColumnUid(9)];
    });
    t.drag(t.center("A"), t.center("B"));
    let config = &t.h.state().config;
    // The hidden column keeps its slot, the unknown one stays listed.
    assert_eq!(
        config.column_order,
        [ColumnUid(1), ColumnUid(0), ColumnUid(2), ColumnUid(9)]
    );
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));
}

#[test]
fn resizing_saves_the_width_and_saved_widths_apply() {
    let mut t = Table::grid(2, 1);
    let edge = t.column_right_edge(0);
    let y = t.center("A1").y;
    t.drag(egui::pos2(edge, y), egui::pos2(edge + 60.0, y));
    let saved =
        *t.h.state()
            .config
            .column_widths
            .get(&A)
            .expect("width saved");
    assert!(saved > 60.0);
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));

    // A fresh view restores it.
    let mut u = Table::grid(2, 1);
    u.config(|c| {
        c.column_widths.insert(A, saved);
    });
    let b_x = u.node("B").rect().min.x;
    assert!((b_x - t.node("B").rect().min.x).abs() < 2.0);

    // Double-click returns it to auto and forgets the saved width.
    let edge = t.column_right_edge(0);
    t.double_click_at(egui::pos2(edge, y));
    assert!(t.h.state().config.column_widths.is_empty());
}

#[test]
fn config_round_trips_through_serde() {
    let mut c = TableViewConfig {
        sort: vec![SortKey::descending(B)],
        column_order: vec![B, A],
        ..Default::default()
    };
    c.filter.quick = "x".into();
    c.filter.columns.insert(A, ColumnFilter::Empty(false));
    c.hidden_columns.insert(B);
    c.pinned_columns = vec![B];
    c.column_widths.insert(A, 120.0);
    let json = serde_json::to_string(&c).unwrap();
    let back: TableViewConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
    // Older saved configs without the new fields still load.
    let old: TableViewConfig = serde_json::from_str(r#"{"minimum_row_height":15.0}"#).unwrap();
    assert!(old.sort.is_empty());
}
