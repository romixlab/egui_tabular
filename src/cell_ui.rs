//! The egui side of a table: how cells are shown and edited.

use crate::row_order::ColumnFilter;
use chrono::{
    DateTime, Duration, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike,
};
use egui::{
    ComboBox, DragValue, Label, Response, RichText, Sense, Stroke, TextEdit, TextWrapMode, Ui,
    Vec2, Widget,
};
use rvariant::{Number, Variant, VariantTy};
use std::borrow::Cow;
use std::cmp::Ordering;
use tabular_core::{CellCoord, CellLevel, ColumnUid, RowUid, TableModel, WrapMode};

/// Shows and edits the cells of a model `M`.
///
/// Implement [`show_row`](Self::show_row) to look a row up once and fill its cells, or
/// [`show_cell`](Self::show_cell) for one cell at a time. The defaults show and edit the model's
/// `Variant` values, so [`VariantCellUi`] needs no code.
///
/// The view draws everything around the content: background, selection, metadata colors,
/// corner markers, tooltips and the skipped state.
pub trait CellUi<M: TableModel> {
    /// View mode for one visible row. Look the row up once, then fill cells by column with
    /// [`RowCells::cell`], in any order. Cells that aren't filled stay empty.
    ///
    /// The default calls [`show_cell`](Self::show_cell) for each cell.
    fn show_row(&mut self, model: &mut M, row: RowUid, cells: &mut RowCells<'_>) {
        for &col in cells.columns() {
            cells.cell(col, |ui| {
                self.show_cell(
                    model,
                    CellCoord {
                        row_uid: row,
                        col_uid: col,
                    },
                    ui,
                )
            });
        }
    }

    /// View mode for one cell. Gets `&mut M`, so that a cell can host an interactive widget that
    /// writes to the model (e.g. a register write button). Such writes don't go through the
    /// view's commands: they aren't undoable and must not change the row or column set.
    ///
    /// The default shows the model's value.
    fn show_cell(&mut self, model: &mut M, coord: CellCoord, ui: &mut Ui) {
        show_value(model, coord, ui);
    }

    /// Plain text for copy, export and filters, used when the model has no value for the cell.
    fn text(&self, model: &M, coord: CellCoord) -> Option<String> {
        let _ = (model, coord);
        None
    }

    /// Initial editor value. `None` means the cell is not editable.
    ///
    /// The default takes the model's value, or the column type's default for an empty cell.
    fn begin_edit(&mut self, model: &M, coord: CellCoord) -> Option<Variant> {
        match model.get(coord) {
            Some(value) => Some(value.into_owned()),
            None => model
                .column(coord.col_uid)?
                .ty
                .as_ref()
                .map(Variant::default_of),
        }
    }

    /// Edit `value`, which the view owns until the edit is committed or cancelled. The view
    /// handles focus, Enter, Escape and Tab; set [`EditorResponse::commit`] to commit right away
    /// (e.g. after a choice in a combo box).
    fn show_editor(
        &mut self,
        model: &M,
        coord: CellCoord,
        value: &mut Variant,
        ui: &mut Ui,
    ) -> EditorResponse {
        let _ = (model, coord);
        edit_value(value, ui)
    }

    /// Order of rows `a` and `b` by column `col`, ascending; the view reverses it for a
    /// descending key. For columns the model has no values for, or that sort by something else
    /// than their value. `None` (the default): the view compares the model's values.
    fn compare(&self, model: &M, col: ColumnUid, a: RowUid, b: RowUid) -> Option<Ordering> {
        let _ = (model, col, a, b);
        None
    }

    /// Whether the cell passes `filter`. `None` (the default): the view's built-in evaluation of
    /// the filter kind. [`ColumnFilter::Custom`] filters have no built-in meaning: without this
    /// hook every row passes them.
    fn matches(&self, model: &M, coord: CellCoord, filter: &ColumnFilter) -> Option<bool> {
        let _ = (model, coord, filter);
        None
    }

    /// Extra content in a column header, above the name.
    fn header_ui(&mut self, model: &mut M, col: ColumnUid, ui: &mut Ui) {
        let _ = (model, col, ui);
    }
}

/// Result of [`CellUi::show_editor`].
pub struct EditorResponse {
    /// The editor widget. The view gives it focus when the edit starts.
    pub response: Response,
    /// Commit now, without waiting for Enter, Tab or a click elsewhere.
    pub commit: bool,
}

/// Shows and edits any model's `Variant` values with the built-in viewers and editors.
///
/// Dates, times and date-times use the default [`DateFormats`]; [`DateCellUi`] sets your own.
pub struct VariantCellUi;

impl<M: TableModel> CellUi<M> for VariantCellUi {}

/// The cells of one visible row, given to [`CellUi::show_row`].
pub struct RowCells<'a> {
    row: RowUid,
    columns: &'a [ColumnUid],
    slots: &'a mut [CellSlot],
}

/// One cell of a [`RowCells`], created by the view.
pub(crate) struct CellSlot {
    pub(crate) ui: Ui,
    /// False for the cell that shows the editor.
    pub(crate) shown: bool,
    pub(crate) level: Option<CellLevel>,
    pub(crate) tooltips: Vec<String>,
}

impl<'a> RowCells<'a> {
    pub(crate) fn new(row: RowUid, columns: &'a [ColumnUid], slots: &'a mut [CellSlot]) -> Self {
        debug_assert_eq!(columns.len(), slots.len());
        RowCells {
            row,
            columns,
            slots,
        }
    }

    pub fn row(&self) -> RowUid {
        self.row
    }

    /// Shown columns of this row in visual order.
    pub fn columns(&self) -> &'a [ColumnUid] {
        self.columns
    }

    /// Whether the cell of this column is shown, i.e. [`cell`](Self::cell) would call its
    /// closure. Use it to skip costly work for columns that aren't on screen.
    pub fn wants(&self, col: ColumnUid) -> bool {
        self.position(col).is_some_and(|idx| self.slots[idx].shown)
    }

    /// Fill the cell of `col`. Any order; filling a cell twice adds to it. Does nothing and
    /// returns `None` if the column isn't shown.
    pub fn cell<R>(&mut self, col: ColumnUid, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let slot = &mut self.slots[self.position(col)?];
        slot.shown.then(|| add(&mut slot.ui))
    }

    /// Highlight the cell of `col`, instead of the model's [`CellMetadata`](tabular_core::CellMetadata) level.
    pub fn level(&mut self, col: ColumnUid, level: CellLevel) {
        if let Some(idx) = self.position(col) {
            self.slots[idx].level = Some(level);
        }
    }

    /// Add a hover text to the cell of `col`.
    pub fn tooltip(&mut self, col: ColumnUid, text: impl Into<String>) {
        if let Some(idx) = self.position(col) {
            self.slots[idx].tooltips.push(text.into());
        }
    }

    fn position(&self, col: ColumnUid) -> Option<usize> {
        self.columns.iter().position(|c| *c == col)
    }
}

/// Built-in viewer: shows the model's value of a cell.
pub fn show_value<M: TableModel>(model: &M, coord: CellCoord, ui: &mut Ui) {
    show_value_with(model, coord, ui, &DateFormats::default());
}

/// [`show_value`] with custom date, time and date-time formats.
pub fn show_value_with<M: TableModel>(
    model: &M,
    coord: CellCoord,
    ui: &mut Ui,
    formats: &DateFormats,
) {
    let Some(value) = model.get(coord) else {
        return;
    };
    let wrap_mode = model
        .metadata(coord)
        .and_then(|m| m.wrap_mode)
        .map(|mode| match mode {
            WrapMode::Extend => TextWrapMode::Extend,
            WrapMode::Wrap => TextWrapMode::Wrap,
            WrapMode::Truncate => TextWrapMode::Truncate,
        });
    let label = |text: Cow<'_, str>| {
        let label = Label::new(RichText::new(text));
        match wrap_mode {
            Some(mode) => label.wrap_mode(mode),
            None => label,
        }
    };
    match value.as_ref() {
        Variant::Empty => {}
        Variant::Bool(v) => check_mark(ui, *v),
        Variant::Str(v) => {
            ui.add(label(Cow::Borrowed(v)));
        }
        Variant::StrList(list) => {
            for (idx, v) in list.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{idx}:"));
                    ui.add(label(Cow::Borrowed(v)));
                });
            }
        }
        v @ (Variant::Date(_) | Variant::Time(_) | Variant::DateTime(_)) => {
            let response = ui.add(label(Cow::Owned(formats.display(v))));
            if formats.relative_days {
                response.on_hover_text(formats.absolute(v));
            }
        }
        other => {
            ui.add(label(Cow::Owned(other.to_string())));
        }
    }
}

/// A read-only check box. A real (or disabled) `Checkbox` would take the click that should
/// select the cell (EDIT-7).
fn check_mark(ui: &mut Ui, checked: bool) {
    let size = ui.spacing().icon_width;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let visuals = &ui.visuals().widgets.noninteractive;
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        visuals.corner_radius,
        visuals.fg_stroke,
        egui::StrokeKind::Inside,
    );
    if checked {
        let r = rect.shrink(size * 0.2);
        painter.line(
            vec![
                r.left_center(),
                egui::pos2(r.center().x - r.width() * 0.1, r.bottom()),
                r.right_top(),
            ],
            Stroke::new(1.5, visuals.fg_stroke.color),
        );
    }
}

/// Built-in editor for a `Variant` value.
pub fn edit_value(value: &mut Variant, ui: &mut Ui) -> EditorResponse {
    edit_value_with(value, ui, &DateFormats::default())
}

/// [`edit_value`] with custom date, time and date-time formats.
pub fn edit_value_with(value: &mut Variant, ui: &mut Ui, formats: &DateFormats) -> EditorResponse {
    const INT_DRAG_SPEED: f32 = 0.1;
    let (response, commit) = match value {
        Variant::Bool(v) => {
            let response = ui.checkbox(v, "");
            let commit = response.changed();
            (response, commit)
        }
        Variant::Enum {
            selected, variants, ..
        } => {
            let inner = ComboBox::from_id_salt("_egui_tabular_enum_edit")
                .selected_text(selected.as_str())
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    for v in variants.iter() {
                        changed |= ui.selectable_value(selected, v.clone(), v).changed();
                    }
                    changed
                });
            (inner.response, inner.inner == Some(true))
        }
        Variant::Str(text) => (
            TextEdit::singleline(text)
                .desired_width(f32::INFINITY)
                .ui(ui),
            false,
        ),
        Variant::Number(Number::U32(num)) => {
            (ui.add(DragValue::new(num).speed(INT_DRAG_SPEED)), false)
        }
        Variant::Number(Number::U64(num)) => {
            (ui.add(DragValue::new(num).speed(INT_DRAG_SPEED)), false)
        }
        Variant::Number(Number::I32(num)) => {
            (ui.add(DragValue::new(num).speed(INT_DRAG_SPEED)), false)
        }
        Variant::Number(Number::I64(num)) => {
            (ui.add(DragValue::new(num).speed(INT_DRAG_SPEED)), false)
        }
        Variant::Date(_) | Variant::Time(_) | Variant::DateTime(_) => {
            (edit_date(value, ui, formats), false)
        }
        v => (
            ui.label(format!(
                "Editor is not implemented for {}",
                VariantTy::from(&*v)
            )),
            false,
        ),
    };
    EditorResponse { response, commit }
}

/// Like [`VariantCellUi`], with your own [`DateFormats`] for `Date`, `Time` and `DateTime` cells.
#[derive(Clone, Debug, Default)]
pub struct DateCellUi {
    pub formats: DateFormats,
}

impl<M: TableModel> CellUi<M> for DateCellUi {
    fn show_cell(&mut self, model: &mut M, coord: CellCoord, ui: &mut Ui) {
        show_value_with(model, coord, ui, &self.formats);
    }

    fn show_editor(
        &mut self,
        model: &M,
        coord: CellCoord,
        value: &mut Variant,
        ui: &mut Ui,
    ) -> EditorResponse {
        let _ = (model, coord);
        edit_value_with(value, ui, &self.formats)
    }
}

/// How `Date`, `Time` and `DateTime` values are shown, and the text the editor parses.
///
/// Format strings are [chrono `strftime`](https://docs.rs/chrono/latest/chrono/format/strftime/)
/// patterns. The editor accepts the configured format first, then ISO 8601.
#[derive(Clone, Debug)]
pub struct DateFormats {
    /// Format of `Date` values, `%Y-%m-%d` by default.
    pub date: String,
    /// Format of `Time` values, `%H:%M:%S` by default.
    pub time: String,
    /// Format of `DateTime` values, `%Y-%m-%d %H:%M:%S` by default. Without an offset in the
    /// format (`%z`, `%:z`), the editor keeps the offset of the value being edited.
    pub datetime: String,
    /// Show `Date` and `DateTime` as days from today ("today", "3 d ago", "in 2 d"), with the
    /// formatted value as a tooltip. The editor accepts the same words as well.
    pub relative_days: bool,
    /// What "today" is for `relative_days`; `None` is the local date. Set it for tests.
    pub today: Option<NaiveDate>,
}

impl Default for DateFormats {
    fn default() -> Self {
        DateFormats {
            date: "%Y-%m-%d".into(),
            time: "%H:%M:%S".into(),
            datetime: "%Y-%m-%d %H:%M:%S".into(),
            relative_days: false,
            today: None,
        }
    }
}

impl DateFormats {
    fn today(&self) -> NaiveDate {
        self.today.unwrap_or_else(|| Local::now().date_naive())
    }

    /// The value in its configured format, never relative.
    pub fn absolute(&self, value: &Variant) -> String {
        match value {
            Variant::Date(d) => d.format(&self.date).to_string(),
            Variant::Time(t) => t.format(&self.time).to_string(),
            Variant::DateTime(dt) => dt.format(&self.datetime).to_string(),
            other => other.to_string(),
        }
    }

    /// The text shown in a cell: relative days if enabled, else [`absolute`](Self::absolute).
    pub fn display(&self, value: &Variant) -> String {
        if self.relative_days {
            let date = match value {
                Variant::Date(d) => Some(*d),
                Variant::DateTime(dt) => Some(dt.date_naive()),
                _ => None,
            };
            if let Some(date) = date {
                return relative_days_text((date - self.today()).num_days());
            }
        }
        self.absolute(value)
    }

    /// Parse `text` into a value of the same kind as `like`.
    pub fn parse(&self, text: &str, like: &Variant) -> Option<Variant> {
        let text = text.trim();
        match like {
            Variant::Date(_) => self.parse_date(text).map(Variant::Date),
            Variant::Time(_) => NaiveTime::parse_from_str(text, &self.time)
                .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M:%S%.f"))
                .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M"))
                .ok()
                .map(Variant::Time),
            Variant::DateTime(old) => self.parse_datetime(text, *old.offset()),
            _ => None,
        }
    }

    fn parse_date(&self, text: &str) -> Option<NaiveDate> {
        if self.relative_days
            && let Some(days) = parse_relative_days(text)
        {
            return self.today().checked_add_signed(Duration::days(days));
        }
        NaiveDate::parse_from_str(text, &self.date)
            .or_else(|_| NaiveDate::parse_from_str(text, "%Y-%m-%d"))
            .ok()
    }

    fn parse_datetime(&self, text: &str, offset: FixedOffset) -> Option<Variant> {
        if self.relative_days
            && let Some(days) = parse_relative_days(text)
        {
            let date = self.today().checked_add_signed(Duration::days(days))?;
            let dt = date
                .and_time(NaiveTime::MIN)
                .and_local_timezone(offset)
                .single()?;
            return Some(Variant::DateTime(dt));
        }
        let local = |naive: NaiveDateTime| naive.and_local_timezone(offset).single();
        DateTime::parse_from_str(text, &self.datetime)
            .ok()
            .or_else(|| {
                NaiveDateTime::parse_from_str(text, &self.datetime)
                    .ok()
                    .and_then(local)
            })
            .or_else(|| DateTime::parse_from_rfc3339(text).ok())
            .or_else(|| {
                NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .and_then(local)
            })
            .map(Variant::DateTime)
    }
}

fn relative_days_text(days: i64) -> String {
    match days {
        0 => "today".into(),
        -1 => "yesterday".into(),
        1 => "tomorrow".into(),
        d if d < 0 => format!("{} d ago", -d),
        d => format!("in {d} d"),
    }
}

/// Inverse of [`relative_days_text`].
fn parse_relative_days(text: &str) -> Option<i64> {
    let text = text.to_lowercase();
    match text.as_str() {
        "today" => return Some(0),
        "yesterday" => return Some(-1),
        "tomorrow" => return Some(1),
        _ => {}
    }
    if let Some(n) = text.strip_suffix(" d ago") {
        return n.trim().parse::<i64>().ok().map(|n| -n);
    }
    text.strip_prefix("in ")?
        .strip_suffix(" d")?
        .trim()
        .parse()
        .ok()
}

/// Text editor for date, time and date-time values. The text is parsed as it is typed: a valid
/// text updates `value`, an invalid one is outlined in red and leaves `value` as it was, so a
/// commit keeps the last valid date.
fn edit_date(value: &mut Variant, ui: &mut Ui, formats: &DateFormats) -> Response {
    // (text, formatted value when the text was last in sync)
    type Buffer = (String, String);
    let id = ui.id().with("_egui_tabular_date_edit");
    let current = formats.display(value);
    let (mut text, synced) = ui
        .data(|d| d.get_temp::<Buffer>(id))
        .unwrap_or_else(|| (current.clone(), current.clone()));
    if synced != current {
        // The value changed under the editor (a new edit in a recycled widget): start over.
        text = current.clone();
    }
    let parsed = formats.parse(&text, value);
    let invalid = parsed.is_none() && !text.trim().is_empty();
    let mut edit = TextEdit::singleline(&mut text).desired_width(f32::INFINITY);
    if invalid {
        edit = edit.text_color(ui.visuals().error_fg_color);
    }
    let response = edit.ui(ui);
    if let Some(parsed) = formats.parse(&text, value) {
        *value = parsed;
    }
    let synced = formats.display(value);
    ui.data_mut(|d| d.insert_temp::<Buffer>(id, (text, synced)));
    response
}

/// A number that orders `Date`, `Time` and `DateTime` values of any kind: seconds since the
/// epoch (dates at midnight, UTC), or since midnight for times. `None` for other values.
/// Use it as the sort key of a date column.
pub fn date_sort_key(value: &Variant) -> Option<i64> {
    match value {
        Variant::Date(d) => Some(d.and_time(NaiveTime::MIN).and_utc().timestamp()),
        Variant::Time(t) => Some(i64::from(t.num_seconds_from_midnight())),
        Variant::DateTime(dt) => Some(dt.timestamp()),
        _ => None,
    }
}

/// Filter for date columns: keeps values between `from` and `to`, both ends included and each
/// optional. This is the date flavor of the planned `ColumnFilter::Range` (FEATURES.md,
/// DESIGN-13). Values that aren't dates, times or date-times, and empty cells, never match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DateRange {
    pub from: Option<Variant>,
    pub to: Option<Variant>,
}

impl DateRange {
    pub fn matches(&self, value: Option<&Variant>) -> bool {
        let Some(key) = value.and_then(date_sort_key) else {
            return false;
        };
        let within = |bound: &Option<Variant>, ok: fn(i64, i64) -> bool| match bound {
            None => true,
            Some(b) => date_sort_key(b).is_some_and(|b| ok(key, b)),
        };
        within(&self.from, |k, b| k >= b) && within(&self.to, |k, b| k <= b)
    }
}
