use crate::fixture::Table;
use egui::{Key, Modifiers};
use tabular_core::backend::TableBackend as _;

#[test]
fn arrows_move_selection() {
    let mut t = Table::grid(3, 3);
    t.click("A1");
    t.press(Key::ArrowDown);
    t.press(Key::ArrowRight);
    assert_eq!(t.copy().as_deref(), Some("B2"));
    t.press(Key::ArrowUp);
    t.press(Key::ArrowLeft);
    assert_eq!(t.copy().as_deref(), Some("A1"));
}

#[test]
fn shift_arrows_extend_selection() {
    let mut t = Table::grid(3, 3);
    t.click("A1");
    t.press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    t.press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    assert_eq!(t.copy().as_deref(), Some("A1\tB1\nA2\tB2"));
}

#[test]
fn selection_stops_at_table_edge() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.press(Key::ArrowUp);
    t.press(Key::ArrowLeft);
    assert_eq!(t.copy().as_deref(), Some("A1"));
    t.click("B2");
    t.press(Key::ArrowDown);
    t.press(Key::ArrowRight);
    assert_eq!(t.copy().as_deref(), Some("B2"));
}

#[test]
fn n_appends_row() {
    let mut t = Table::grid(2, 2);
    t.hover("A1");
    t.press(Key::N);
    assert_eq!(t.h.state().backend.row_count(), 3);
    // The new row is shown: its row number appears in the tool column.
    assert!(t.has("2"));
}

#[test]
#[ignore = "SEL-4: keyboard input depends on the pointer being over the table, not on focus"]
fn keyboard_works_without_pointer_over_table() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.pointer_away();
    t.press(Key::ArrowDown);
    t.hover("A1");
    assert_eq!(t.copy().as_deref(), Some("A2"));
}

#[test]
#[ignore = "SEL-2: arrow keys panic (usize underflow) when all rows are removed"]
fn arrows_with_zero_rows_dont_panic() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.h.state_mut().backend.clear();
    t.h.run();
    t.h.hover_at(t.h.ctx.content_rect().center());
    t.press(Key::ArrowDown);
    t.press(Key::ArrowRight);
}
