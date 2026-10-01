use crate::commands::TableCommand;
use egui::{Button, Ui, UiKind};
use tabular_core::{Capabilities, RowPosition, RowUid, TableModel};

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
/// was clicked.
pub(super) fn tool_column_header_menu_ui(
    ui: &mut Ui,
    caps: Capabilities,
    commands: &mut Vec<TableCommand>,
) -> bool {
    let export = ui.button("Export CSV").clicked();
    if export {
        ui.close_kind(UiKind::Menu);
    }
    append_row(ui, caps, commands);
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
