use egui::{Color32, Id, Ui};
use tabular_core::{CellCoord, ColumnUid};

pub trait TableFrontend {
    fn show_cell_view(&mut self, coord: CellCoord, ui: &mut Ui, id: Id);
    fn show_cell_editor(
        &mut self,
        coord: CellCoord,
        ui: &mut Ui,
        id: Id,
    ) -> Option<egui::Response> {
        let (_, _, _) = (coord, ui, id);
        None
    }
    fn cancel_edit(&mut self) {}

    fn custom_column_ui(&mut self, _col_uid: ColumnUid, _ui: &mut Ui, _id: Id) {}

    /// Override default cell color
    fn cell_color(&self, _coord: CellCoord) -> Option<Color32> {
        None
    }

    /// Show tooltip on cell hover
    fn cell_tooltips(&self, _coord: CellCoord) -> Vec<&str> {
        vec![]
    }

    /// Show colored corner in a cell
    fn cell_corner(&self, _coord: CellCoord) -> Option<Color32> {
        None
    }
}
