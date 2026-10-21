//! Row hover and the selection outline.

use crate::fixture::Table;

/// Pixel snapshot of a selected cell in the middle of the table while the pointer is on the row
/// below, whose fill used to cover the shared bottom edge: all four edges of the outline show.
/// `UPDATE_SNAPSHOTS=1 cargo test --test ui` regenerates it.
#[test]
fn selection_outline_snapshot() {
    let mut t = Table::grid_with(
        egui_kittest::Harness::builder().with_size(egui::Vec2::new(260.0, 160.0)),
        3,
        5,
    );
    t.click("B3");
    t.hover("C4");
    t.h.snapshot("selection_outline");
}
