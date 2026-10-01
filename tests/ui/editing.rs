use crate::fixture::Table;
use egui::Key;

#[test]
fn click_selected_cell_opens_editor_with_value() {
    let mut t = Table::grid(2, 2);
    t.click("B1");
    assert!(t.editor().is_none(), "first click only selects");
    t.click("B1");
    assert_eq!(t.editor_text(), "B1");
}

#[test]
fn enter_commits() {
    let mut t = Table::grid(2, 2);
    t.start_edit("B1");
    t.type_replace("new");
    t.press(Key::Enter);
    assert!(t.editor().is_none());
    assert_eq!(t.value(0, 1).as_deref(), Some("new"));
    assert!(t.has("new"), "committed value is shown");
}

#[test]
fn escape_cancels() {
    let mut t = Table::grid(2, 2);
    t.start_edit("B1");
    t.type_replace("new");
    t.press(Key::Escape);
    assert!(t.editor().is_none());
    assert_eq!(t.value(0, 1).as_deref(), Some("B1"));
}

#[test]
fn clicking_another_cell_commits() {
    let mut t = Table::grid(2, 2);
    t.start_edit("A1");
    t.type_replace("new");
    t.click("B2");
    assert!(t.editor().is_none());
    assert_eq!(t.value(0, 0).as_deref(), Some("new"));
    assert_eq!(t.copy().as_deref(), Some("B2"));
}

#[test]
fn e_starts_editing() {
    let mut t = Table::grid(2, 2);
    t.click("B2");
    t.press(Key::E);
    assert_eq!(t.editor_text(), "B2");
}

#[test]
fn tab_commits_and_edits_next_cell() {
    let mut t = Table::grid(3, 2);
    t.start_edit("A1");
    t.type_replace("new");
    t.press(Key::Tab);
    assert_eq!(t.value(0, 0).as_deref(), Some("new"));
    assert_eq!(t.editor_text(), "B1");
}

#[test]
#[ignore = "BACKEND-1: VariantBackend::set_read_only has no effect"]
fn read_only_table_does_not_edit() {
    let mut t = Table::grid(2, 2);
    t.h.state_mut().backend.set_read_only(true);
    t.click("A1");
    t.click("A1");
    t.press(Key::E);
    assert!(t.editor().is_none());
}

#[test]
#[ignore = "EDIT-1: tool column click while editing drops the edit"]
fn row_number_click_commits_edit() {
    let mut t = Table::grid(2, 2);
    t.start_edit("A1");
    t.type_replace("new");
    t.click("1");
    assert!(t.editor().is_none());
    assert_eq!(t.value(0, 0).as_deref(), Some("new"));
}

#[test]
#[ignore = "EDIT-2: Escape with the pointer outside the table leaves a stale edit buffer"]
fn escape_away_from_table_cancels_in_backend() {
    let mut t = Table::grid(2, 2);
    t.start_edit("A1");
    t.type_replace("stale");
    t.pointer_away();
    t.press(Key::Escape);
    assert!(t.editor().is_none());
    // Editing the cell again must start from the stored value.
    t.start_edit("A1");
    assert_eq!(t.editor_text(), "A1");
}

#[test]
#[ignore = "EDIT-4: E and Tab use backend column order, not visual order"]
fn e_edits_visual_column_after_reorder() {
    let mut t = Table::grid(2, 2);
    t.drag_column(1, 0);
    assert_eq!(t.header_order(), ["B", "A"]);
    t.click("B1"); // now the first visual column
    t.press(Key::E);
    t.type_replace("new");
    // Enter commits the coord the view renders; clicking elsewhere commits the stored one.
    t.click("A2");
    assert_eq!(t.value(0, 1).as_deref(), Some("new"));
}

#[test]
#[ignore = "EDIT-4: E and Tab use backend column order, not visual order"]
fn tab_edits_visual_column_after_reorder() {
    let mut t = Table::grid(2, 2);
    t.drag_column(1, 0);
    t.start_edit("B1");
    t.press(Key::Tab); // now editing A1
    t.type_replace("new");
    t.press(Key::Tab);
    assert_eq!(t.value(0, 0).as_deref(), Some("new"));
}

#[test]
#[ignore = "EDIT-9: no F2/Enter/type-to-edit"]
fn f2_starts_editing() {
    let mut t = Table::grid(2, 2);
    t.click("B2");
    t.press(Key::F2);
    assert_eq!(t.editor_text(), "B2");
}
