use crate::fixture::{Table, col_name};
use egui_kittest::kittest::Queryable;
use egui_tabular::{ColumnUid, TableEvent};

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
fn drag_header_moves_column() {
    let mut t = Table::grid(4, 1);
    t.drag_column(0, 3);
    assert_eq!(t.header_order(), ["B", "C", "D", "A"]);
}

#[test]
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

#[test]
fn drag_header_moves_column_backwards() {
    let mut t = Table::grid(4, 1);
    t.drag_column(3, 0);
    assert_eq!(t.header_order(), ["D", "A", "B", "C"]);
}

#[test]
fn hide_in_header_menu_hides_the_column() {
    let mut t = Table::grid(3, 1);
    t.right_click("B");
    t.click("Hide");
    assert_eq!(t.header_order(), ["A", "C"]);
    assert!(!t.has("B1"));
    assert!(t.h.state().config.hidden_columns.contains(&ColumnUid(1)));
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));
}

#[test]
fn column_chooser_shows_hidden_columns_again() {
    let mut t = Table::grid(3, 1);
    t.config(|c| {
        c.hidden_columns.insert(ColumnUid(1));
    });
    assert_eq!(t.header_order(), ["A", "C"]);
    t.right_click_tool_header();
    t.click("Columns ⏵");
    // Lists every column in the user's order, hidden ones unchecked.
    t.click_checkbox("B");
    assert_eq!(t.header_order(), ["A", "B", "C"]);
    assert!(t.h.state().config.hidden_columns.is_empty());
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));
    // Unchecking hides again.
    t.click_checkbox("A");
    assert_eq!(t.header_order(), ["B", "C"]);
}

#[test]
fn pin_in_header_menu_pins_the_column() {
    let mut t = Table::grid(3, 1);
    t.right_click("B");
    t.click("Pin");
    assert_eq!(t.h.state().config.pinned_columns, vec![ColumnUid(1)]);
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));
    // Unchecking it again un-pins it.
    t.right_click("B");
    t.click("Pin");
    assert!(t.h.state().config.pinned_columns.is_empty());
}

#[test]
fn pinned_column_stays_put_while_the_rest_scrolls_sideways() {
    let mut t = Table::grid(6, 1);
    t.config(|c| {
        for col in 0..6 {
            c.column_widths.insert(ColumnUid(col), 150.0);
        }
        c.pinned_columns = vec![ColumnUid(0)];
    });
    let pinned_x = t.node("A").rect().min.x;
    let scrollable_x_before = t.node("B").rect().min.x;

    t.h.hover_at(t.center("B1"));
    t.h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(-300.0, 0.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    t.h.run();

    assert_eq!(
        t.node("A").rect().min.x,
        pinned_x,
        "the pinned column must not move while the rest scrolls"
    );
    assert_ne!(
        t.node("B").rect().min.x,
        scrollable_x_before,
        "scrolling sideways didn't move the scrollable columns"
    );
}

#[test]
fn arrow_right_reveals_a_scrolled_out_column_and_skips_pinned_ones() {
    let mut t = Table::grid(6, 1);
    t.config(|c| {
        for col in 0..6 {
            c.column_widths.insert(ColumnUid(col), 150.0);
        }
        c.pinned_columns = vec![ColumnUid(0)];
    });
    let pinned_x = t.node("A").rect().min.x;
    let f_x_before = t.node("F").rect().min.x;

    t.click("A1");
    for _ in 0..5 {
        t.press(egui::Key::ArrowRight);
    }

    assert_ne!(
        t.node("F").rect().min.x,
        f_x_before,
        "arrow-right should have scrolled column F into view"
    );
    assert_eq!(
        t.node("A").rect().min.x,
        pinned_x,
        "the pinned column never needs to be revealed, so it never moves"
    );
}

#[test]
fn header_types_can_be_hidden() {
    let mut t = crate::fixture::Table::grid(2, 1);
    assert!(t.h.query_all_by_label("String").count() > 0);
    t.h.state_mut().view.options_mut().header_types = false;
    t.h.run();
    assert_eq!(t.h.query_all_by_label("String").count(), 0);
}
