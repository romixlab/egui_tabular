use crate::fixture::Table;
use egui::Key;
use egui_tabular::TableEvent;

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

/// egui-winit turns Ctrl+C into `Event::Copy` and sends no key press for it: the copy must come
/// from the event alone (SEL-5; before, only a `Key::C` press copied, which a real window never sends).
#[test]
fn the_copy_event_copies_the_selection() {
    let mut t = Table::grid(2, 2);
    t.click("B2");
    t.press_copy();
    assert_eq!(t.h.state().copied.as_deref(), Some("B2"));
}

/// Cmd+C on macOS arrives as the same `Event::Copy` (SEL-5).
#[test]
fn cmd_c_copies_on_mac() {
    let builder = egui_kittest::Harness::builder().with_os(egui::os::OperatingSystem::Mac);
    let mut t = Table::grid_with(builder, 1, 1);
    t.click("A1");
    t.press_copy();
    assert_eq!(t.h.state().copied.as_deref(), Some("A1"));
}

/// Text in cells is selected with a drag and copied with Ctrl+C (`selectable_text`).
#[test]
fn selectable_text_drag_copies_part_of_a_cell() {
    let mut t = Table::grid(2, 2);
    t.options(|o| o.selectable_text = true);
    let r = t.node("B2").rect();
    t.drag(r.left_center() + egui::vec2(1.0, 0.0), r.center());
    t.press_copy();
    assert_eq!(t.h.state().copied.as_deref(), Some("B"));
}

/// With selectable text a click on the text still selects the cell and reports the row.
#[test]
fn selectable_text_click_still_selects_the_cell() {
    let mut t = Table::grid(2, 2);
    t.options(|o| o.selectable_text = true);
    t.click("B2");
    assert_eq!(t.copy().as_deref(), Some("B2"));
    assert!(t.saw(|e| matches!(e, TableEvent::RowClicked { col: Some(_), .. })));
    t.double_click_at(t.center("A1"));
    assert!(t.saw(|e| matches!(e, TableEvent::RowDoubleClicked { col: Some(_), .. })));
}

/// Without the option a drag over the text selects no text (the default, as before).
#[test]
fn text_is_not_selectable_by_default() {
    let mut t = Table::grid(2, 2);
    let r = t.node("B2").rect();
    t.drag(r.left_center() + egui::vec2(1.0, 0.0), r.center());
    t.press_copy();
    assert_eq!(t.h.state().copied, None);
}
