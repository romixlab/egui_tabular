use std::collections::HashMap;
use tabular_core::backend::BackendColumn;
use tabular_core::ColumnUid;

pub(super) struct State {
    pub(super) row_heights: Vec<f32>,
    pub(super) columns_ordered: Vec<ColumnUid>,
    pub(super) columns: HashMap<ColumnUid, BackendColumn>,
    pub(super) selected_range: Option<SelectedRange>,

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
            row_heights: vec![],
            columns_ordered: Vec::new(),
            columns: Default::default(),
            selected_range: None,
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

/// All indices are from 0 to row or column count currently in view
#[derive(Copy, Clone, Eq, Debug)]
pub(crate) struct SelectedRange {
    row_start: usize,
    row_end: usize,
    col_start: usize,
    col_end: usize,
    is_editing: bool,
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
            is_editing: false,
        }
    }

    pub fn single_row(row_idx: usize, col_count: usize) -> Self {
        SelectedRange {
            row_start: row_idx,
            row_end: row_idx,
            col_start: 0,
            col_end: col_count,
            is_editing: false,
        }
    }

    pub fn rect(width: usize, height: usize) -> Self {
        SelectedRange {
            row_start: 0,
            row_end: height.saturating_sub(1),
            col_start: 0,
            col_end: width.saturating_sub(1),
            is_editing: false,
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
        self.is_editing
    }

    pub fn set_editing(&mut self, is_editing: bool) {
        if is_editing {
            self.is_editing = self.is_single_cell();
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
            self.is_editing = false;
        }
    }

    pub fn stretch_multi_row(&mut self, row_idx: usize, col_count: usize) {
        if row_idx < self.row_start {
            self.row_start = row_idx;
        } else if row_idx > self.row_end {
            self.row_end = row_idx;
        }
        self.col_start = 0;
        self.col_end = col_count;
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
        }
    }

    pub fn move_right(&mut self, expand: bool, col_count: usize) {
        if self.col_end < col_count - 1 {
            self.col_end += 1;
            if !expand {
                self.col_start += 1;
            }
        }
    }

    pub fn move_up(&mut self, expand: bool) {
        if self.row_start > 0 {
            self.row_start = self.row_start - 1;
            if !expand {
                self.row_end = self.row_end - 1;
            }
        }
    }

    pub fn move_down(&mut self, expand: bool, row_count: usize) {
        if self.row_end < row_count - 1 {
            self.row_end += 1;
            if !expand {
                self.row_start = self.row_start + 1;
            }
        }
    }

    pub fn width(&self) -> usize {
        self.col_end - self.col_start + 1
    }

    pub fn height(&self) -> usize {
        self.row_end - self.row_start + 1
    }
}
