//! Scrolling the body under the header.

use crate::fixture::Table;

/// Scroll the body by `dy` points with the wheel, the pointer over `over`.
fn wheel(t: &mut Table, over: &str, dy: f32) {
    t.h.hover_at(t.center(over));
    t.h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -dy),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    t.h.run();
}

/// A row scrolled part way under the header is cut at the header's bottom edge: the header stays on
/// top, and a click on it there is the header's, not the row's (VIEW-18).
#[test]
fn a_row_scrolled_under_the_header_is_cut_off_there() {
    let mut t = Table::grid(2, 40);
    let body_top = t.node("A1").rect().top();
    wheel(&mut t, "A3", 8.0);
    let a1 = t.node("A1").rect();
    assert!(
        a1.top() < body_top,
        "the first row is part way under the header"
    );
    // on the first row's text, but above the body: the header's
    t.click_at(egui::pos2(a1.center().x, (a1.top() + body_top) / 2.0));
    assert!(
        !t.saw(|e| matches!(e, egui_tabular::TableEvent::RowClicked { .. })),
        "the row under the header took the click"
    );
    println!("{:?}", t.copy());
}

/// Pixel snapshot of the same: the header drawn whole over the body scrolled by part of a row.
/// `UPDATE_SNAPSHOTS=1 cargo test --test ui` regenerates it.
#[test]
fn header_over_a_scrolled_row_snapshot() {
    let mut t = Table::grid_with(
        egui_kittest::Harness::builder().with_size(egui::Vec2::new(200.0, 120.0)),
        2,
        20,
    );
    wheel(&mut t, "A3", 8.0);
    t.pointer_away();
    t.h.snapshot("header_over_scrolled_row");
}
