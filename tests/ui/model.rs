//! The model contract (DESIGN-8): custom models and cell UIs, the row builder, revisions,
//! events and capabilities.

use crate::fixture::{Table, col_name};
use egui::{Key, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    Capabilities, CellCoord, CellUi, ColumnDef, ColumnInfo, ColumnUid, Revision, RowCells, RowUid,
    TableEvent, TableModel, TableView, TableViewConfig, TableViewOptions, VariantCellUi,
    VariantTable,
};
use std::cell::Cell;

/// Bus frames kept in the app's own type. No `Variant` values: everything is drawn by
/// [`FramesUi`]. Counts how often a frame is looked up.
struct Frames {
    frames: Vec<(u32, u8)>,
    columns: Vec<ColumnInfo>,
    lookups: Cell<usize>,
}

impl Frames {
    fn new(frames: Vec<(u32, u8)>) -> Self {
        Frames {
            frames,
            // Named "A", "B", "C" so that the fixture's column helpers work.
            columns: (0..3).map(|c| ColumnInfo::new(col_name(c), None)).collect(),
            lookups: Cell::new(0),
        }
    }

    fn frame(&self, row: RowUid) -> Option<(u32, u8)> {
        self.lookups.set(self.lookups.get() + 1);
        self.frames.get(row.0 as usize).copied()
    }
}

impl TableModel for Frames {
    fn revision(&self) -> Revision {
        Revision::default()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn columns(&self) -> impl Iterator<Item = ColumnUid> {
        (0..self.columns.len() as u32).map(ColumnUid)
    }

    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo> {
        self.columns.get(col.0 as usize)
    }

    fn rows(&self) -> impl Iterator<Item = RowUid> {
        (0..self.frames.len() as u64).map(RowUid)
    }
}

fn frame_text(col: ColumnUid, (id, dlc): (u32, u8)) -> String {
    match col.0 {
        0 => format!("id{id}"),
        1 => format!("dlc{dlc}"),
        _ => format!("x{id:03X}"),
    }
}

struct FramesUi;

impl CellUi<Frames> for FramesUi {
    fn show_row(&mut self, model: &mut Frames, row: RowUid, cells: &mut RowCells<'_>) {
        let Some(frame) = model.frame(row) else {
            return;
        };
        // Filled in reverse: the view places cells by column, not by call order.
        for col in [2, 1, 0].map(ColumnUid) {
            cells.cell(col, |ui| ui.label(frame_text(col, frame)));
        }
    }

    fn text(&self, model: &Frames, coord: CellCoord) -> Option<String> {
        let frame = model.frames.get(coord.row_uid.0 as usize)?;
        Some(frame_text(coord.col_uid, *frame))
    }
}

fn frames() -> Table<Frames, FramesUi> {
    Table::custom(Frames::new(vec![(1, 8), (2, 4), (3, 2)]), FramesUi)
}

#[test]
fn row_builder_looks_up_each_row_once_per_frame() {
    let mut t = frames();
    t.h.state().table.lookups.set(0);
    t.h.step();
    assert_eq!(t.h.state().table.lookups.get(), 3);
}

#[test]
fn row_builder_places_cells_by_column_after_a_move() {
    let mut t = frames();
    t.drag_column(0, 2);
    assert_eq!(t.header_order(), ["C", "B", "A"]);
    for (cell, header) in [("id1", "A"), ("dlc8", "B"), ("x001", "C")] {
        let (cell_x, header_x) = (t.node(cell).rect().min.x, t.node(header).rect().min.x);
        assert!(
            (cell_x - header_x).abs() < 1.0,
            "{cell} is at x={cell_x}, its column {header} at x={header_x}"
        );
    }
}

#[test]
fn copy_uses_cell_ui_text_when_the_model_has_no_values() {
    let mut t = frames();
    t.click("dlc4");
    assert_eq!(t.copy().as_deref(), Some("dlc4"));
    t.ctrl(Key::A);
    assert_eq!(
        t.copy().as_deref(),
        Some("id1\tdlc8\tx001\nid2\tdlc4\tx002\nid3\tdlc2\tx003")
    );
}

#[test]
fn model_without_values_is_not_editable() {
    let mut t = frames();
    t.click("id1");
    t.click("id1");
    t.press(Key::E);
    assert!(t.editor().is_none());
}

/// Two views over one model, stacked.
struct TwoViews {
    table: VariantTable,
    views: [TableView; 2],
    config: TableViewConfig,
}

#[test]
fn every_view_sees_model_changes() {
    // FLAGS-1: change notifications used to be consumed by the first view.
    let mut table = VariantTable::new([ColumnDef::new("A", VariantTy::Str)]);
    table.insert_row([(ColumnUid(0), Variant::Str("first".into()))]);
    let view = |salt: &str| {
        TableView::new(TableViewOptions {
            id_salt: egui::Id::new(salt),
            max_height: Some(150.0),
            ..Default::default()
        })
    };
    let state = TwoViews {
        table,
        views: [view("one"), view("two")],
        config: TableViewConfig::default(),
    };
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            |ui, s: &mut TwoViews| {
                for view in &mut s.views {
                    view.show(ui, &mut s.table, &mut VariantCellUi, &mut s.config);
                }
            },
            state,
        );
    h.run();
    assert_eq!(h.query_all_by_label("first").count(), 2);

    h.state_mut()
        .table
        .insert_row([(ColumnUid(0), Variant::Str("second".into()))]);
    h.run();
    assert_eq!(h.query_all_by_label("second").count(), 2);
}

#[test]
fn edits_report_committed_and_cancelled_cells() {
    let mut t = Table::grid(2, 2);
    let a1 = CellCoord::from((t.rows[0], t.cols[0]));
    t.start_edit("A1");
    t.type_replace("new");
    t.press(Key::Enter);
    assert!(t.h.state().events.contains(&TableEvent::CellCommitted(a1)));

    t.start_edit("B1");
    t.press(Key::Escape);
    let b1 = CellCoord::from((t.rows[0], t.cols[1]));
    assert!(t.h.state().events.contains(&TableEvent::EditCancelled(b1)));
}

#[test]
fn selection_change_reports_rows() {
    let mut t = Table::grid(2, 3);
    t.click("A2");
    let rows = vec![t.rows[1]];
    assert!(
        t.h.state()
            .events
            .contains(&TableEvent::SelectionChanged { rows })
    );
}

#[test]
fn paste_without_selection_tells_the_user() {
    // VIEW-10: this was only logged.
    let mut t = Table::grid(2, 2);
    t.hover("A1");
    t.paste("x");
    assert!(
        t.h.state()
            .events
            .iter()
            .any(|e| matches!(e, TableEvent::Message { .. }))
    );
}

#[test]
fn read_only_option_disables_editing_and_appending() {
    let mut t = Table::grid(2, 2);
    t.h.state_mut().view.options_mut().read_only = true;
    t.click("A1");
    t.click("A1");
    assert!(t.editor().is_none());
    t.press(Key::N);
    assert!(!t.has("2"), "no row was appended");
}

#[test]
fn bool_cell_click_selects_the_cell() {
    // EDIT-7: the viewer was a live checkbox that took the click.
    let mut table = VariantTable::new([ColumnDef::new("A", VariantTy::Bool)]);
    table.insert_row([(ColumnUid(0), Variant::Bool(true))]);
    let mut t = Table::build(
        Harness::builder().with_size(Vec2::new(600.0, 400.0)),
        table,
        VariantCellUi,
    );
    t.h.run();
    // The check mark sits at the left of the cell, below the header label.
    let pos = egui::pos2(t.node("A").rect().min.x + 4.0, t.center("0").y);
    t.click_at(pos);
    assert_eq!(t.copy().as_deref(), Some("true"));
}

#[test]
fn read_only_table_offers_no_add_row_or_create_column() {
    // VIEW-4: these buttons ignored read-only and the model's capabilities.
    let t = Table::custom(VariantTable::new([]), VariantCellUi);
    assert!(t.has("Create column"), "editable tables offer it");
    let t = Table::custom(
        VariantTable::new([ColumnDef::new("A", VariantTy::Str)]),
        VariantCellUi,
    );
    assert!(t.has("Add row"), "editable tables offer it");

    let mut empty = VariantTable::new([ColumnDef::new("A", VariantTy::Str)]);
    empty.set_read_only(true);
    let t = Table::custom(empty, VariantCellUi);
    assert!(t.has("A"));
    assert!(!t.has("Add row"));

    let mut no_columns = VariantTable::new([]);
    no_columns.set_read_only(true);
    let t = Table::custom(no_columns, VariantCellUi);
    assert!(!t.has("Create column"));
}
