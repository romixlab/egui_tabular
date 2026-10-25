//! Row spans from the model (VIEW-16): one cell over several rows of a column, read-only, one
//! unit for drawing, selection and keyboard moves.

use crate::fixture::{Table, col_name};
use egui::{Key, Modifiers, Pos2};
use egui_tabular::rvariant::Variant;
use egui_tabular::{
    Capabilities, CellCoord, ColumnInfo, ColumnUid, ModelError, Revision, RowSpan, RowUid, SortKey,
    TableEvent, TableModel, VariantCellUi,
};
use std::borrow::Cow;

/// A schedule: column A names the row ("a0", "a1", ...), column B holds bookings that span rows
/// and "b<row>" in the free rows.
struct Schedule {
    rows: usize,
    columns: Vec<ColumnInfo>,
    /// First row, row count, text.
    bookings: Vec<(usize, usize, &'static str)>,
    /// Whether rows covered by a booking still answer "b<row>", as a model may.
    covered_text: bool,
}

impl Schedule {
    fn new() -> Self {
        Schedule {
            rows: 60,
            columns: (0..2).map(|c| ColumnInfo::new(col_name(c), None)).collect(),
            bookings: vec![(2, 4, "Job1"), (10, 30, "Long")],
            covered_text: false,
        }
    }

    fn booking(&self, row: usize) -> Option<(usize, usize, &'static str)> {
        self.bookings
            .iter()
            .copied()
            .find(|&(first, count, _)| (first..first + count).contains(&row))
    }
}

impl TableModel for Schedule {
    fn revision(&self) -> Revision {
        Revision::default()
    }

    /// Editable, so that the tests can tell that spans are not.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            edit_cells: true,
            ..Capabilities::NONE
        }
    }

    fn columns(&self) -> impl Iterator<Item = ColumnUid> {
        (0..self.columns.len() as u32).map(ColumnUid)
    }

    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo> {
        self.columns.get(col.0 as usize)
    }

    fn rows(&self) -> impl Iterator<Item = RowUid> {
        (0..self.rows as u64).map(RowUid)
    }

    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
        let row = coord.row_uid.0 as usize;
        let text = match (coord.col_uid.0, self.booking(row)) {
            (0, _) => format!("a{row}"),
            (_, Some((first, _, text))) if first == row => text.to_owned(),
            (_, Some(_)) if !self.covered_text => return None,
            _ => format!("b{row}"),
        };
        Some(Cow::Owned(Variant::Str(text)))
    }

    fn set(&mut self, _coord: CellCoord, _value: Variant) -> Result<(), ModelError> {
        Ok(())
    }

    fn row_span(&self, coord: CellCoord) -> Option<RowSpan> {
        if coord.col_uid != B {
            return None;
        }
        let (first, rows, _) = self.booking(coord.row_uid.0 as usize)?;
        Some(RowSpan {
            first: RowUid(first as u64),
            rows,
        })
    }
}

const A: ColumnUid = ColumnUid(0);
const B: ColumnUid = ColumnUid(1);
const JOB1: &str = "Job1\n\n\n";

fn table() -> Table<Schedule, VariantCellUi> {
    let mut t = Table::custom(Schedule::new(), VariantCellUi);
    t.h.run();
    t
}

/// A point in column B at the height of row `row`, which may be covered by a span.
fn in_b(t: &Table<Schedule, VariantCellUi>, row: usize) -> Pos2 {
    Pos2::new(t.center("B").x, t.center(&format!("a{row}")).y)
}

#[test]
fn span_shows_its_text_once() {
    let mut model = Schedule::new();
    model.covered_text = true;
    let mut t = Table::custom(model, VariantCellUi);
    t.h.run();
    assert!(t.has("Job1"));
    assert!(t.has("b1") && t.has("b6"));
    assert!(!t.has("b3"), "covered cells draw nothing of their own");
}

#[test]
fn click_selects_the_whole_span() {
    let mut t = table();
    t.click("Job1");
    assert_eq!(t.copy().as_deref(), Some(JOB1));
    assert!(t.saw(|e| matches!(
        e,
        TableEvent::RowClicked {
            row: RowUid(2),
            col: Some(B)
        }
    )));
}

#[test]
fn click_on_a_covered_row_hits_the_span() {
    let mut t = table();
    t.click_at(in_b(&t, 4));
    assert_eq!(t.copy().as_deref(), Some(JOB1));
}

#[test]
fn span_starting_above_the_viewport_is_drawn_and_clickable() {
    let mut t = table();
    t.scroll_to_row(38);
    assert!(!t.has("a10"), "the span's first row is scrolled away");
    assert!(t.has("a30"));
    t.click_at(in_b(&t, 30));
    let copied = t.copy().unwrap();
    assert_eq!(copied, format!("Long{}", "\n".repeat(29)));
    assert!(t.saw(|e| matches!(
        e,
        TableEvent::RowClicked {
            row: RowUid(10),
            col: Some(B)
        }
    )));
}

#[test]
fn arrows_skip_over_a_span() {
    let mut t = table();
    t.click("b1");
    t.press(Key::ArrowDown);
    assert_eq!(t.copy().as_deref(), Some(JOB1));
    t.press(Key::ArrowDown);
    assert_eq!(t.copy().as_deref(), Some("b6"));
    t.press(Key::ArrowUp);
    assert_eq!(t.copy().as_deref(), Some(JOB1));
    t.press(Key::ArrowUp);
    assert_eq!(t.copy().as_deref(), Some("b1"));
}

#[test]
fn left_and_right_enter_and_leave_a_span() {
    let mut t = table();
    t.click("a4");
    t.press(Key::ArrowRight);
    assert_eq!(t.copy().as_deref(), Some(JOB1));
    t.press(Key::ArrowLeft);
    assert_eq!(
        t.copy().as_deref(),
        Some("a2"),
        "back to the span's first row"
    );
}

#[test]
fn shift_arrow_takes_the_span_whole() {
    let mut t = table();
    t.click("b1");
    t.press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    assert_eq!(t.copy().as_deref(), Some("b1\nJob1\n\n\n"));
}

#[test]
fn span_is_not_edited() {
    let mut t = table();
    t.start_edit("b1");
    t.press(Key::Escape);
    t.click("Job1");
    t.click("Job1");
    t.press(Key::E);
    assert!(t.editor().is_none());
}

#[test]
fn spans_follow_heterogeneous_row_heights() {
    let mut t = table();
    t.config(|c| c.use_heterogeneous_row_heights = true);
    t.click_at(in_b(&t, 5));
    assert_eq!(t.copy().as_deref(), Some(JOB1));
    t.press(Key::ArrowDown);
    assert_eq!(t.copy().as_deref(), Some("b6"));
}

#[test]
fn sorted_view_draws_every_cell_alone() {
    let mut t = table();
    t.config(|c| c.sort = vec![SortKey::ascending(A)]);
    t.click("Job1");
    assert_eq!(t.copy().as_deref(), Some("Job1"));
}
