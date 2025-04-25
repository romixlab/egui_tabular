use egui::{Color32, Id, Ui};
use egui_extras::Column as TableColumnConfig;
use tabular_core::{CellCoord, ColumnUid};

pub trait TableFrontend {
    fn show_cell_view(&self, coord: CellCoord, ui: &mut Ui, id: Id);
    fn show_cell_editor(&mut self, coord: CellCoord, ui: &mut Ui, id: Id)
        -> Option<egui::Response>;

    /// Returns the rendering configuration for the column.
    fn column_render_config(&mut self, col_uid: ColumnUid) -> TableColumnConfig {
        let _ = col_uid;
        TableColumnConfig::auto().resizable(true)
    }

    /// Use this to check if given cell is going to take any dropped payload / use as drag
    /// source.
    fn on_cell_view_response(&mut self, coord: CellCoord, resp: &egui::Response) -> Option<()> {
        let _ = (coord, resp);
        None
    }

    fn custom_column_ui(&mut self, _col_uid: ColumnUid, _ui: &mut Ui, _id: Id) {}

    /// Override default cell color
    fn cell_color(&self, _coord: CellCoord) -> Option<Color32> {
        None
    }

    /// Show tooltip on cell hover
    fn cell_tooltip(&self, _coord: CellCoord) -> Option<&str> {
        None
    }
}
