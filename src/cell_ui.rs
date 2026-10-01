//! The egui side of a table: how cells are shown and edited.

use egui::{
    ComboBox, DragValue, Label, Response, RichText, Sense, Stroke, TextEdit, TextWrapMode, Ui,
    Vec2, Widget,
};
use rvariant::{Number, Variant, VariantTy};
use std::borrow::Cow;
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

    /// Plain text for copy and export, used when the model has no value for the cell.
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
