use crate::frontend::TableFrontend;
use egui::{Ui, UiKind};
use tabular_core::backend::TableBackend;
use tabular_core::RowUid;

pub(super) fn tool_column_context_menu_ui<T: TableFrontend + TableBackend>(
    ui: &mut Ui,
    table: &mut T,
    row_uid: RowUid,
) {
    if ui.button("Append row").clicked() {
        table.create_row([]);
        ui.close_kind(UiKind::Menu);
    }
    if table.are_rows_skippable() {
        let mut is_row_skipped = table.is_row_skipped(row_uid);
        if ui.checkbox(&mut is_row_skipped, "Skip row").changed() {
            table.skip_row(row_uid, is_row_skipped);
            ui.close_kind(UiKind::Menu);
        }
    }
}
