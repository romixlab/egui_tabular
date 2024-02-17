use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Neg;

use egui::{
    Align, Color32, Event, Frame, Id, Key, Label, Layout, RichText, ScrollArea, Sense, TextFormat,
    Ui, Vec2,
};
use egui_modal::Modal;
use itertools::Itertools;
use log::{debug, warn};

use crate::backend::TableBackend;
use crate::cell::{CellCoord, CellKind, StaticCellKind};
use crate::filter::{FilterOperation, RowFilter, VariantFilter};
use rvariant::{Variant, VariantTy};

mod cell_edit;
mod cell_view;
#[allow(dead_code)]
mod table;
use table::{Column, SelectedRange, SelectionEvent, TableBody};

use self::widgets::flag_label;
#[allow(dead_code)]
mod layout;
#[allow(dead_code)]
mod sizing;
mod util;
mod widgets;

pub struct TableView {
    required_columns: Vec<RequiredColumn>,
    state: State,
    id: String,

    settings: Settings,
    persistent_settings: PersistentSettings,
    tool_ui: Option<CustomToolUiFn>,
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
struct PersistentSettings {
    loose_column_order: Vec<String>,
    row_height: f32,
    show_column_types: bool,
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

#[derive(Clone)]
pub struct RequiredColumn {
    pub name: String,
    pub synonyms: Vec<String>,
    pub ty: VariantTy,
    pub ty_locked: bool,
    pub default_value: Option<Variant>,
    custom_ui: Option<CustomUiFn>,
    custom_edit_ui: Option<CustomEditUiFn>,
    user_map: Option<u32>,
}

impl RequiredColumn {
    pub fn new(
        name: impl AsRef<str>,
        synonyms: impl IntoIterator<Item = &'static str>,
        ty: VariantTy,
        ty_locked: bool,
        default_value: Option<Variant>,
        user_map: Option<u32>,
        custom_ui: Option<CustomUiFn>,
        custom_edit_ui: Option<CustomEditUiFn>,
    ) -> Self {
        let mut synonyms_sum = vec![name.as_ref().to_lowercase()];
        synonyms_sum.extend(synonyms.into_iter().map(|s| s.to_lowercase()));
        RequiredColumn {
            name: name.as_ref().to_string(),
            synonyms: synonyms_sum,
            ty,
            ty_locked,
            default_value,
            user_map,
            custom_ui,
            custom_edit_ui,
        }
    }
}

#[derive(Default)]
struct State {
    columns: Vec<UiColumn>,
    cell_metadata: HashMap<CellCoord, CellMetadata>,
    rows_skip: HashMap<u32, bool>,

    last_pressed: Option<(usize, usize)>,
    cell_when_editing: Option<Variant>,
    save_cell_changes_and_deselect: bool, // TODO: improve and keep coords of currently editing cell, instead of deferring to ui loop to commit changes?
    discard_cell_changes_and_deselect: bool,
    // dirty_cell: Option<(usize, usize)>,
    selected_range: Option<SelectedRange>,

    disabled_row: Option<usize>,
    enabled_row: Option<usize>,
    disabled_col: Option<usize>,
    enabled_col: Option<usize>,

    custom_ui_response: Option<(CellCoord, CustomUiResponse)>,
    custom_ui_state: HashMap<CellCoord, Variant>,

    tool_ui_state: HashMap<i32, Variant>,
    tool_ui_response: Option<(u32, CustomUiResponse)>,

    filter_value_text: String,
    scroll_to_row: Option<usize>,

    about_to_paste_rows: Vec<Vec<String>>,
    pasting_block_with_holes: bool,
    pasting_block_width: usize,
    create_rows_on_paste: bool,
    fill_with_same_on_paste: bool,
    create_adhoc_cols_on_paste: bool,

    clear_requested: bool,
}

#[derive(Default)]
pub struct CellMetadata {
    pub lints: Vec<Lint>,
    pub tooltip: String,
}

impl CellMetadata {
    pub fn from_lint(lint: Lint) -> Self {
        Self {
            lints: vec![lint],
            tooltip: String::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Lint {
    Color,
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

#[derive(Clone, Debug)]
struct UiColumn {
    name: String,
    ty: VariantTy,
    ty_locked: bool,
    default_value: Option<Variant>,
    // At which index this column is actually provided.
    col_uid: u32,
    skip: bool,
    kind: CellKind,
    // Header is correctly recognized by upstream code, show checkmark to reassure user
    recognized: bool,
    width: f32,
    user_map: Option<u32>,
    custom_ui: Option<CustomUiFn>,
    custom_edit_ui: Option<CustomEditUiFn>,
    is_tool: bool, // TODO: Refactor into enum
}

impl UiColumn {
    fn show_kind(&self, ui: &mut Ui) {
        let (icon, tooltip) = match self.kind {
            CellKind::Static(kind) => match kind {
                StaticCellKind::Plain => (egui_phosphor::regular::CPU, StaticCellKind::PLAIN_DOC),
                StaticCellKind::CausesSideEffects => (
                    egui_phosphor::regular::FLOW_ARROW,
                    StaticCellKind::CAUSES_SIDE_EFFECTS_DOC,
                ),
                StaticCellKind::AutoGenerated => (
                    egui_phosphor::regular::FUNCTION,
                    StaticCellKind::AUTO_GENERATED_DOC,
                ),
            },
            CellKind::Global => (egui_phosphor::regular::HARD_DRIVES, CellKind::GLOBAL_DOC),
            CellKind::Adhoc => (egui_phosphor::regular::CIRCLE, CellKind::ADHOC_DOC),
        };
        ui.label(RichText::new(icon).size(16.0))
            .on_hover_text(tooltip);
        if self.ty_locked {
            ui.label(RichText::new(egui_phosphor::regular::LOCK_SIMPLE).size(14.0))
                .on_hover_text("VariantType is locked and cannot be changed");
        }
        ui.add(Label::new(format!("{}", self.ty)).truncate(true));
    }

    fn is_read_only(&self) -> bool {
        self.kind == CellKind::Static(StaticCellKind::AutoGenerated)
    }
}

impl Hash for UiColumn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.ty.hash(state);
    }
}

#[derive(Copy, Clone)]
struct SelectionKeyNavigation {
    shift: bool,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl SelectionKeyNavigation {
    pub fn from_ui(ui: &Ui) -> Self {
        let (shift, up, down, left, right) = ui.input(|i| {
            (
                i.modifiers.shift,
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
            )
        });
        Self {
            shift,
            up,
            down,
            left,
            right,
        }
    }

    pub fn any_moves(&self) -> bool {
        self.up || self.down || self.left || self.right
    }
}

impl TableView {
    pub fn new<S: AsRef<str>>(
        required_columns: impl IntoIterator<Item = RequiredColumn>,
        id: S,
    ) -> Self {
        TableView {
            required_columns: required_columns.into_iter().collect(),
            state: State::default(),
            id: id.as_ref().to_string(),

            settings: Settings::default(),
            persistent_settings: PersistentSettings::default(),
            tool_ui: None,
        }
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    pub fn show(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        // if !data.flags().column_info_present {
        //     ui.label("Empty table");
        //     return;
        // }
        if data.one_shot_flags().column_info_updated {
            self.map_columns(data);
        }
        if data.one_shot_flags().cleared {
            self.state.cell_metadata.clear();
        }
        data.poll();
        if self.state.columns.is_empty() {
            ui.label("TableView: Empty table");
            return;
        }

        let mut modal = Modal::new(ui.ctx(), "table_view_modal").with_close_on_outside_click(false);
        let is_empty_table = data.row_count() == 0;
        let window_contains_pointer = ui.rect_contains_pointer(ui.max_rect());
        self.top_strip_ui(
            data,
            ui,
            &mut modal,
            is_empty_table,
            window_contains_pointer,
        );
        ui.separator();
        // column selector and rearrange
        // column filters and sort

        let key_navigation = SelectionKeyNavigation::from_ui(ui);
        self.handle_key_input(data, ui);

        if window_contains_pointer {
            self.handle_paste(ui, data, &mut modal);
        }
        self.handle_paste_continue(data, &mut modal);
        self.handle_clear_request(data, &mut modal);
        modal.show_dialog();

        ScrollArea::horizontal().drag_to_scroll(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                let dragged = egui_dnd::dnd(ui, ("table_view_dnd", self.id.as_str())).show_custom_vec(&mut self.state.columns, |ui, items, item_iter| {
                    items.iter_mut().enumerate().for_each(|(idx, ui_column)| {
                        let size = Vec2::new(ui_column.width, 16.0);
                        let frame_padding = 0.0;
                        let size = size + Vec2::splat(frame_padding) * 2.0;
                        item_iter.next(ui, Id::new((&self.id, &ui_column.name)), idx, true, |ui, item_handle| {
                            item_handle.ui_sized(ui, size, |ui, handle, _state| {
                                Frame::none()
                                    .inner_margin(0.0)
                                    .fill(ui.visuals().panel_fill)
                                    // .rounding(4.0)
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            let required_column = false;
                                            if self.settings.editable_column_names && !required_column {
                                                ui.text_edit_singleline(&mut ui_column.name);
                                            } else {
                                                ui.horizontal(|ui| {
                                                    handle.ui(ui, |ui| {
                                                        ui.add(Label::new(RichText::new(&ui_column.name).strong().monospace()).truncate(true).selectable(false));
                                                        // ui.monospace(&ui_column.name);
                                                    });
                                                    ui.menu_button(egui_phosphor::regular::FUNNEL, |ui| {
                                                        Self::value_filter_ui(data, &mut self.state.filter_value_text, ui_column, ui);
                                                    });
                                                });
                                            }
                                            ui.horizontal(|ui| {
                                                ui_column.show_kind(ui);
                                                if ui_column.recognized {
                                                    ui.colored_label(Color32::LIGHT_GREEN, "✔").on_hover_text("Column is recognized and picked up for additional checks or processing.");
                                                }
                                                if self.settings.skippable_columns && !required_column {
                                                    if ui.checkbox(&mut ui_column.skip, "Skip").changed() && ui_column.skip {
                                                        self.state.disabled_col = Some(idx);
                                                    } else {
                                                        self.state.enabled_col = Some(idx);
                                                    }
                                                }
                                            });
                                            ui.separator();
                                        });
                                        // let rect = ui.max_rect();
                                        // ui.painter().rect_filled(rect, Rounding::none(), ui.visuals().extreme_bg_color);
                                        // handle.ui(ui, |ui| {
                                        //     let rect = ui.max_rect();
                                        //     ui.painter().rect_filled(rect, Rounding::none(), ui.visuals().extreme_bg_color);
                                        //     ui.label(&column.name);
                                        // });
                                    });
                            })
                        });
                    });
                }).is_drag_finished();
                if dragged {
                    debug!("Drag finished");
                }
            });

            let table_builder = table::TableBuilder::new(ui)
                .resizable(true)
                .striped(true)
                .select_range(self.state.selected_range);
            let mut table_builder = if let Some(idx) = self.state.scroll_to_row.take() {
                table_builder.scroll_to_row(idx, Some(Align::Center))
            } else {
                table_builder
            };
            for h in &self.state.columns {
                table_builder.push_column(Column::initial(h.name.len() as f32 * 8.0 + 54.0).at_least(36.0).clip(true));
            }
            let mut table_builder = table_builder
                .header(24.0, |mut row| {
                    row.col(|_ui| {});
                });
            // columns might have been swapped by dragging around, so swap their widths as well
            table_builder.set_column_widths(self.state.columns.iter().map(|c| c.width));
            let column_widths = table_builder.body(|body| self.add_table_rows(body, data, window_contains_pointer, key_navigation));
            // columns might have been resized, update our state
            for (ui_column, new_width) in self.state.columns.iter_mut().zip(column_widths.iter()) {
                ui_column.width = *new_width;
            }
            if is_empty_table {
                ui.label("Empty table");
            }
        });
    }

    fn top_strip_ui(
        &mut self,
        data: &mut impl TableBackend,
        ui: &mut Ui,
        modal: &mut Modal,
        is_empty_table: bool,
        window_contains_pointer: bool,
    ) {
        ui.horizontal(|ui| {
            self.show_filters_in_use(data, ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let show_edit_buttons =
                    self.settings.editable_cells && !data.persistent_flags().is_read_only;
                let add_row_clicked = show_edit_buttons && ui.button("Add row").clicked();
                if show_edit_buttons && !is_empty_table && ui.button("Clear").clicked() {
                    self.state.clear_requested = true;
                    modal.open();
                }
                let add_row_shortcut_pressed = self.state.cell_when_editing.is_none()
                    && window_contains_pointer
                    && ui.input(|i| i.key_pressed(Key::R) && i.modifiers.shift);
                if add_row_clicked || add_row_shortcut_pressed {
                    data.create_row(
                        self.state
                            .columns
                            .iter()
                            .filter_map(|c| c.default_value.clone().map(|d| (c.col_uid, d)))
                            .collect(),
                    );
                }
                if self.settings.editable_cells {
                    ui.label(egui_phosphor::regular::PENCIL)
                        .on_hover_text("Edit mode");
                } else {
                    ui.label(egui_phosphor::regular::EYE)
                        .on_hover_text("View only mode, enable edit mode if needed");
                }
                if is_empty_table {
                    ui.label("Empty table");
                    if data.persistent_flags().is_read_only {
                        ui.label("R/O").on_hover_text(
                            "Data source for this table is read only, cannot modify anything",
                        );
                    }
                }
            });
        });
    }

    fn handle_key_input(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            if self.state.cell_when_editing.is_some() {
                self.state.save_cell_changes_and_deselect = true;
            } else {
                self.state.selected_range = None;
                self.state.last_pressed = None;
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            if self.state.cell_when_editing.is_some() {
                self.state.discard_cell_changes_and_deselect = true;
            } else {
                self.state.selected_range = None;
                self.state.last_pressed = None;
            }
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::C)) {
            if let Some(selected) = self.state.selected_range {
                let mut text = String::new();
                for mono_row_idx in selected.row_start..=selected.row_end {
                    let Some(row_uid) = data.row_uid(mono_row_idx) else {
                        continue;
                    };
                    for mono_col_idx in selected.col_start..=selected.col_end {
                        let Some(col_uid) = self.state.columns.get(mono_col_idx).map(|u| u.col_uid)
                        else {
                            continue;
                        };
                        let cell_value = data
                            .cell(CellCoord(row_uid, col_uid))
                            .map(|v| v.value.to_string())
                            .unwrap_or_default();
                        text += cell_value.as_str();
                        if mono_col_idx != selected.col_end {
                            text += "\t";
                        }
                    }
                    if mono_row_idx != selected.row_end {
                        text += "\n";
                    }
                }
                if !text.is_empty() {
                    ui.output_mut(|i| i.copied_text = text);
                }
            }
        }
    }

    fn handle_paste(&mut self, ui: &mut Ui, data: &mut impl TableBackend, modal: &mut Modal) {
        let paste = ui.input(|i| {
            i.events
                .iter()
                .find(|e| matches!(e, Event::Paste(_)))
                .cloned()
        });
        let Some(Event::Paste(text)) = paste else {
            return;
        };
        let mut rows = vec![];
        for row in text.split('\n') {
            let mut cols = vec![];
            for col in row.split('\t') {
                cols.push(col.trim().to_string());
            }
            rows.push(cols);
        }
        if rows.is_empty() {
            return;
        }
        self.state.pasting_block_width = rows[0].len();
        let is_equal_lengths =
            rows.iter()
                .map(|c| c.len())
                .tuple_windows()
                .fold(0i32, |acc, (l1, l2)| {
                    self.state.pasting_block_width = l1.max(l2);
                    acc + l1 as i32 - l2 as i32
                })
                == 0;
        self.state.pasting_block_with_holes = !is_equal_lengths;

        if let Some(selected_range) = &self.state.selected_range {
            let selection_is_exact = rows.len() == selected_range.height()
                && self.state.pasting_block_width == selected_range.width()
                && is_equal_lengths;
            self.state.about_to_paste_rows = rows;
            if selection_is_exact {
                self.paste_block(data);
            } else {
                // ask user what to do in handle_paste_continue
                self.state.create_rows_on_paste = false;
                self.state.fill_with_same_on_paste = false;
                self.state.create_adhoc_cols_on_paste = false;
                modal.open();
            }
        } else {
            warn!("Refusing to paste without selection");
        }
    }

    fn handle_paste_continue(&mut self, data: &mut impl TableBackend, modal: &mut Modal) {
        if self.state.about_to_paste_rows.is_empty() {
            return;
        }
        let rows = self.state.about_to_paste_rows.len();
        let cols = self.state.pasting_block_width;
        let Some(selected_range) = self.state.selected_range else {
            return;
        };

        modal.show(|ui| {
            modal.title(ui, "Paste");
            ui.horizontal(|ui| {
                modal.icon(ui, egui_modal::Icon::Warning);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    let with_holes = if self.state.pasting_block_with_holes {
                        " (with holes)"
                    } else {
                        ""
                    };
                    ui.label(format!(
                        "You are about to paste {}x{} block{} into {}x{} selection",
                        rows,
                        cols,
                        with_holes,
                        selected_range.height(),
                        selected_range.width(),
                    ));
                    if selected_range.height() < rows {
                        ui.checkbox(&mut self.state.create_rows_on_paste, "Create more rows");
                    }
                    if selected_range.height() > rows || selected_range.width() > cols {
                        ui.checkbox(
                            &mut self.state.fill_with_same_on_paste,
                            "Fill with repeated values",
                        );
                    }
                    if selected_range.width() < cols {
                        ui.checkbox(
                            &mut self.state.create_adhoc_cols_on_paste,
                            "Create adhoc columns (not implemented yet)",
                        );
                    }
                });
            });
            modal.buttons(ui, |ui| {
                if modal.button(ui, "Close").clicked() {
                    self.state.about_to_paste_rows.clear();
                }
                if modal.suggested_button(ui, "Paste").clicked() {
                    self.paste_block(data);
                }
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.state.about_to_paste_rows.clear();
                modal.close();
            }
            if ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.ctrl) {
                self.paste_block(data);
            }
        });
    }

    fn paste_block(&mut self, data: &mut impl TableBackend) {
        let Some(selected_range) = &self.state.selected_range else {
            return;
        };
        let mut row_ids: Vec<Option<u32>> = (0..selected_range.height())
            .map(|mono_row_idx| data.row_uid(mono_row_idx + selected_range.row_start))
            .collect();

        if self.state.create_rows_on_paste
            && self.state.about_to_paste_rows.len() > selected_range.height()
        {
            for _ in 0..self.state.about_to_paste_rows.len() - selected_range.height() {
                row_ids.push(data.create_row(HashMap::new()));
            }
        }

        let col_ids: Vec<Option<(u32, VariantTy)>> = (0..selected_range.width())
            .map(|mono_col_idx| {
                self.state
                    .columns
                    .get(mono_col_idx + selected_range.col_start)
                    .map(|col| (col.col_uid, col.ty))
            })
            .collect();

        // TODO: create adhoc columns on paste
        // if self.state.create_adhoc_cols_on_paste {
        //     for _ in 0..self.state.about_to_paste_rows[0].len() - selected_range.width() {
        //         col_ids.push(data.create_column());
        //     }
        // }
        let mut changed_coords = vec![];

        if self.state.fill_with_same_on_paste {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter().cycle())
            {
                let Some(row_id) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter().cycle()) {
                    let Some((col_id, ty)) = col_id_ty else {
                        continue;
                    };
                    let coord = CellCoord(row_id, *col_id);
                    changed_coords.push(coord);
                    data.modify_one(coord, Variant::from_str(cell, *ty));
                }
            }
        } else {
            for (row_id, row) in row_ids
                .into_iter()
                .zip(self.state.about_to_paste_rows.iter())
            {
                let Some(row_id) = row_id else { continue };
                for (col_id_ty, cell) in col_ids.iter().zip(row.iter()) {
                    let Some((col_id, ty)) = col_id_ty else {
                        continue;
                    };
                    let coord = CellCoord(row_id, *col_id);
                    changed_coords.push(coord);
                    data.modify_one(coord, Variant::from_str(cell, *ty));
                }
            }
        }

        data.one_shot_flags_mut().cells_updated = changed_coords;
        self.state.about_to_paste_rows.clear();
    }

    fn handle_clear_request(&mut self, data: &mut impl TableBackend, modal: &mut Modal) {
        if !self.state.clear_requested {
            return;
        }
        modal.show(|ui| {
            modal.title(ui, "Clear all data");
            ui.horizontal(|ui| {
                modal.icon(ui, egui_modal::Icon::Warning);
                ui.label("About to clear all table's data, are you sure?");
            });
            // modal.frame(ui, |ui| {
            //     modal.icon(ui, egui_modal::Icon::Warning);
            //     modal.body(ui, "About to clear all table's data, are you sure?");
            // });
            modal.buttons(ui, |ui| {
                if modal.caution_button(ui, "Clear").clicked() {
                    self.state.clear_requested = false;
                    data.clear();
                }
                if modal.suggested_button(ui, "Cancel").clicked() {
                    self.state.clear_requested = false;
                    modal.close();
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.state.clear_requested = false;
                    modal.close();
                }
            });
        });
    }

    fn value_filter_ui(
        data: &mut impl TableBackend,
        value_filter_text: &mut String,
        ui_column: &mut UiColumn,
        ui: &mut Ui,
    ) {
        if ui.text_edit_singleline(value_filter_text).lost_focus() {
            let Ok(value) = Variant::try_from_str(&value_filter_text, ui_column.ty) else {
                warn!("Convert to required VariantTy failed");
                return;
            };
            debug!("Applying value filter with value: {:?}", value);
            data.add_row_filter(
                RowFilter::ShowByVariant(VariantFilter {
                    col_uid: ui_column.col_uid,
                    op: FilterOperation::Contains,
                    value,
                }),
                false,
                format!("{} contains {}", ui_column.name, value_filter_text),
            );
            ui.close_menu();
        }
    }

    pub fn show_properties(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("TableView properties").size(14.0).strong());
        ui.horizontal(|ui| {
            ui.with_layout(Layout::left_to_right(Align::default()), |ui| {
                // ui.menu_button(
                //     format!("{} Table view options", egui_phosphor::regular::TABLE),
                //     |ui| {
                egui::Grid::new("table_view_settings_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Row height");
                        ui.add(egui::Slider::new(
                            &mut self.persistent_settings.row_height,
                            10.0..=500.0,
                        ));
                        ui.end_row();

                        ui.label("Edit mode");
                        ui.checkbox(&mut self.settings.editable_cells, "Enable");
                        ui.end_row();

                        ui.label("Commit on edit");
                        ui.checkbox(&mut self.settings.commit_on_edit, "Enable");
                        ui.end_row();

                        ui.label("Editable column names");
                        ui.checkbox(&mut self.settings.editable_column_names, "Enable");
                        ui.end_row();

                        // ui.label("Show tool column");
                        // ui.checkbox(
                        //     &mut self.persistent_settings.show_tool_column,
                        //     "Enable",
                        // );
                        // ui.end_row();

                        ui.label("Show column types");
                        ui.checkbox(&mut self.persistent_settings.show_column_types, "Enable");
                        ui.end_row();
                    });
                // },
                // );
            });
        });
    }

    fn show_filters_in_use(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        let mut filter_to_remove = None;
        for (idx, (filter, name)) in data.row_filters().iter().enumerate() {
            match filter {
                RowFilter::HideByUid(_) => {
                    ui.label(egui_phosphor::regular::MINUS_SQUARE);
                }
                RowFilter::ShowByUid(_) => {
                    ui.label(egui_phosphor::regular::PLUS_SQUARE);
                }
                RowFilter::ShowByVariant(_value_filter) => {
                    ui.label(egui_phosphor::regular::FUNNEL);
                }
            }
            ui.label("Rows")
                .on_hover_text(format!("Select rows by UID\n{name}"));
            if ui.button(egui_phosphor::regular::TRASH).clicked() {
                filter_to_remove = Some(idx);
            }
        }
        if let Some(idx) = filter_to_remove {
            data.remove_row_filter(idx);
        }
    }

    fn add_table_rows(
        &mut self,
        body: TableBody<'_>,
        data: &mut impl TableBackend,
        window_contains_pointer: bool,
        key_navigation: SelectionKeyNavigation,
    ) {
        let selection_event = body.rows(
            self.persistent_settings.row_height,
            data.row_count(),
            window_contains_pointer,
            |row_idx, mut ui_row| {
                let Some(row_uid) = data.row_uid(row_idx) else {
                    warn!("TableBackend: inconsistent row index mapping detected");
                    return;
                };
                let row_skip = self.state.rows_skip.entry(row_uid).or_default();
                for (monotonic_col_idx, ui_column) in self.state.columns.iter().enumerate() {
                    if ui_column.is_tool {
                        let hovered = ui_row.hovered();
                        ui_row.col(|ui| {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("#{row_idx} G{row_uid}"));
                                if self.settings.skippable_rows && (hovered || *row_skip) {
                                    let changed = ui.checkbox(row_skip, "Skip").changed();
                                    if *row_skip && changed {
                                        self.state.disabled_row = Some(row_idx);
                                    } else if !*row_skip && changed {
                                        self.state.enabled_row = Some(row_idx);
                                    }
                                }
                                if ui.button(egui_phosphor::regular::TRASH).clicked() {
                                    data.remove_rows(vec![row_uid]);
                                }
                            });

                            let table_view_tool_state = self.state.tool_ui_state.entry((row_uid as i32).neg()).or_default();
                            let row_flagged = *table_view_tool_state == Variant::Bool(true);
                            if (hovered || row_flagged) && ui.add(flag_label(row_flagged).sense(Sense::click())).clicked() {
                                *table_view_tool_state = Variant::Bool(!row_flagged);
                            }

                            if hovered {
                                if let Some(tool_ui) = &mut self.tool_ui {
                                    let r = tool_ui(row_uid, self.state.tool_ui_state.entry(row_uid as i32).or_default(), ui);
                                    if let Some(r) = r {
                                        self.state.tool_ui_response = Some((row_uid, r));
                                    }
                                }
                            }
                        });
                        continue;
                    }
                    let col_skip = ui_column.skip;
                    let cell_uid_coord = CellCoord(row_uid, ui_column.col_uid);
                    ui_row.col(|ui| {
                        let skip = *row_skip || col_skip;
                        if let Some(cell_ref) = data.cell(cell_uid_coord) {
                            match VariantTy::from(cell_ref.value) {
                                VariantTy::Empty if !skip => {
                                    if cell_edit::add_and_select_missing_cell(
                                        self.settings.editable_cells,
                                        ui,
                                    ) {
                                        self.state.selected_range = Some(SelectedRange::single(row_idx, monotonic_col_idx));
                                        data.create_one(cell_uid_coord, Variant::Str(String::new()));
                                    }
                                    return;
                                }
                                VariantTy::Never => {
                                    // TODO: draw hatched grid
                                    ui.label("_Never_");
                                    return;
                                }
                                _ => {}
                            }
                            let cell_text = format!("{}", cell_ref);
                            if skip {
                                ui.colored_label(
                                    Color32::DARK_GRAY,
                                    RichText::new(cell_text).strikethrough(),
                                );
                            } else if self.settings.editable_cells
                                && self.state.selected_range == Some(SelectedRange::single(row_idx, monotonic_col_idx))
                                && !ui_column.is_read_only()
                            {
                                let mut first_pass = false;
                                if self.state.cell_when_editing.is_none() {
                                    first_pass = true;
                                    self.state.cell_when_editing = Some(cell_ref.value.clone());
                                }
                                let changed_value = if self.state.save_cell_changes_and_deselect || self.state.discard_cell_changes_and_deselect {
                                    self.state.selected_range = None;
                                    self.state.last_pressed = None;
                                    if self.state.save_cell_changes_and_deselect {
                                        self.state.save_cell_changes_and_deselect = false;
                                        self.state.cell_when_editing.take()
                                    } else {
                                        self.state.cell_when_editing = None;
                                        self.state.discard_cell_changes_and_deselect = false;
                                        None
                                    }
                                } else {
                                    let cell_when_editing =
                                        self.state.cell_when_editing.as_mut().expect("");
                                    if let Some(custom_editor) = ui_column.custom_edit_ui {
                                        let state = self.state.custom_ui_state.entry(cell_uid_coord).or_default();
                                        if let Some(action) = custom_editor(row_uid, cell_when_editing, state, ui) {
                                            match action {
                                                CustomEditUiResponse::UpdateCell(value) => Some(value),
                                                CustomEditUiResponse::CustomUiResponse(response) => {
                                                    self.state.custom_ui_response = Some((cell_uid_coord, response));
                                                    None
                                                }
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        cell_edit::show_cell_editor(
                                            row_uid,
                                            cell_when_editing,
                                            first_pass,
                                            ui_column,
                                            ui,
                                        )
                                    }
                                };
                                if let Some(changed_value) = changed_value {
                                    if &changed_value != cell_ref.value {
                                        debug!("updating cell {cell_uid_coord:?} with new value: {changed_value} ty: {}", VariantTy::from(&changed_value));
                                        data.modify_one(
                                            cell_uid_coord,
                                            changed_value,
                                        );
                                        data.one_shot_flags_mut().cells_updated = vec![cell_uid_coord];
                                        // self.state.dirty_cell = Some((row_idx, monotonic_col_idx));
                                    }
                                    self.state.cell_when_editing = None;
                                    self.state.selected_range = None;
                                }
                            } else {
                                let has_correct_type = VariantTy::from(cell_ref.value) == ui_column.ty;
                                if has_correct_type && !cell_text.is_empty() {
                                    match ui_column.custom_ui {
                                        Some(custom) => {
                                            let state = self.state.custom_ui_state.entry(cell_uid_coord).or_default();
                                            if let Some(response) = custom(row_uid, cell_ref.value, state, ui) {
                                                self.state.custom_ui_response = Some((cell_uid_coord, response));
                                            }
                                        },
                                        None => cell_view::show_cell(
                                            self.state.cell_metadata.get(&cell_uid_coord),
                                            ui,
                                            cell_ref.value,
                                            &cell_text,
                                        )
                                    }
                                } else if cell_text.is_empty() {
                                    if !has_correct_type {
                                        ui.colored_label(ui.ctx().style().visuals.warn_fg_color, "Incorrect");
                                    }
                                } else {
                                    ui.colored_label(ui.ctx().style().visuals.warn_fg_color, cell_text.as_str());
                                };
                                if ui.ui_contains_pointer() && !has_correct_type {
                                    egui::show_tooltip(
                                        ui.ctx(),
                                        "tableview_incorrect_data_tooltip".into(),
                                        |ui| {
                                            ui.label("Incorrect value for the required data type");
                                        },
                                    );
                                }
                            }
                        }
                    });
                }
            },
        );
        self.handle_selections(selection_event, key_navigation);
    }

    fn handle_selections(
        &mut self,
        selection_event: Option<SelectionEvent>,
        key_navigation: SelectionKeyNavigation,
    ) {
        if key_navigation.any_moves() {
            let was_selected = self.state.selected_range;
            if let Some(already_selected) = &mut self.state.selected_range {
                if key_navigation.left {
                    already_selected.move_left(key_navigation.shift);
                }
                if key_navigation.right {
                    already_selected.move_right(key_navigation.shift, self.state.columns.len());
                }
                if key_navigation.up {
                    already_selected.move_up(key_navigation.shift);
                }
                if key_navigation.down {
                    already_selected.move_down(key_navigation.shift, 100); // TODO: unhardcode
                }
            }
            if was_selected != self.state.selected_range {
                self.state.save_cell_changes_and_deselect = true;
                if let Some(r) = self.state.selected_range {
                    self.state.last_pressed = Some((r.row_start, r.col_start));
                }
            }
        }
        let Some(e) = selection_event else {
            return;
        };
        debug!("{e:?}");
        match e {
            SelectionEvent::Pressed(row, col) => {
                if self.state.last_pressed == Some((row, col)) {
                    debug!("Ignoring");
                } else {
                    if let Some(already_selected) = self.state.selected_range {
                        if key_navigation.shift {
                            debug!("Ignoring due to shift pressed");
                            return;
                        }
                        if already_selected.is_single_cell() {
                            debug!("Deselect and save if any changes");
                            self.state.save_cell_changes_and_deselect = true;
                        } else {
                            debug!("Dropping multi cell selection");
                            self.state.selected_range = None;
                        }
                    }
                    self.state.last_pressed = Some((row, col));
                }
            }
            SelectionEvent::Released(row, col) => {
                let Some(prev_pressed) = self.state.last_pressed else {
                    return;
                };
                let new_selected_range =
                    SelectedRange::ordered(prev_pressed.0, prev_pressed.1, row, col);
                if self.state.selected_range != Some(new_selected_range) {
                    debug!("setting new selection range: {:?}", new_selected_range);
                    self.state.selected_range = Some(new_selected_range);
                } else {
                    debug!("ignoring same selection");
                }
            }
        }
    }

    fn map_columns(&mut self, data: &impl TableBackend) {
        // debug!("TableView: Mapping columns");
        let data_columns = data.used_columns();
        let mut matched_headers = Vec::new();
        let mut columns = Vec::new();
        // skip absent columns for now, unless backend adds columns after the fact - non existent use case?
        // let mut absent_col_idx = u32::MAX;
        for required in &self.required_columns {
            for (col_id, col) in data_columns.iter() {
                let mut matched = false;
                let name = col.name.to_lowercase();
                for synonym in &required.synonyms {
                    if &name == synonym {
                        matched = true;
                        break;
                    }
                }
                if matched {
                    if matched_headers.contains(col_id) {
                        warn!("Double match for column: {}", col.name);
                    } else {
                        matched_headers.push(*col_id);
                    }
                    let header = UiColumn {
                        name: col.name.clone(),
                        ty: required.ty,
                        ty_locked: required.ty_locked,
                        default_value: required.default_value.clone(),
                        col_uid: *col_id,
                        skip: false,
                        kind: col.kind,
                        recognized: true,
                        width: 0.0,
                        user_map: required.user_map,
                        custom_ui: required.custom_ui,
                        custom_edit_ui: required.custom_edit_ui,
                        is_tool: false,
                    };
                    columns.push((col.name.as_str(), header));
                }
            }
        }
        // Put all additional columns to the right of required ones
        for (col_id, col) in data_columns.iter().sorted_by_key(|(col_id, _)| **col_id) {
            if !matched_headers.contains(col_id) {
                columns.push((
                    col.name.as_str(),
                    UiColumn {
                        name: col.name.clone(),
                        ty: col.ty,
                        ty_locked: false,
                        default_value: col.default.clone(),
                        col_uid: *col_id,
                        skip: false,
                        kind: col.kind,
                        recognized: false,
                        width: 0.0,
                        user_map: None,
                        custom_ui: None,
                        custom_edit_ui: None,
                        is_tool: false,
                    },
                ));
            }
        }
        self.state.columns.push(UiColumn {
            name: "Tool".to_string(),
            ty: VariantTy::Empty,
            ty_locked: false,
            default_value: None,
            col_uid: 0,
            skip: false,
            kind: CellKind::Static(StaticCellKind::AutoGenerated),
            recognized: false,
            width: 0.0,
            user_map: None,
            custom_ui: None,
            custom_edit_ui: None,
            is_tool: true,
        });
        for ordered_name in &self.persistent_settings.loose_column_order {
            if let Some(pos) = columns.iter().position(|(name, _)| name == ordered_name) {
                let (_, column) = columns.remove(pos);
                self.state.columns.push(column);
            }
        }
        for (_, column) in columns.into_iter() {
            self.state.columns.push(column);
        }
        debug!("mapped columns: {:?}", &self.state.columns);
    }

    // pub fn cell<'a>(
    //     &self,
    //     coord: CellCoord,
    //     data: &'a mut impl TableBackend,
    // ) -> Option<TableCellRef<'a>> {
    //     let user_mapped_cel = coord.1;
    //     for col in &self.state.columns {
    //         if col.user_map == Some(user_mapped_cel) {
    //             return data.cell(CellCoord(coord.0, col.backend_map));
    //         }
    //     }
    //     None
    // }

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

    pub fn add_cell_lint(&mut self, row_uid: u32, col_user_map: u32, lint: Lint) {
        let col_uid = self
            .state
            .columns
            .iter()
            .find(|c| c.user_map == Some(col_user_map))
            .map(|c| c.col_uid);
        if let Some(col_uid) = col_uid {
            let lints = &mut self
                .state
                .cell_metadata
                .entry(CellCoord(row_uid, col_uid))
                .or_default()
                .lints;
            if !lints.contains(&lint) {
                lints.push(lint);
            }
        } else {
            warn!("Failed to add lint for {row_uid} {col_user_map}");
        }
    }

    pub fn add_cell_tooltip(&mut self, row_uid: u32, col_user_map: u32, tooltip: impl AsRef<str>) {
        let col_uid = self
            .state
            .columns
            .iter()
            .find(|c| c.user_map == Some(col_user_map))
            .map(|c| c.col_uid);
        if let Some(col_uid) = col_uid {
            self.state
                .cell_metadata
                .entry(CellCoord(row_uid, col_uid))
                .or_default()
                .tooltip = tooltip.as_ref().to_string();
        }
    }

    pub fn clear_cell_lints(&mut self, row_uid: u32, col_user_map: u32) {
        let col_uid = self
            .state
            .columns
            .iter()
            .find(|c| c.user_map == Some(col_user_map))
            .map(|c| c.col_uid);
        if let Some(col_uid) = col_uid {
            self.state
                .cell_metadata
                .entry(CellCoord(row_uid, col_uid))
                .and_modify(|m| {
                    m.tooltip.clear();
                    m.lints.clear();
                });
        }
    }

    // pub fn load_state(&mut self, storage: &dyn eframe::Storage) {
    //     let key = format!("table_view_{}", self.id);
    //     self.persistent_settings = eframe::get_value(storage, key.as_str()).unwrap_or_default();
    // }

    // pub fn save_state(&self, storage: &mut dyn eframe::Storage) {
    //     let key = format!("table_view_{}", self.id);
    //     let settings = PersistentSettings {
    //         loose_column_order: self.state.columns.iter().map(|c| c.name.clone()).collect(),
    //         ..self.persistent_settings
    //     };
    //     // let mut column_names = self.state.columns.iter().map(|c| c.name.as_str());
    //     // let column_names = IteratorAdapter(RefCell::new(&mut column_names));

    //     eframe::set_value(storage, key.as_str(), &settings);
    // }

    pub fn scroll_to_row(&mut self, monotonic_row_idx: usize) {
        self.state.scroll_to_row = Some(monotonic_row_idx);
    }
}
