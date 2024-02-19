use std::rc::Rc;

use egui::{Color32, TextFormat, Ui};
use rvariant::Variant;

use crate::cell::CellCoord;

use super::TableView;

#[derive(Clone, PartialEq)]
pub enum Lint {
    /// Highlight string range
    HighlightRange,
    /// Highlight one of the items of StrList or other array-like Variants
    HighlightIndex {
        idx: usize,
        text_format: TextFormat,
    },
    AddButton,
    AddIcon {
        color: Color32,
        icon: &'static str,
    },
}

// TODO: column ty conversion dropdown
// TODO: kind change changes cell holes somehow
#[derive(Default)]
pub struct Settings {
    pub skippable_rows: bool,
    pub skippable_columns: bool,
    pub editable_column_names: bool,

    /// Whether cells can be edited and rows added / removed
    pub editable_cells: bool,
    /// Whether column types / names can be changed
    pub editable_columns: bool,
    pub commit_on_edit: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PersistentSettings {
    pub(crate) loose_column_order: Vec<String>,
    pub(crate) row_height: f32,
    pub(crate) show_column_types: bool,
}

impl Default for PersistentSettings {
    fn default() -> Self {
        PersistentSettings {
            loose_column_order: vec![],
            row_height: 24.0,
            show_column_types: true,
        }
    }
}

pub type CustomUiFn = fn(
    row_uid: u32,
    cell_value: &Variant,
    state: &mut Variant,
    ui: &mut Ui,
) -> Option<CustomUiResponse>;
pub type CustomToolUiFn = fn(row_uid: u32, &mut Variant, &mut Ui) -> Option<CustomUiResponse>;

// TODO: Switch to numbers + consts instead?
#[derive(Clone, Debug)]
pub enum CustomUiResponse {
    UserEventI32(u32, i32),
    UserEventU32(u32, u32),
    UserEventString(u32, String),
}

pub type CustomEditUiFn =
    fn(u32, &mut Variant, &mut Variant, &mut Ui) -> Option<CustomEditUiResponse>;

pub enum CustomEditUiResponse {
    UpdateCell(Variant),
    CustomUiResponse(CustomUiResponse),
}

impl TableView {
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    pub fn custom_ui_state(&self, uid_coord: CellCoord) -> Option<&Variant> {
        self.state.custom_ui_state.get(&uid_coord)
    }

    pub fn custom_ui_state_for(
        &self,
        col_uid: u32,
        allow_empty: bool,
    ) -> Vec<(CellCoord, Variant)> {
        let mut values = vec![];
        for (coord, value) in &self.state.custom_ui_state {
            if coord.1 != col_uid {
                continue;
            }
            if allow_empty || !value.is_empty() {
                values.push((*coord, value.clone()));
            }
        }
        values
    }

    pub fn set_custom_ui_state(&mut self, uid_coord: CellCoord, value: Variant) {
        self.state.custom_ui_state.insert(uid_coord, value);
    }

    pub fn clear_custom_ui_state_for(&mut self, col_uid: u32) {
        for (coord, value) in self.state.custom_ui_state.iter_mut() {
            if coord.1 == col_uid {
                *value = Variant::Empty;
            }
        }
    }

    pub fn take_custom_ui_response(&mut self) -> Option<(CellCoord, CustomUiResponse)> {
        // for (coord, response) in self.state.custom_ui_responses.drain() {
        //     f(coord, response);
        // }
        self.state.custom_ui_response.take()
    }

    pub fn add_tool_ui(&mut self, tool_ui: CustomToolUiFn) {
        self.tool_ui = Some(tool_ui);
    }

    pub fn take_tool_ui_response(&mut self) -> Option<(u32, CustomUiResponse)> {
        self.state.tool_ui_response.take()
    }

    pub fn add_cell_lint(&mut self, coord: CellCoord, lint: Lint) {
        let lints = &mut self.state.cell_metadata.entry(coord).or_default().lints;
        if !lints.contains(&lint) {
            lints.push(lint);
        }
    }

    pub fn add_cell_tooltip(&mut self, coord: CellCoord, tooltip: Rc<String>) {
        self.state
            .cell_metadata
            .entry(coord)
            .or_default()
            .tooltips
            .push(tooltip);
    }

    pub fn set_cell_text_format(&mut self, coord: CellCoord, format: TextFormat) {
        self.state
            .cell_metadata
            .entry(coord)
            .or_default()
            .text_format = Some(format);
    }

    pub fn clear_cell_lints(&mut self, coord: CellCoord) {
        self.state.cell_metadata.entry(coord).and_modify(|m| {
            m.tooltips.clear();
            m.lints.clear();
        });
    }

    pub fn load_state(&mut self, state: PersistentSettings) {
        self.persistent_settings = state;
    }

    pub fn save_state(&self) -> PersistentSettings {
        PersistentSettings {
            loose_column_order: self.state.columns.iter().map(|c| c.name.clone()).collect(),
            ..self.persistent_settings
        }
    }

    pub fn set_custom_ui(&mut self, col_id: u32, ui: CustomUiFn) {
        self.custom_ui.insert(col_id, ui);
    }

    pub fn set_custom_edit_ui(&mut self, col_id: u32, ui: CustomEditUiFn) {
        self.custom_edit_ui.insert(col_id, ui);
    }

    pub fn set_recognized(&mut self, col_id: u32, recognized: bool) {
        if let Some(column) = self.state.columns.iter_mut().find(|c| c.col_uid == col_id) {
            column.recognized = recognized;
        }
    }

    pub fn scroll_to_row(&mut self, monotonic_row_idx: u32) {
        self.state.scroll_to_row = Some(monotonic_row_idx);
    }

    pub fn row_disabled(&self) -> Option<u32> {
        self.state.disabled_row
    }

    pub fn row_enabled(&self) -> Option<u32> {
        self.state.enabled_row
    }

    pub fn col_disabled(&self) -> Option<u32> {
        self.state.disabled_col
    }

    pub fn col_enabled(&self) -> Option<u32> {
        self.state.enabled_col
    }

    pub fn is_row_skipped(&self, row_uid: u32) -> bool {
        self.state.rows_skip.get(&row_uid).cloned().unwrap_or(false)
    }

    pub fn skip_row(&mut self, row_uid: u32, skip: bool) {
        self.state.rows_skip.insert(row_uid, skip);
    }
}
