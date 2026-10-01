use crate::fixture::Table;
use egui::{Key, Modifiers};

#[test]
fn click_selects_cell() {
    let mut t = Table::grid(3, 3);
    t.click("B2");
    assert_eq!(t.copy().as_deref(), Some("B2"));
}

#[test]
fn click_on_empty_part_of_cell_selects_it() {
    let mut t = Table::grid(3, 3);
    // Just right of the text, still inside the cell.
    let pos = t.node("B2").rect().right_center() + egui::vec2(2.0, 0.0);
    t.click_at(pos);
    assert_eq!(t.copy().as_deref(), Some("B2"));
}

#[test]
fn shift_click_extends_selection() {
    let mut t = Table::grid(3, 3);
    t.click("A1");
    t.shift_click("B2");
    assert_eq!(t.copy().as_deref(), Some("A1\tB1\nA2\tB2"));
}

#[test]
fn ctrl_a_selects_all() {
    let mut t = Table::grid(2, 2);
    t.hover("A1");
    t.ctrl(Key::A);
    assert_eq!(t.copy().as_deref(), Some("A1\tB1\nA2\tB2"));
}

#[test]
fn escape_clears_selection() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.press(Key::Escape);
    assert_eq!(t.copy(), None);
}

#[test]
fn row_number_click_selects_row() {
    let mut t = Table::grid(2, 3);
    t.click("1"); // tool column, row index 1
    let copied = t.copy().expect("row selection copies something");
    assert!(copied.starts_with("A2\tB2"), "{copied:?}");
}

#[test]
fn row_selection_copies_exactly_the_row() {
    let mut t = Table::grid(2, 3);
    t.click("1");
    assert_eq!(t.copy().as_deref(), Some("A2\tB2"));
}

#[test]
#[ignore = "SEL-3: shift-extend has no anchor and can't shrink"]
fn shift_click_can_shrink_selection() {
    let mut t = Table::grid(3, 3);
    t.click("A1");
    t.shift_click("C3");
    t.shift_click("B2");
    assert_eq!(t.copy().as_deref(), Some("A1\tB1\nA2\tB2"));
}

#[test]
#[ignore = "SEL-5: copy checks modifiers.ctrl, so Cmd+C on macOS does nothing"]
fn cmd_c_copies_on_mac() {
    let builder = egui_kittest::Harness::builder().with_os(egui::os::OperatingSystem::Mac);
    let mut t = Table::grid_with(builder, 1, 1);
    t.click("A1");
    t.press_modifiers(Modifiers::MAC_CMD | Modifiers::COMMAND, Key::C);
    assert_eq!(t.h.state().copied.as_deref(), Some("A1"));
}
