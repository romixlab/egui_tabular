use crate::backend::{
    BackendColumn, CellCoord, ColumnUid, OneShotFlags, PersistentFlags, RowUid, TableBackend,
    VisualRowIdx,
};
use egui::{Response, Ui};
use rvariant::{Variant, VariantTy};
use std::collections::HashMap;

pub struct VariantBackend {
    cell_data: HashMap<CellCoord, Variant>,
    row_order: Vec<RowUid>,
    next_row_uid: RowUid,
    columns: HashMap<ColumnUid, (BackendColumn, VariantColumn)>,
    persistent_flags: PersistentFlags,
    one_shot_flags: OneShotFlags,
}

struct VariantColumn {
    _ty: VariantTy,
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
                        ty: format!("{ty}"),
                        is_sortable: true,
                    };
                    let variant_column = VariantColumn { _ty: ty, default };
                    (col_uid, (backend_column, variant_column))
                })
                .collect(),
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
        ty: VariantTy,
        default: Option<Variant>,
    ) {
        let backend_column = BackendColumn {
            name,
            ty: format!("{ty}"),
            is_sortable: true,
        };
        let variant_column = VariantColumn { _ty: ty, default };
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

    fn show_cell_editor(&mut self, _coord: CellCoord, _ui: &mut Ui) -> Option<Response> {
        todo!()
    }
}
