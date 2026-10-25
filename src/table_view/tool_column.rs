use crate::TableEvent;
use crate::commands::TableCommand;
use egui::containers::menu::{MenuConfig, SubMenuButton};
use egui::{Button, PopupCloseBehavior, Ui, UiKind};
use std::collections::{BTreeSet, HashMap};
use tabular_core::{Capabilities, ColumnInfo, ColumnUid, RowPosition, RowUid, TableModel};

/// Shown when right-clicked on tool column row.
pub(super) fn tool_column_row_menu_ui<M: TableModel>(
    ui: &mut Ui,
    model: &M,
    caps: Capabilities,
    row_uid: RowUid,
    commands: &mut Vec<TableCommand>,
) {
    append_row(ui, caps, commands);
    if caps.skip_rows {
        let mut is_row_skipped = model.is_row_skipped(row_uid);
        if ui.checkbox(&mut is_row_skipped, "Skip row").changed() {
            commands.push(TableCommand::SkipRows {
                rows: vec![row_uid],
                skipped: is_row_skipped,
            });
            ui.close_kind(UiKind::Menu);
        }
    }
}

/// Shown when right-clicked on tool column header (table icon). Returns true if "Export CSV"
/// was clicked. On wasm there is no save dialog (EXPORT-4), so no "Export CSV" either.
pub(super) fn tool_column_header_menu_ui(
    ui: &mut Ui,
    caps: Capabilities,
    all_columns: &[ColumnUid],
    infos: &HashMap<ColumnUid, ColumnInfo>,
    hidden_columns: &mut BTreeSet<ColumnUid>,
    events: &mut Vec<TableEvent>,
    commands: &mut Vec<TableCommand>,
) -> bool {
    let export = cfg!(not(target_arch = "wasm32")) && ui.button("Export CSV").clicked();
    if export {
        ui.close_kind(UiKind::Menu);
    }
    append_row(ui, caps, commands);
    column_chooser(ui, all_columns, infos, hidden_columns, events);
    if caps.clear {
        ui.add_space(24.0);
        // TODO: Ask before clearing (VIEW-6)
        if ui.button("Clear").clicked() {
            commands.push(TableCommand::Clear);
        }
    }
    export
}

fn append_row(ui: &mut Ui, caps: Capabilities, commands: &mut Vec<TableCommand>) {
    let r = ui.add_enabled(caps.create_rows, Button::new("Append row (N)"));
    if r.clicked() {
        commands.push(TableCommand::CreateRows {
            at: RowPosition::Append,
            count: 1,
        });
    }
    r.on_disabled_hover_text("This table is read-only");
}

/// Checkbox list of every column in the user's order; unchecked columns are hidden.
fn column_chooser(
    ui: &mut Ui,
    all_columns: &[ColumnUid],
    infos: &HashMap<ColumnUid, ColumnInfo>,
    hidden_columns: &mut BTreeSet<ColumnUid>,
    events: &mut Vec<TableEvent>,
) {
    // Stays open while columns are toggled.
    let config = MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside);
    SubMenuButton::new("Columns").config(config).ui(ui, |ui| {
        for col in all_columns {
            let name = infos.get(col).map_or("", |info| info.name.as_str());
            let name = if name.is_empty() { "No name" } else { name };
            let mut shown = !hidden_columns.contains(col);
            if ui.checkbox(&mut shown, name).changed() {
                if shown {
                    hidden_columns.remove(col);
                } else {
                    hidden_columns.insert(*col);
                }
                events.push(TableEvent::ConfigChanged);
            }
        }
    });
}
