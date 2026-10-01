use crate::fixture::{Table, col_name};

#[test]
fn drag_header_reorders_columns() {
    let mut t = Table::grid(2, 2);
    t.drag_column(0, 1);
    assert_eq!(t.header_order(), ["B", "A"]);
    // Copy follows the visual order.
    t.hover("A1");
    t.ctrl(egui::Key::A);
    assert_eq!(t.copy().as_deref(), Some("B1\tA1\nB2\tA2"));
}

#[test]
#[ignore = "DND-1: drop swaps columns instead of moving"]
fn drag_header_moves_column() {
    let mut t = Table::grid(4, 1);
    t.drag_column(0, 3);
    assert_eq!(t.header_order(), ["B", "C", "D", "A"]);
}

#[test]
#[ignore = "DND-3: SelectedRange::swap_col is a no-op when the selection is in the drop target"]
fn selection_follows_drop_target_column() {
    let mut t = Table::grid(3, 1);
    t.click("B1");
    t.drag_column(2, 1); // drop C on B
    assert_eq!(t.header_order(), ["A", "C", "B"]);
    t.hover("A1");
    assert_eq!(t.copy().as_deref(), Some("B1"));
}

#[test]
fn selection_follows_dragged_column() {
    let mut t = Table::grid(3, 1);
    t.click("B1");
    t.drag_column(1, 2); // drop B on C
    assert_eq!(t.header_order(), ["A", "C", "B"]);
    t.hover("A1");
    assert_eq!(t.copy().as_deref(), Some("B1"));
}

#[test]
fn drag_edge_resizes_column_and_double_click_resets() {
    let mut t = Table::grid(2, 1);
    let b_before = t.node(&col_name(1)).rect().min.x;
    let edge = t.column_right_edge(0);
    let y = t.center("A1").y;
    t.drag(egui::pos2(edge, y), egui::pos2(edge + 60.0, y));
    let b_after = t.node(&col_name(1)).rect().min.x;
    assert!(
        (b_after - b_before - 60.0).abs() < 2.0,
        "{b_before} -> {b_after}"
    );

    let edge = t.column_right_edge(0);
    t.double_click_at(egui::pos2(edge, y));
    let b_reset = t.node(&col_name(1)).rect().min.x;
    assert!((b_reset - b_before).abs() < 2.0, "{b_before} -> {b_reset}");
}
