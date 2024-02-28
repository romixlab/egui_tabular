use egui::{Color32, DragValue, TextEdit, Ui, Widget};
use log::warn;
use rvariant::Variant;

use crate::table_view::UiColumn;

const INT_DRAG_SPEED: f32 = 0.1;

pub(super) fn show_cell_editor(
    _row_uid: u32,
    cell_when_editing: &mut Variant,
    first_pass: bool,
    ui_column: &UiColumn,
    ui: &mut Ui,
) -> Option<Variant> {
    match cell_when_editing {
        Variant::Enum {
            enum_uid,
            discriminant: discriminant_edit,
        } => {
            let changed = egui::ComboBox::from_id_source("enum_edit")
                .selected_text(
                    rvariant::uid_to_variant_name(*enum_uid, *discriminant_edit).expect(""),
                )
                .width(ui_column.width)
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    for (d, v) in rvariant::variant_names(*enum_uid).expect("") {
                        changed |= ui.selectable_value(discriminant_edit, *d, v).changed();
                    }
                    changed
                })
                .inner;
            if changed == Some(true) {
                let discriminant = *discriminant_edit;
                Some(Variant::Enum {
                    enum_uid: *enum_uid,
                    discriminant,
                })
            } else {
                None
            }
        }
        Variant::Str(edit_text) => {
            let edit = if first_pass {
                let edit = TextEdit::singleline(edit_text)
                    .cursor_at_end(false)
                    .desired_width(f32::INFINITY)
                    .ui(ui);
                edit.request_focus();
                edit
            } else {
                TextEdit::singleline(edit_text)
                    .desired_width(f32::INFINITY)
                    .ui(ui)
            };
            if edit.lost_focus() {
                let converted = Variant::from_str(edit_text, ui_column.ty);
                Some(converted)
            } else {
                None
            }
        }
        Variant::U32(num) => {
            ui.horizontal(|ui| {
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
            .inner
        }
        Variant::U64(num) => {
            ui.horizontal(|ui| {
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
            .inner
        }
        v => {
            warn!("Editor is not implemented for {v}");
            Some(v.clone())
        }
    }
}

pub(super) fn add_and_select_missing_cell(is_edit_mode: bool, ui: &mut Ui) -> bool {
    let tooltip_text = if is_edit_mode {
        if ui.button("Add").clicked() {
            return true;
        }
        "No data, click to add."
    } else {
        ui.colored_label(Color32::LIGHT_YELLOW, "No data");
        "No data, enable edit mode and add."
    };
    if ui.ui_contains_pointer() {
        egui::show_tooltip(ui.ctx(), "table_view_missing_data_tooltip".into(), |ui| {
            ui.label(tooltip_text);
        });
    }
    false
}
