//! The sort control in column headers and the header menu (SORT-2).

use crate::fixture::{Table, col_name};
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    Capabilities, CellCoord, CellUi, ColumnDef, ColumnInfo, ColumnUid, Revision, RowUid, SortKey,
    SortPolicy, TableEvent, TableModel, VariantCellUi, VariantTable,
};
use std::borrow::Cow;

const A: ColumnUid = ColumnUid(0);
const B: ColumnUid = ColumnUid(1);

/// Column A: `a`, column B: `b`, row by row.
fn table(a: &[&str], b: &[&str]) -> Table {
    let mut table = VariantTable::new((0..2).map(|c| ColumnDef::new(col_name(c), VariantTy::Str)));
    for (x, y) in a.iter().zip(b) {
        table.insert_row([
            (A, Variant::Str(x.to_string())),
            (B, Variant::Str(y.to_string())),
        ]);
    }
    Table::custom(table, VariantCellUi)
}

fn sort(t: &Table) -> Vec<SortKey> {
    sort_of(t)
}

fn sort_of<M: TableModel, C: CellUi<M>>(t: &Table<M, C>) -> Vec<SortKey> {
    t.h.state().config.sort.clone()
}

#[test]
fn click_cycles_ascending_descending_off() {
    let mut t = table(&["b", "c", "a"], &["1", "2", "3"]);
    t.click("Sort by A");
    assert_eq!(sort(&t), [SortKey::ascending(A)]);
    assert_eq!(t.top_down(&["a", "b", "c"]), ["a", "b", "c"]);
    assert!(t.saw(|e| *e == TableEvent::SortChanged));
    assert!(t.saw(|e| *e == TableEvent::ConfigChanged));
    t.click("Sort by A");
    assert_eq!(sort(&t), [SortKey::descending(A)]);
    assert_eq!(t.top_down(&["a", "b", "c"]), ["c", "b", "a"]);
    t.click("Sort by A");
    assert!(sort(&t).is_empty());
    assert_eq!(t.top_down(&["a", "b", "c"]), ["b", "c", "a"]);
}

#[test]
fn click_on_another_column_replaces_the_sort() {
    let mut t = table(&["b", "a"], &["1", "2"]);
    t.click("Sort by A");
    t.click("Sort by B");
    assert_eq!(sort(&t), [SortKey::ascending(B)]);
}

#[test]
fn shift_click_adds_a_key() {
    let mut t = table(&["x", "y", "x"], &["2", "1", "1"]);
    t.click("Sort by A");
    t.shift_click("Sort by B");
    assert_eq!(sort(&t), [SortKey::ascending(A), SortKey::ascending(B)]);
    // x1 (row 3), x2 (row 1), y1 (row 2): row numbers are natural (0-based).
    let rows = t.h.state().view.visible_rows().to_vec();
    assert_eq!(rows, [t.rows[2], t.rows[0], t.rows[1]]);
}

#[test]
fn clicking_the_control_does_not_drag_or_select() {
    let mut t = table(&["a"], &["1"]);
    t.click("Sort by A");
    assert_eq!(t.header_order(), ["A", "B"]);
    t.hover("a");
    assert_eq!(t.copy(), None);
}

#[test]
fn header_menu_sorts_and_clears() {
    let mut t = table(&["b", "a"], &["1", "2"]);
    t.right_click("A");
    t.click("Sort descending");
    assert_eq!(sort(&t), [SortKey::descending(A)]);
    t.right_click("A");
    t.click("Clear sort");
    assert!(sort(&t).is_empty());
}

/// Two text columns; B is not sortable.
struct NoSortB {
    rows: Vec<[&'static str; 2]>,
    columns: [ColumnInfo; 2],
}

impl TableModel for NoSortB {
    fn revision(&self) -> Revision {
        Revision::default()
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }
    fn columns(&self) -> impl Iterator<Item = ColumnUid> {
        [A, B].into_iter()
    }
    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo> {
        self.columns.get(col.0 as usize)
    }
    fn rows(&self) -> impl Iterator<Item = RowUid> {
        (0..self.rows.len() as u64).map(RowUid)
    }
    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
        let text = self.rows.get(coord.row_uid.0 as usize)?[coord.col_uid.0 as usize];
        Some(Cow::Owned(Variant::Str(text.into())))
    }
}

#[test]
fn unsortable_columns_have_no_control_and_a_disabled_menu() {
    let model = NoSortB {
        rows: vec![["a", "1"]],
        columns: [
            ColumnInfo::new("A", Some(VariantTy::Str)),
            ColumnInfo::new("B", Some(VariantTy::Str)).sortable(false),
        ],
    };
    let mut t = Table::custom(model, VariantCellUi);
    assert!(t.has("Sort by A"));
    assert!(!t.has("Sort by B"));
    t.right_click("B");
    t.click("Sort ascending");
    assert!(sort_of(&t).is_empty());
}

#[test]
fn stale_order_is_refreshed_by_the_control() {
    let mut t = table(&["a", "b"], &["1", "2"]);
    t.h.state_mut().view.options_mut().row_order.policy = SortPolicy::OnDemand;
    t.click("Sort by A");
    let row = t.rows[0];
    t.h.state_mut()
        .table
        .set((row, A).into(), Variant::Str("z".into()))
        .unwrap();
    t.h.run();
    assert!(t.h.state().view.is_row_order_stale());
    t.click("Sort by A");
    // The click re-sorts instead of changing the direction.
    assert_eq!(sort(&t), [SortKey::ascending(A)]);
    assert_eq!(t.top_down(&["z", "b"]), ["b", "z"]);
}

/// Pixel snapshot of the header controls: A descending (key 1), B ascending (key 2), C unsorted.
/// `UPDATE_SNAPSHOTS=1 cargo test --test ui` regenerates it.
#[test]
fn sort_controls_snapshot() {
    let mut table = VariantTable::new((0..3).map(|c| ColumnDef::new(col_name(c), VariantTy::Str)));
    for r in 0..2 {
        table.insert_row((0..3).map(|c| (ColumnUid(c), Variant::Str(format!("v{r}{c}")))));
    }
    let builder = egui_kittest::Harness::builder().with_size(egui::Vec2::new(260.0, 90.0));
    let mut t = Table::build(builder, table, VariantCellUi);
    t.config(|c| c.sort = vec![SortKey::descending(A), SortKey::ascending(B)]);
    t.h.snapshot("sort_controls");
}
