use crate::frontend::TableFrontend;
use egui::{Button, Ui, UiKind};
use tabular_core::backend::TableBackend;
use tabular_core::RowUid;

/// Shown when right-clicked on tool column row
pub(super) fn tool_column_row_menu_ui<T: TableFrontend + TableBackend>(
    ui: &mut Ui,
    table: &mut T,
    row_uid: RowUid,
) {
    append_row(ui, table);
    if table.persistent_flags().are_rows_skippable {
        let mut is_row_skipped = table.is_row_skipped(row_uid);
        if ui.checkbox(&mut is_row_skipped, "Skip row").changed() {
            table.skip_row(row_uid, is_row_skipped);
            ui.close_kind(UiKind::Menu);
        }
    }
}

/// Shown when right-clicked on tool column header (table icon)
pub(super) fn tool_column_header_menu_ui<T: TableBackend>(ui: &mut Ui, table: &mut T) {
    if table.persistent_flags().is_get_variant_supported {
        if ui.button("Export CSV").clicked() {
            crate::util::export_csv(table);
            ui.close_kind(UiKind::Menu);
        }
    }
    append_row(ui, table);
    if table.is_clearable() {
        ui.add_space(24.0);
        // TODO: Ask before clearing
        if ui.button("Clear").clicked() {
            table.clear();
        }
    }
}

fn append_row<T: TableBackend>(ui: &mut Ui, table: &mut T) {
    let enabled = !table.persistent_flags().is_read_only;
    let r = ui.add_enabled(enabled, Button::new("Append row (N)"));
    if r.clicked() {
        table.create_row([]);
    }
    r.on_disabled_hover_text("This table is read-only");
}
