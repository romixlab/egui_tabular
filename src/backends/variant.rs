use crate::backend::{
    BackendColumn, CellCoord, ColumnUid, OneShotFlags, PersistentFlags, RowUid, TableBackend,
    VisualRowIdx,
};
use egui::{ComboBox, DragValue, Response, TextEdit, Ui, Widget};
use rvariant::{Variant, VariantTy};
use std::cell::Cell;
use std::collections::HashMap;

pub struct VariantBackend {
    cell_data: HashMap<CellCoord, Variant>,
    row_order: Vec<RowUid>,
    next_row_uid: RowUid,
    columns: HashMap<ColumnUid, (BackendColumn, VariantColumn)>,
    cell_edit: Cell<Option<(CellCoord, Variant)>>,
    persistent_flags: PersistentFlags,
    one_shot_flags: OneShotFlags,
}

struct VariantColumn {
    ty: VariantTy,
    default: Option<Variant>,
}

impl VariantBackend {
    pub fn new(columns: impl IntoIterator<Item = (String, VariantTy, Option<Variant>)>) -> Self {
        VariantBackend {
            cell_data: Default::default(),
            row_order: vec![],
            next_row_uid: RowUid(0),
            columns: columns
                .into_iter()
                .enumerate()
                .map(|(idx, (name, ty, default))| {
                    let col_uid = ColumnUid(idx as u32);
                    let backend_column = BackendColumn {
                        name,
                        synonyms: vec![],
                        ty: format!("{ty}"),
                        is_sortable: true,
                        is_required: true,
                        is_used: true,
                    };
                    let variant_column = VariantColumn { ty: ty, default };
                    (col_uid, (backend_column, variant_column))
                })
                .collect(),
            cell_edit: Cell::new(None),
            persistent_flags: PersistentFlags {
                is_read_only: false,
                column_info_present: true,
                row_set_present: true,
                ..Default::default()
            },
            one_shot_flags: OneShotFlags {
                column_info_updated: true,
                ..Default::default()
            },
        }
    }

    pub fn insert_row(&mut self, values: impl IntoIterator<Item = (ColumnUid, Variant)>) {
        let mut provided_cells = vec![];
        for (col_uid, v) in values {
            let coord = CellCoord {
                row_uid: self.next_row_uid,
                col_uid,
            };
            self.cell_data.insert(coord, v);
            provided_cells.push(col_uid);
        }
        for (col_uid, (_, col)) in &self.columns {
            if let Some(default) = &col.default {
                if !provided_cells.contains(col_uid) {
                    let coord = CellCoord {
                        row_uid: self.next_row_uid,
                        col_uid: *col_uid,
                    };
                    self.cell_data.insert(coord, default.clone());
                }
            }
        }
        self.row_order.push(self.next_row_uid);
        self.next_row_uid = RowUid(self.next_row_uid.0 + 1)
    }

    /// Remove all columns and all data
    pub fn remove_all_columns(&mut self) {
        self.columns.clear();
        self.clear();
        self.one_shot_flags.column_info_updated = true;
    }

    pub fn insert_column(
        &mut self,
        col_uid: ColumnUid,
        name: String,
        synonyms: Vec<String>,
        ty: VariantTy,
        default: Option<Variant>,
        is_required: bool,
        is_used: bool,
    ) {
        let backend_column = BackendColumn {
            name,
            synonyms,
            ty: format!("{ty}"),
            is_sortable: true,
            is_required,
            is_used,
        };
        let variant_column = VariantColumn { ty: ty, default };
        self.columns
            .insert(col_uid, (backend_column, variant_column));
        self.one_shot_flags.column_info_updated = true;
    }
}

impl TableBackend for VariantBackend {
    fn clear(&mut self) {
        self.cell_data.clear();
        self.row_order.clear();
        self.next_row_uid = RowUid(0);
    }

    fn persistent_flags(&self) -> &PersistentFlags {
        &self.persistent_flags
    }

    fn one_shot_flags(&self) -> &OneShotFlags {
        &self.one_shot_flags
    }

    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags {
        &mut self.one_shot_flags
    }

    fn available_columns(&self) -> impl Iterator<Item = ColumnUid> {
        self.columns.keys().copied()
    }

    fn used_columns(&self) -> impl Iterator<Item = ColumnUid> {
        self.columns.keys().copied()
    }

    fn column_info(&self, col_uid: ColumnUid) -> Option<&BackendColumn> {
        self.columns.get(&col_uid).map(|(b, _)| b)
    }

    fn row_count(&self) -> usize {
        self.row_order.len()
    }

    fn row_uid(&self, row_idx: VisualRowIdx) -> Option<RowUid> {
        self.row_order.get(row_idx.0).copied()
    }

    fn show_cell_view(&self, coord: CellCoord, ui: &mut Ui) {
        let Some(value) = self.cell_data.get(&coord) else {
            return;
        };
        match value {
            Variant::Empty => {}
            Variant::Bool(v) => {
                let mut v = *v;
                ui.checkbox(&mut v, "");
            }
            Variant::Str(v) => {
                ui.label(v);
            }
            Variant::StrList(list) => {
                for (idx, v) in list.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{idx}:"));
                        ui.label(v);
                    });
                }
            }
            // Variant::List(_list) => {
            //
            // }
            other => {
                ui.label(other.to_string().as_str());
            }
        }
    }

    fn show_cell_editor(&self, coord: CellCoord, ui: &mut Ui) -> Option<Response> {
        const INT_DRAG_SPEED: f32 = 0.1;

        let cell_ty = self
            .columns
            .get(&coord.col_uid)
            .map(|(_, c)| c.ty)
            .unwrap_or(VariantTy::Str);

        let mut value = if let Some((prev_coord, value)) = self.cell_edit.take() {
            if prev_coord == coord {
                value
            } else {
                self.cell_data
                    .get(&coord)
                    .cloned()
                    .unwrap_or(Variant::default_of(cell_ty))
            }
        } else {
            self.cell_data
                .get(&coord)
                .cloned()
                .unwrap_or(Variant::default_of(cell_ty))
        };
        let resp = match &mut value {
            Variant::Enum {
                enum_uid,
                discriminant: discriminant_edit,
            } => {
                let resp = ComboBox::from_id_salt("_egui_tabular_enum_edit")
                    .selected_text(
                        rvariant::uid_to_variant_name(*enum_uid, *discriminant_edit).expect(""),
                    )
                    // .width(ui_column.width)
                    .show_ui(ui, |ui| {
                        let mut changed = false;
                        for (d, v) in rvariant::variant_names(*enum_uid).expect("") {
                            changed |= ui.selectable_value(discriminant_edit, *d, v).changed();
                        }
                        changed
                    })
                    .response;
                Some(resp)
            }
            Variant::Str(edit_text) => {
                // let edit = if first_pass {
                //     let edit = TextEdit::singleline(edit_text)
                //         .cursor_at_end(false)
                //         .desired_width(f32::INFINITY)
                //         .ui(ui);
                //     edit.request_focus();
                //     edit
                // } else {
                let resp = TextEdit::singleline(edit_text)
                    .desired_width(f32::INFINITY)
                    .ui(ui);

                Some(resp)
                // };
                // if edit.lost_focus() {
                //     let converted = Variant::from_str(edit_text, cell_ty);
                //     Some(converted)
                // } else {
                //     None
                // }
            }
            Variant::U32(num) => {
                let resp = ui
                    .horizontal(|ui| {
                        ui.label("u32:");
                        if ui
                            .add(DragValue::new(num).speed(INT_DRAG_SPEED))
                            .lost_focus()
                        {
                            Some(Variant::U32(*num))
                        } else {
                            None
                        }
                    })
                    .response;
                Some(resp)
            }
            Variant::U64(num) => {
                let resp = ui
                    .horizontal(|ui| {
                        ui.label("u64:");
                        if ui
                            .add(DragValue::new(num).speed(INT_DRAG_SPEED))
                            .lost_focus()
                        {
                            Some(Variant::U64(*num))
                        } else {
                            None
                        }
                    })
                    .response;
                Some(resp)
            }
            v => {
                ui.label(format!("Editor is not implemented for {v}"));
                None
            }
        };
        self.cell_edit.set(Some((coord, value)));
        resp
    }

    fn commit_cell_edit(&mut self, coord: CellCoord) {
        if let Some((last_edited_coord, value)) = self.cell_edit.take() {
            if last_edited_coord == coord {
                self.cell_data.insert(coord, value);
            }
        }
    }
}
