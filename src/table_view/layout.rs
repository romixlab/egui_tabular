//! View-owned layout state: anchor-based vertical scrolling and column widths.
//!
//! The vertical scroll position is a row (by uid) plus a pixel offset into that row, not a pixel
//! offset from the top of the table. Each frame rows are laid out downwards from the anchor until
//! the viewport is full, so only rows in view are measured and the total pixel height of the table
//! is never needed. Measured heights are cached by [`RowUid`], so they follow rows through inserts,
//! removals and reordering.

use egui::emath::GuiRounding;
use std::collections::{HashMap, HashSet};
use tabular_core::{ColumnUid, RowUid};

/// Auto-sized columns grow up to this width (content area, without padding).
pub(super) const MAX_AUTO_COLUMN_WIDTH: f32 = 400.0;
pub(super) const MIN_COLUMN_WIDTH: f32 = 20.0;
const TOOL_COLUMN_WIDTH: f32 = 48.0;
const WIDTH_ANIMATION_TIME: f32 = 0.12;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Anchor {
    /// Row at the top of the viewport, `None` if there are no rows.
    row: Option<RowUid>,
    /// Index of `row` in the visible order. Used as a fallback when `row` disappears.
    idx: usize,
    /// How many pixels of the anchor row are scrolled above the viewport top.
    /// `0..row_height` after [`RowLayout::normalize`].
    offset: f32,
}

#[derive(Copy, Clone, Debug)]
enum Reveal {
    Index(usize),
    Row(RowUid),
}

#[derive(Default)]
pub(super) struct RowLayout {
    rows: Vec<RowUid>,
    index: HashMap<RowUid, usize>,
    /// Measured full row heights (including spacing).
    heights: HashMap<RowUid, f32>,
    anchor: Anchor,
    /// Height of rows not measured yet, and of all rows in uniform mode.
    default_height: f32,
    uniform: bool,
    /// The end of the table was in view after the last layout.
    at_bottom: bool,
    /// The last layout didn't fit into the viewport.
    overflows: bool,
    /// Rows that fit into the viewport during the last layout (fractional), for the scroll bar.
    visible_count: f32,
    reveal: Option<Reveal>,
}

impl RowLayout {
    /// Rows in display order.
    pub(super) fn rows(&self) -> &[RowUid] {
        &self.rows
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn set_metrics(&mut self, default_height: f32, uniform: bool) {
        self.default_height = default_height.max(1.0);
        self.uniform = uniform;
    }

    /// Replace the display order. Keeps the anchor on the same row if it still exists, and keeps
    /// the view at the end of the table when rows were appended while it was there.
    pub(super) fn sync(&mut self, rows: Vec<RowUid>) {
        let grew = rows.len() > self.rows.len();
        if grew && rows.starts_with(&self.rows) {
            // Rows appended: only index the new ones.
            let start = self.rows.len();
            self.index.extend(
                rows[start..]
                    .iter()
                    .enumerate()
                    .map(|(i, uid)| (*uid, start + i)),
            );
        } else {
            self.index = rows.iter().enumerate().map(|(i, uid)| (*uid, i)).collect();
            self.heights.retain(|uid, _| self.index.contains_key(uid));
        }
        self.rows = rows;
        if let Some(Reveal::Row(row)) = self.reveal
            && !self.index.contains_key(&row)
        {
            self.reveal = None;
        }
        match self.anchor.row.and_then(|uid| self.index.get(&uid)) {
            Some(&idx) => self.anchor.idx = idx,
            None => {
                self.anchor.idx = self.anchor.idx.min(self.rows.len().saturating_sub(1));
                self.anchor.offset = 0.0;
            }
        }
        self.anchor.row = self.rows.get(self.anchor.idx).copied();
        if grew && self.at_bottom {
            self.scroll_to_end();
        }
    }

    /// Full height of the row at `idx`: measured, or the default if not measured yet.
    pub(super) fn height(&self, idx: usize) -> f32 {
        if self.uniform {
            return self.default_height;
        }
        self.rows
            .get(idx)
            .and_then(|uid| self.heights.get(uid))
            .copied()
            .unwrap_or(self.default_height)
    }

    /// Record a measured height. Returns true if it differs from what was assumed for the layout.
    pub(super) fn set_measured(&mut self, idx: usize, height: f32) -> bool {
        let previous = self.height(idx);
        if let Some(uid) = self.rows.get(idx) {
            self.heights.insert(*uid, height);
        }
        (previous - height).abs() > 0.5
    }

    /// First row to lay out, and how far its top is above the viewport top.
    pub(super) fn anchor(&self) -> (usize, f32) {
        (self.anchor.idx, self.anchor.offset)
    }

    /// Positive `dy` scrolls towards the end of the table.
    pub(super) fn scroll_by(&mut self, dy: f32) {
        self.anchor.offset += dy;
    }

    pub(super) fn scroll_to_end(&mut self) {
        if let Some(last) = self.rows.len().checked_sub(1) {
            self.anchor.idx = last;
            self.anchor.offset = self.height(last);
        }
    }

    /// Fractional row index of the viewport top.
    pub(super) fn position(&self) -> f32 {
        self.anchor.idx as f32 + self.anchor.offset / self.height(self.anchor.idx)
    }

    pub(super) fn scroll_to_position(&mut self, position: f32) {
        if self.rows.is_empty() {
            return;
        }
        let position = position.clamp(0.0, self.rows.len() as f32);
        let idx = (position.floor() as usize).min(self.rows.len() - 1);
        self.anchor.idx = idx;
        self.anchor.offset = (position - idx as f32) * self.height(idx);
    }

    /// Scroll just enough to bring the row at `idx` fully into view on the next layout.
    pub(super) fn reveal(&mut self, idx: usize) {
        self.reveal = Some(Reveal::Index(idx));
    }

    /// Like [`Self::reveal`], for a row that may not be in the display order yet (just created).
    pub(super) fn reveal_row(&mut self, row: RowUid) {
        self.reveal = Some(Reveal::Row(row));
    }

    /// Apply a pending reveal and clamp the anchor so that the viewport is covered with rows
    /// wherever possible: no gap above the first row, and no gap below the last one unless the
    /// whole table fits.
    pub(super) fn normalize(&mut self, viewport_height: f32) {
        let len = self.rows.len();
        if len == 0 {
            self.anchor = Anchor::default();
            self.reveal = None;
            return;
        }
        let mut idx = self.anchor.idx.min(len - 1);
        let mut offset = if self.anchor.offset.is_finite() {
            self.anchor.offset
        } else {
            0.0
        };

        let reveal = match self.reveal {
            Some(Reveal::Index(idx)) => Some(idx),
            Some(Reveal::Row(row)) => self.index.get(&row).copied(),
            None => None,
        };
        // A row that isn't synced yet stays pending; `sync` drops it if it never appears.
        if reveal.is_some() {
            self.reveal = None;
        }
        if let Some(target) = reveal.filter(|&r| r < len) {
            if target < idx || (target == idx && offset > 0.0) {
                idx = target;
                offset = 0.0;
            } else {
                let bottom = (idx..=target).map(|i| self.height(i)).sum::<f32>() - offset;
                if bottom > viewport_height {
                    // Align the row's bottom with the viewport bottom, or its top with the
                    // viewport top if it is taller than the viewport.
                    idx = target;
                    offset = (self.height(target) - viewport_height).min(0.0);
                }
            }
        }

        while idx + 1 < len && offset >= self.height(idx) {
            offset -= self.height(idx);
            idx += 1;
        }
        while offset < 0.0 && idx > 0 {
            idx -= 1;
            offset += self.height(idx);
        }
        offset = offset.max(0.0);

        let mut covered = -offset;
        let mut i = idx;
        while covered < viewport_height && i < len {
            covered += self.height(i);
            i += 1;
        }
        if covered < viewport_height {
            offset -= viewport_height - covered;
            while offset < 0.0 && idx > 0 {
                idx -= 1;
                offset += self.height(idx);
            }
            offset = offset.max(0.0);
        }

        self.anchor = Anchor {
            row: Some(self.rows[idx]),
            idx,
            offset,
        };
    }

    /// Record the outcome of a layout pass.
    /// `end` is one past the last laid out row, `content_bottom` is the bottom of that row
    /// relative to the viewport top.
    pub(super) fn finish_layout(&mut self, end: usize, content_bottom: f32, viewport_height: f32) {
        let (start, offset) = self.anchor();
        let all_in_view = end >= self.rows.len() && content_bottom <= viewport_height + 0.5;
        self.at_bottom = all_in_view;
        self.overflows = !(all_in_view && start == 0 && offset <= 0.0);
        let mut visible = end.saturating_sub(start) as f32;
        if let Some(last) = end.checked_sub(1)
            && content_bottom > viewport_height
        {
            visible -= (content_bottom - viewport_height) / self.height(last);
        }
        visible -= offset / self.height(start);
        self.visible_count = visible.max(0.0);
    }

    pub(super) fn overflows(&self) -> bool {
        self.overflows
    }

    pub(super) fn visible_count(&self) -> f32 {
        self.visible_count
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum ColumnKey {
    Tool,
    Data(ColumnUid),
}

#[derive(Copy, Clone, Debug)]
pub(super) struct ColumnWidth {
    /// Full width of the column, including padding.
    pub(super) width: f32,
    /// Grows to fit the widest content seen, until the user resizes the column.
    pub(super) auto: bool,
}

#[derive(Default)]
pub(super) struct ColumnWidths {
    /// Columns not in the map have never been measured and use the defaults from [`Self::get`].
    widths: HashMap<ColumnKey, ColumnWidth>,
    /// Widest content seen per column in the current frame.
    frame_max: HashMap<ColumnKey, f32>,
    /// Auto columns that take this frame's measurement even if it is narrower.
    remeasure: HashSet<ColumnKey>,
    /// Columns measured for the first time; they appear at their width without easing.
    fresh: HashSet<ColumnKey>,
}

impl ColumnWidths {
    pub(super) fn get(&self, key: ColumnKey) -> ColumnWidth {
        self.widths.get(&key).copied().unwrap_or(match key {
            ColumnKey::Tool => ColumnWidth {
                width: TOOL_COLUMN_WIDTH,
                auto: false,
            },
            ColumnKey::Data(_) => ColumnWidth {
                width: MIN_COLUMN_WIDTH,
                auto: true,
            },
        })
    }

    /// True once the column has a measured or user-set width.
    pub(super) fn is_known(&self, key: ColumnKey) -> bool {
        self.widths.contains_key(&key)
    }

    /// How long a width change of this column should be eased over: auto-sizing changes are
    /// animated, user resizing and the first measurement are not.
    pub(super) fn animation_time(&mut self, key: ColumnKey) -> f32 {
        let fresh = self.fresh.remove(&key);
        if fresh || !self.is_known(key) || !self.get(key).auto {
            0.0
        } else {
            WIDTH_ANIMATION_TIME
        }
    }

    /// Record the width a cell's contents need in the current frame.
    pub(super) fn observe(&mut self, key: ColumnKey, used_width: f32) {
        let max = self.frame_max.entry(key).or_insert(0.0);
        *max = max.max(used_width);
    }

    /// Apply this frame's measurements: auto columns grow to fit, or take the measured width
    /// right after [`Self::reset`]. Returns true if any width changed.
    pub(super) fn end_frame(&mut self) -> bool {
        let mut changed = false;
        let measured: Vec<_> = self.frame_max.drain().collect();
        for (key, used) in measured {
            let mut w = self.get(key);
            if !w.auto {
                continue;
            }
            let used = used.max(MIN_COLUMN_WIDTH).ceil();
            let width = if self.remeasure.remove(&key) {
                used
            } else {
                w.width.max(used)
            };
            if !self.widths.contains_key(&key) {
                self.fresh.insert(key);
            }
            if (width - w.width).abs() > 0.5 || !self.widths.contains_key(&key) {
                changed |= (width - w.width).abs() > 0.5;
                w.width = width;
                self.widths.insert(key, w);
            }
        }
        changed
    }

    pub(super) fn resize(&mut self, key: ColumnKey, delta: f32) {
        let w = self.get(key);
        self.widths.insert(
            key,
            ColumnWidth {
                width: (w.width + delta).max(MIN_COLUMN_WIDTH).round_ui(),
                auto: false,
            },
        );
        self.remeasure.remove(&key);
    }

    /// Return a column to auto sizing. The current width is kept until the next measurement,
    /// so the column doesn't collapse first.
    pub(super) fn reset(&mut self, key: ColumnKey) {
        match key {
            ColumnKey::Tool => {
                self.widths.remove(&key);
            }
            ColumnKey::Data(_) => {
                let w = self.get(key);
                self.widths.insert(
                    key,
                    ColumnWidth {
                        width: w.width,
                        auto: true,
                    },
                );
                self.remeasure.insert(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(count: u32, height: f32) -> RowLayout {
        let mut l = RowLayout::default();
        l.set_metrics(height, false);
        l.sync((0..count).map(RowUid).collect());
        l
    }

    #[test]
    fn clamps_to_top_and_bottom() {
        let mut l = layout(100, 10.0);
        l.scroll_by(-50.0);
        l.normalize(95.0);
        assert_eq!(l.anchor(), (0, 0.0));

        l.scroll_by(10_000.0);
        l.normalize(95.0);
        // 100 rows * 10px = 1000px; viewport top must be at 905px.
        assert_eq!(l.anchor(), (90, 5.0));
    }

    #[test]
    fn short_table_stays_at_top() {
        let mut l = layout(3, 10.0);
        l.scroll_by(25.0);
        l.normalize(100.0);
        assert_eq!(l.anchor(), (0, 0.0));
    }

    #[test]
    fn scroll_crosses_measured_rows() {
        let mut l = layout(10, 10.0);
        l.set_measured(0, 30.0);
        l.scroll_by(35.0);
        l.normalize(20.0);
        assert_eq!(l.anchor(), (1, 5.0));
        l.scroll_by(-10.0);
        l.normalize(20.0);
        assert_eq!(l.anchor(), (0, 25.0));
    }

    #[test]
    fn anchor_follows_row_through_insert_and_remove() {
        let mut l = layout(10, 10.0);
        l.scroll_by(52.0);
        l.normalize(20.0);
        assert_eq!(l.anchor(), (5, 2.0));

        // Insert two rows above the anchor row.
        let mut rows: Vec<RowUid> = vec![RowUid(100), RowUid(101)];
        rows.extend((0..10).map(RowUid));
        l.sync(rows);
        assert_eq!(l.anchor(), (7, 2.0));

        // Remove the anchor row: stay at the same index.
        l.sync(
            [100, 101, 0, 1, 2, 3, 4, 6, 7, 8, 9]
                .into_iter()
                .map(RowUid)
                .collect(),
        );
        assert_eq!(l.anchor(), (7, 0.0));
    }

    #[test]
    fn sticks_to_bottom_when_rows_appended() {
        let mut l = layout(10, 10.0);
        l.scroll_to_end();
        l.normalize(30.0);
        l.finish_layout(10, 30.0, 30.0);
        l.sync((0..12).map(RowUid).collect());
        l.normalize(30.0);
        assert_eq!(l.anchor(), (9, 0.0));
    }

    #[test]
    fn reveal_scrolls_minimally() {
        let mut l = layout(100, 10.0);
        l.reveal(20);
        l.normalize(50.0);
        // Row 20 spans 200..210, viewport bottom aligned to 210.
        assert_eq!(l.anchor(), (16, 0.0));

        l.reveal(18);
        l.normalize(50.0);
        assert_eq!(l.anchor(), (16, 0.0));

        l.reveal(3);
        l.normalize(50.0);
        assert_eq!(l.anchor(), (3, 0.0));
    }

    #[test]
    fn empty_table() {
        let mut l = layout(0, 10.0);
        l.scroll_by(100.0);
        l.reveal(3);
        l.normalize(50.0);
        assert_eq!(l.anchor(), (0, 0.0));
        l.scroll_to_end();
        l.scroll_to_position(5.0);
    }

    #[test]
    fn append_keeps_heights_and_reveals_new_row() {
        let mut l = layout(10, 10.0);
        l.set_measured(3, 25.0);
        l.sync((0..11).map(RowUid).collect());
        assert_eq!(l.height(3), 25.0);
        l.reveal_row(RowUid(10));
        l.normalize(30.0);
        assert_eq!(l.anchor(), (8, 0.0));

        // Revealed before it is synced: applied on the first layout after the sync.
        l.reveal_row(RowUid(11));
        l.normalize(30.0);
        l.sync((0..12).map(RowUid).collect());
        l.normalize(30.0);
        assert_eq!(l.anchor(), (9, 0.0));
    }

    #[test]
    fn auto_width_grows_then_remeasures() {
        let key = ColumnKey::Data(ColumnUid(0));
        let mut w = ColumnWidths::default();
        assert!(!w.is_known(key));
        w.observe(key, 80.0);
        w.observe(key, 120.0);
        w.end_frame();
        assert_eq!(w.get(key).width, 120.0);
        w.observe(key, 60.0);
        w.end_frame();
        assert_eq!(
            w.get(key).width,
            120.0,
            "auto columns don't shrink on their own"
        );

        w.resize(key, 30.0);
        w.observe(key, 400.0);
        w.end_frame();
        assert_eq!(w.get(key).width, 150.0, "user width is kept");

        w.reset(key);
        assert_eq!(w.get(key).width, 150.0, "keeps width until measured");
        w.observe(key, 60.0);
        w.end_frame();
        assert_eq!(w.get(key).width, 60.0);
    }

    #[test]
    fn position_round_trip() {
        let mut l = layout(100, 10.0);
        l.scroll_to_position(42.5);
        l.normalize(50.0);
        assert_eq!(l.anchor(), (42, 5.0));
        assert_eq!(l.position(), 42.5);
    }
}
