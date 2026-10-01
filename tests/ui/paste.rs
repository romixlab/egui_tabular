use crate::fixture::Table;

#[test]
fn paste_into_matching_selection() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.shift_click("B1");
    t.paste("x\ty");
    assert_eq!(t.value(0, 0).as_deref(), Some("x"));
    assert_eq!(t.value(0, 1).as_deref(), Some("y"));
}

#[test]
fn paste_size_mismatch_opens_dialog() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.paste("x\ty");
    assert!(t.has("Paste"), "paste dialog is open");
    assert_eq!(t.value(0, 0).as_deref(), Some("A1"));
}

#[test]
#[ignore = "PASTE-2: trailing newline from spreadsheets adds a bogus row"]
fn paste_with_trailing_newline() {
    let mut t = Table::grid(2, 2);
    t.click("A1");
    t.shift_click("B1");
    t.paste("x\ty\n");
    assert!(!t.has("Paste"), "no dialog");
    assert_eq!(t.value(0, 0).as_deref(), Some("x"));
}
