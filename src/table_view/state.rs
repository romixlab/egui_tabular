use super::TableEvent;
use super::layout::{ColumnWidths, RowLayout};
use crate::commands::TableCommand;
use egui::Rect;
use rvariant::Variant;
use std::collections::HashMap;
use tabular_core::{CellCoord, ColumnInfo, ColumnUid, Revision};

pub(super) struct State {
    pub(super) rows: RowLayout,
    pub(super) column_widths: ColumnWidths,
    /// Measured on the previous frame.
    pub(super) header_height: f32,
    /// Header and body area of the previous frame, for mouse wheel hit testing.
    pub(super) table_rect: Option<Rect>,
    pub(super) columns_ordered: Vec<ColumnUid>,
    pub(super) columns: HashMap<ColumnUid, ColumnInfo>,
    pub(super) selected_range: Option<SelectedRange>,
    /// Value of the cell being edited, owned by the view until committed or cancelled.
    pub(super) edit: Option<EditBuffer>,
    /// Model revision the columns and rows were last synced with.
    pub(super) revision: Option<Revision>,
    /// Changes to apply to the model, outside of rendering.
    pub(super) commands: Vec<TableCommand>,
    pub(super) events: Vec<TableEvent>,

    pub(crate) pasting_block_width: usize,
    pub(crate) pasting_block_with_holes: bool,
    pub(crate) about_to_paste_rows: Vec<Vec<String>>,
    pub(crate) create_rows_on_paste: bool,
    pub(crate) fill_with_same_on_paste: bool,
    pub(crate) create_cols_on_paste: bool,
    pub(crate) create_adhoc_cols_on_paste: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            rows: RowLayout::default(),
            column_widths: ColumnWidths::default(),
            header_height: 20.0,
            table_rect: None,
            columns_ordered: Vec::new(),
            columns: Default::default(),
            selected_range: None,
            edit: None,
            revision: None,
            commands: vec![],
            events: vec![],
            pasting_block_width: 0,
            pasting_block_with_holes: false,
            about_to_paste_rows: vec![],
            create_rows_on_paste: false,
            fill_with_same_on_paste: false,
            create_cols_on_paste: false,
            create_adhoc_cols_on_paste: false,
        }
    }
}

pub(super) struct EditBuffer {
    pub(super) coord: CellCoord,
    pub(super) value: Variant,
    /// The editor gets focus on its first frame.
    pub(super) first_frame: bool,
}

/// All indices are from 0 to row or column count currently in view
#[derive(Copy, Clone, Eq)]
pub(crate) struct SelectedRange {
    row_start: usize,
    row_end: usize,
    col_start: usize,
    col_end: usize,
    editing: Option<CellCoord>,
}

impl PartialEq for SelectedRange {
    fn eq(&self, other: &Self) -> bool {
        self.row_start == other.row_start
            && self.row_end == other.row_end
            && self.col_start == other.col_start
            && self.col_end == other.col_end
    }
}

impl SelectedRange {
    pub fn single_cell(row_idx: usize, col_idx: usize) -> Self {
        SelectedRange {
            row_start: row_idx,
            row_end: row_idx,
            col_start: col_idx,
            col_end: col_idx,
            editing: None,
        }
    }

    pub fn single_row(row_idx: usize, col_count: usize) -> Self {
        SelectedRange {
            row_start: row_idx,
            row_end: row_idx,
            col_start: 0,
            col_end: col_count.saturating_sub(1),
            editing: None,
        }
    }

    pub fn rect(width: usize, height: usize) -> Self {
        SelectedRange {
            row_start: 0,
            row_end: height.saturating_sub(1),
            col_start: 0,
            col_end: width.saturating_sub(1),
            editing: None,
        }
    }

    pub fn row_start(&self) -> usize {
        self.row_start
    }

    pub fn row_end(&self) -> usize {
        self.row_end
    }

    pub fn col_start(&self) -> usize {
        self.col_start
    }

    pub fn col_end(&self) -> usize {
        self.col_end
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub fn editing(&self) -> Option<CellCoord> {
        self.editing
    }

    pub fn set_editing(&mut self, editing: Option<CellCoord>) {
        if self.is_single_cell() {
            self.editing = editing;
        }
    }

    pub fn is_single_cell(&self) -> bool {
        self.row_start == self.row_end && self.col_start == self.col_end
    }

    pub fn swap_col(&mut self, col1_idx: usize, col2_idx: usize) {
        if !self.is_single_cell() {
            return;
        }
        if self.col_start == col1_idx {
            self.col_start = col2_idx;
            self.col_end = col2_idx;
        }
        if self.col_start == col2_idx {
            self.col_start = col1_idx;
            self.col_end = col1_idx;
        }
    }

    pub fn stretch_to(&mut self, row_idx: usize, col_idx: usize) {
        if row_idx > self.row_end {
            self.row_end = row_idx;
        }
        if row_idx < self.row_start {
            self.row_start = row_idx;
        }
        if col_idx > self.col_end {
            self.col_end = col_idx;
        }
        if col_idx < self.col_start {
            self.col_start = col_idx;
        }
        if !self.is_single_cell() {
            self.editing = None;
        }
    }

    pub fn stretch_multi_row(&mut self, row_idx: usize, col_count: usize) {
        if row_idx < self.row_start {
            self.row_start = row_idx;
        } else if row_idx > self.row_end {
            self.row_end = row_idx;
        }
        self.col_start = 0;
        self.col_end = col_count.saturating_sub(1);
    }

    pub fn contains(&self, row_idx: usize, col_idx: usize) -> bool {
        row_idx >= self.row_start
            && row_idx <= self.row_end
            && col_idx >= self.col_start
            && col_idx <= self.col_end
    }

    pub fn contains_col(&self, col_idx: usize) -> bool {
        col_idx >= self.col_start && col_idx <= self.col_end
    }

    pub fn contains_row(&self, row_idx: usize) -> bool {
        row_idx >= self.row_start && row_idx <= self.row_end
    }

    pub fn move_left(&mut self, expand: bool) {
        if self.col_start > 0 {
            self.col_start = self.col_start - 1;
            if !expand {
                self.col_end = self.col_end - 1;
            }
            self.set_editing(None);
        }
    }

    pub fn move_right(&mut self, expand: bool, col_count: usize) {
        if self.col_end < col_count - 1 {
            self.col_end += 1;
            if !expand {
                self.col_start += 1;
            }
            self.set_editing(None);
        }
    }

    pub fn move_up(&mut self, expand: bool) {
        if self.row_start > 0 {
            self.row_start = self.row_start - 1;
            if !expand {
                self.row_end = self.row_end - 1;
            }
            self.set_editing(None);
        }
    }

    pub fn move_down(&mut self, expand: bool, row_count: usize) {
        if self.row_end < row_count - 1 {
            self.row_end += 1;
            if !expand {
                self.row_start = self.row_start + 1;
            }
            self.set_editing(None);
        }
    }

    pub fn width(&self) -> usize {
        self.col_end - self.col_start + 1
    }

    pub fn height(&self) -> usize {
        self.row_end - self.row_start + 1
    }
}

impl State {
    pub fn is_editing(&self) -> bool {
        self.selected_range
            .map(|r| r.editing.is_some())
            .unwrap_or(false)
    }

    /// Queue the edited value of `coord` to be written to the model and leave edit mode.
    pub(super) fn commit_edit(&mut self, coord: CellCoord) {
        if let Some(edit) = self.edit.take_if(|e| e.coord == coord) {
            self.commands.push(TableCommand::Set {
                coord,
                value: edit.value,
            });
        }
        if let Some(r) = &mut self.selected_range
            && r.editing() == Some(coord)
        {
            r.set_editing(None);
        }
    }

    /// Drop the edit buffer if its cell is no longer being edited.
    pub(super) fn end_stale_edit(&mut self) {
        let editing = self.selected_range.and_then(|r| r.editing());
        if let Some(edit) = self.edit.take_if(|e| Some(e.coord) != editing) {
            self.events.push(TableEvent::EditCancelled(edit.coord));
        }
    }
}
