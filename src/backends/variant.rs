use crate::frontend::TableFrontend;
use egui::{ComboBox, DragValue, Id, Response, TextEdit, Ui, Widget};
use rvariant::{Variant, VariantTy};
use std::collections::HashMap;
use tabular_core::backend::{
    BackendColumn, OneShotFlags, PersistentFlags, TableBackend, VisualRowIdx,
};
use tabular_core::{CellCoord, ColumnUid, RowUid};

pub struct VariantBackend {
    cell_data: HashMap<CellCoord, Variant>,
    row_order: Vec<RowUid>,
    next_row_uid: RowUid,
    columns: HashMap<ColumnUid, (BackendColumn, VariantColumn)>,
    cell_edit: Option<(CellCoord, Variant)>,
    persistent_flags: PersistentFlags,
    one_shot_flags: OneShotFlags,
    one_shot_flags_delay: OneShotFlags,

    column_mapping_choices: Vec<String>,
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
                    let variant_column = VariantColumn { ty, default };
                    (col_uid, (backend_column, variant_column))
                })
                .collect(),
            cell_edit: None,
            persistent_flags: PersistentFlags {
                is_read_only: false,
                column_info_present: true,
                row_set_present: true,
                ..Default::default()
            },
            one_shot_flags: OneShotFlags {
                columns_reset: true,
                row_set_updated: true,
                ..Default::default()
            },
            one_shot_flags_delay: Default::default(),
            column_mapping_choices: vec![],
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
        self.one_shot_flags.row_set_updated = true;
        self.next_row_uid = RowUid(self.next_row_uid.0 + 1)
    }

    /// Remove all columns and all data
    pub fn remove_all_columns(&mut self) {
        self.columns.clear();
        self.clear();
        self.one_shot_flags.columns_reset = true;
    }

    pub fn insert_column(
        &mut self,
        col_uid: Option<ColumnUid>,
        name: String,
        synonyms: Vec<String>,
        ty: VariantTy,
        default: Option<Variant>,
        is_required: bool,
        is_used: bool,
    ) -> ColumnUid {
        let col_uid = if let Some(col_uid) = col_uid {
            col_uid
        } else {
            let next = self
                .columns
                .keys()
                .map(|col_uid| col_uid.0)
                .max()
                .map(|max| max + 1)
                .unwrap_or(0);
            ColumnUid(next)
        };
        let backend_column = BackendColumn {
            name,
            synonyms,
            ty: format!("{ty}"),
            is_sortable: true,
            is_required,
            is_used,
        };
        let variant_column = VariantColumn { ty, default };
        self.columns
            .insert(col_uid, (backend_column, variant_column));
        self.one_shot_flags.columns_reset = true;
        col_uid
    }

    pub fn get(&self, coord: CellCoord) -> Option<&Variant> {
        self.cell_data.get(&coord)
    }

    pub fn clear_mapping_choices(&mut self) {
        self.column_mapping_choices.clear();
    }

    pub fn set_mapping_choices<S: AsRef<str>>(&mut self, choices: impl Iterator<Item = S>) {
        self.column_mapping_choices = choices.map(|s| s.as_ref().to_string()).collect();
    }

    pub fn push_mapping_choices<S: AsRef<str>>(&mut self, choices: impl Iterator<Item = S>) {
        self.column_mapping_choices
            .extend(choices.map(|s| s.as_ref().to_string()));
    }
}

impl TableBackend for VariantBackend {
    fn clear(&mut self) {
        self.cell_data.clear();
        self.row_order.clear();
        self.one_shot_flags.row_set_updated = true;
        self.next_row_uid = RowUid(0);
    }

    fn persistent_flags(&self) -> &PersistentFlags {
        &self.persistent_flags
    }

    fn one_shot_flags(&self) -> &OneShotFlags {
        &self.one_shot_flags_delay
    }

    fn one_shot_flags_internal(&self) -> &OneShotFlags {
        &self.one_shot_flags
    }

    fn one_shot_flags_archive(&mut self) {
        self.one_shot_flags_delay = self.one_shot_flags;
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

    fn commit_cell_edit(&mut self, coord: CellCoord) {
        if let Some((last_edited_coord, value)) = self.cell_edit.take() {
            if last_edited_coord == coord {
                self.cell_data.insert(coord, value);
            }
        }
    }

    fn column_mapping_choices(&self) -> &[String] {
        &self.column_mapping_choices
    }
}

impl TableFrontend for VariantBackend {
    fn show_cell_view(&self, coord: CellCoord, ui: &mut Ui, _id: Id) {
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

    fn show_cell_editor(&mut self, coord: CellCoord, ui: &mut Ui, id: Id) -> Option<Response> {
        const INT_DRAG_SPEED: f32 = 0.1;

        let cell_ty = self
            .columns
            .get(&coord.col_uid)
            .map(|(_, c)| c.ty)
            .unwrap_or(VariantTy::Str);

        let mut is_first_pass = false;
        let mut value = if let Some((prev_coord, value)) = self.cell_edit.take() {
            if prev_coord == coord {
                value
            } else {
                is_first_pass = true;
                self.cell_data
                    .get(&coord)
                    .cloned()
                    .unwrap_or(Variant::default_of(cell_ty))
            }
        } else {
            is_first_pass = true;
            self.cell_data
                .get(&coord)
                .cloned()
                .unwrap_or(Variant::default_of(cell_ty))
        };
        let resp = match &mut value {
            Variant::Bool(v) => Some(ui.checkbox(v, "")),
            Variant::Enum {
                enum_uid,
                discriminant: discriminant_edit,
            } => {
                let resp = ComboBox::from_id_salt(id.with("_egui_tabular_enum_edit"))
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
        if is_first_pass {
            if let Some(resp) = &resp {
                resp.request_focus();
            }
        }
        self.cell_edit = Some((coord, value));
        resp
    }
}
