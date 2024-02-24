use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Neg;

use egui::{Align, Color32, Frame, Id, Key, Label, Layout, RichText, ScrollArea, Sense, Ui, Vec2};
use egui_modal::Modal;
use itertools::Itertools;
use log::{debug, warn};

use crate::backend::TableBackend;
use crate::cell::{CellCoord, CellKind, StaticCellKind, TableCellRef};
use crate::column::RequiredColumn;
use crate::filter::{FilterOperation, RowFilter, VariantFilter};
use rvariant::{Variant, VariantTy};

mod cell_edit;
mod cell_view;
#[allow(dead_code)]
mod table;
use table::{Column, SelectedRange, TableBody};

use self::cell_view::CellMetadata;
use self::interface::{
    CustomEditUiFn, CustomEditUiResponse, CustomToolUiFn, CustomUiFn, CustomUiResponse, Lint,
};
use self::widgets::flag_label;
mod interaction;
pub mod interface;
#[allow(dead_code)]
mod layout;
#[allow(dead_code)]
mod sizing;
mod util;
mod widgets;
pub use interface::{PersistentSettings, Settings};

pub struct TableView {
    id: String,
    state: State,
    required_columns: Vec<RequiredColumn>,

    settings: Settings,
    persistent_settings: PersistentSettings,

    tool_ui: Option<CustomToolUiFn>,

    // required_columns' idx -> CustomUiFn
    custom_ui: HashMap<u32, CustomUiFn>,
    custom_edit_ui: HashMap<u32, CustomEditUiFn>,
}

#[derive(Default)]
struct State {
    columns: Vec<UiColumn>,
    cell_metadata: HashMap<CellCoord, CellMetadata>,
    rows_skip: HashMap<u32, bool>,

    last_pressed: Option<(u32, u32)>,
    cell_when_editing: Option<Variant>,
    save_cell_changes_and_deselect: bool, // TODO: improve and keep coords of currently editing cell, instead of deferring to ui loop to commit changes?
    discard_cell_changes_and_deselect: bool,
    // dirty_cell: Option<(usize, usize)>,
    selected_range: Option<SelectedRange>,

    disabled_row: Option<u32>,
    enabled_row: Option<u32>,
    disabled_col: Option<u32>,
    enabled_col: Option<u32>,

    custom_ui_response: Option<(CellCoord, CustomUiResponse)>,
    custom_ui_state: HashMap<CellCoord, Variant>,

    tool_ui_state: HashMap<i32, Variant>,
    tool_ui_response: Option<(u32, CustomUiResponse)>,

    filter_value_text: String,
    scroll_to_row: Option<u32>,

    about_to_paste_rows: Vec<Vec<String>>,
    pasting_block_with_holes: bool,
    pasting_block_width: u32,
    create_rows_on_paste: bool,
    fill_with_same_on_paste: bool,
    create_adhoc_cols_on_paste: bool,

    clear_requested: bool,
}

#[derive(Clone, Debug)]
struct UiColumn {
    name: String,
    synonyms: Vec<String>,
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
    custom_ui: Option<CustomUiFn>,
    custom_edit_ui: Option<CustomEditUiFn>,
    is_tool: bool, // TODO: Refactor into enum
    is_required: bool,
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
pub(crate) struct SelectionKeyNavigation {
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
            id: id.as_ref().to_string(),
            state: State::default(),
            required_columns: required_columns.into_iter().collect(),

            settings: Settings::default(),
            persistent_settings: PersistentSettings::default(),

            tool_ui: None,
            custom_ui: HashMap::new(),
            custom_edit_ui: HashMap::new(),
        }
    }

    pub fn show(&mut self, data: &mut impl TableBackend, ui: &mut Ui) {
        // if !data.flags().column_info_present {
        //     ui.label("Empty table");
        //     return;
        // }
        if data.one_shot_flags().cleared {
            // self.state.cell_metadata.clear();
            self.state = State::default();
        }
        if data.one_shot_flags().first_pass || data.one_shot_flags().column_info_updated {
            self.map_columns(data);
        }
        data.poll();
        // if self.state.columns.is_empty() {
        //     ui.label("TableView: Empty table");
        //     return;
        // }

        self.state.disabled_col = None;
        self.state.enabled_col = None;
        self.state.disabled_row = None;
        self.state.enabled_row = None;

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
                                                        ui.add(Label::new(RichText::new(&ui_column.name).strong().monospace()).truncate(true).selectable(false)).on_hover_ui(|ui| {
                                                            if !ui_column.synonyms.is_empty() {
                                                                ui.label(format!("Alternative names: {:?}", ui_column.synonyms));
                                                            }
                                                        });
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
                                                if self.settings.skippable_columns && !required_column && ui.checkbox(&mut ui_column.skip, "Skip").changed() {
                                                    if ui_column.skip {
                                                        self.state.disabled_col = Some(idx as u32);
                                                    } else {
                                                        self.state.enabled_col = Some(idx as u32);
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
                table_builder.push_column(Column::initial(h.name.len() as f32 * 8.0 + 80.0).at_least(36.0).clip(true));
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
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Table View").strong().monospace());

            self.show_filters_in_use(data, ui);
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
                        .filter_map(|c| {
                            if c.is_tool {
                                None
                            } else {
                                c.default_value.clone().map(|d| (c.col_uid, d))
                            }
                        })
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
            ui.checkbox(&mut self.settings.editable_cells, "Edit");
            if is_empty_table {
                ui.label("Empty table");
                if data.persistent_flags().is_read_only {
                    ui.label("R/O").on_hover_text(
                        "Data source for this table is read only, cannot modify anything",
                    );
                }
            }

            ui.label("Row height");
            ui.add(egui::Slider::new(
                &mut self.persistent_settings.row_height,
                10.0..=500.0,
            ));
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
                    let monotonic_col_idx = monotonic_col_idx as u32;
                    if ui_column.is_tool {
                        let hovered = ui_row.hovered();
                        ui_row.col(|ui| {
                            ui.horizontal_wrapped(|ui| {
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

                                let table_view_tool_state = self.state.tool_ui_state.entry((row_uid as i32).neg()).or_insert(Variant::Empty);
                                let row_flagged = *table_view_tool_state == Variant::Bool(true);
                                if (hovered || row_flagged) && ui.add(flag_label(row_flagged).sense(Sense::click())).clicked() {
                                    *table_view_tool_state = Variant::Bool(!row_flagged);
                                }

                                if hovered {
                                    if let Some(tool_ui) = &mut self.tool_ui {
                                        let r = tool_ui(row_uid, self.state.tool_ui_state.entry(row_uid as i32).or_insert(Variant::Empty), ui);
                                        if let Some(r) = r {
                                            self.state.tool_ui_response = Some((row_uid, r));
                                        }
                                    }
                                }
                                });
                        });
                        continue;
                    }
                    let col_skip = ui_column.skip;
                    let cell_uid_coord = CellCoord(row_uid, ui_column.col_uid);
                    ui_row.col(|ui| {
                        let skip = *row_skip || col_skip;
                        let showing_editor = self.settings.editable_cells
                        && self.state.selected_range == Some(SelectedRange::single(row_idx, monotonic_col_idx))
                        && !ui_column.is_read_only();

                        match data.cell(cell_uid_coord) {
                            TableCellRef::Available { value, .. } => {
                                if skip {
                                    ui.colored_label(
                                        Color32::DARK_GRAY,
                                        RichText::new(value.to_string()).strikethrough(),
                                    );
                                } else if showing_editor {
                                    let mut first_pass = false;
                                    if self.state.cell_when_editing.is_none() {
                                        first_pass = true;
                                        self.state.cell_when_editing = Some(value.clone());
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
                                            let state = self.state.custom_ui_state.entry(cell_uid_coord).or_insert(Variant::Empty);
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
                                        if &changed_value != value {
                                            debug!("updating cell {cell_uid_coord:?} with new value: {changed_value} ty: {}", VariantTy::from(&changed_value));
                                            data.modify_one(
                                                cell_uid_coord,
                                                changed_value,
                                            );
                                        }
                                        self.state.cell_when_editing = None;
                                        self.state.selected_range = None;
                                    }
                                } else {
                                    match ui_column.custom_ui {
                                        Some(custom) => {
                                            let state = self.state.custom_ui_state.entry(cell_uid_coord).or_insert(Variant::Empty);
                                            if let Some(response) = custom(row_uid, value, state, ui) {
                                                self.state.custom_ui_response = Some((cell_uid_coord, response));
                                            }
                                        },
                                        None => cell_view::show_cell(
                                            self.state.cell_metadata.get(&cell_uid_coord),
                                            ui,
                                            value,
                                        )
                                    }
                                    let has_correct_type = VariantTy::from(value) == ui_column.ty;
                                    if !has_correct_type && ui.ui_contains_pointer() {
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
                            TableCellRef::Empty => {
                                if !skip && ui_column.is_required && cell_edit::add_and_select_missing_cell(
                                        self.settings.editable_cells,
                                        ui,
                                    ) {
                                    self.state.selected_range = Some(SelectedRange::single(row_idx, monotonic_col_idx));
                                    data.create_one(cell_uid_coord, ui_column.default_value.clone().unwrap_or(Variant::Str(String::new())));
                                }
                                // when clicking in edit mode on an empty cell
                                if showing_editor {
                                    debug!("Creating empty {cell_uid_coord:?}");
                                    data.create_one(cell_uid_coord, ui_column.default_value.clone().unwrap_or(Variant::Str(String::new())));
                                }
                            }
                            TableCellRef::Never => {
                                ui.label("TODO: hatch cell");
                            }
                        }
                    });
                }
            },
        );
        self.handle_selections(selection_event, key_navigation);
    }

    fn map_columns(&mut self, data: &impl TableBackend) {
        // debug!("TableView: Mapping columns");
        self.state.columns.clear();
        let data_columns = data.used_columns();
        let mut matched_headers = Vec::new();
        let mut columns = Vec::new();

        for (required_col_idx, required) in self.required_columns.iter().enumerate() {
            let required_col_idx = required_col_idx as u32;
            if let Some(data_col_id) = required.find_match_map(data_columns) {
                let data_col = data_columns.get(&data_col_id).unwrap();
                if matched_headers.contains(&data_col_id) {
                    warn!("Double match for column: {}", data_col.name);
                } else {
                    matched_headers.push(data_col_id);

                    let header = UiColumn {
                        name: required.name.clone(),
                        synonyms: required.synonyms.clone(),
                        ty: required.ty,
                        ty_locked: required.ty_locked,
                        default_value: required.default_value.clone(),
                        col_uid: data_col_id,
                        skip: false,
                        kind: data_col.kind,
                        recognized: true,
                        width: 0.0,
                        custom_ui: self.custom_ui.get(&required_col_idx).cloned(),
                        custom_edit_ui: self.custom_edit_ui.get(&required_col_idx).cloned(),
                        is_tool: false,
                        is_required: true,
                    };
                    columns.push((data_col.name.as_str(), header));
                }
            } else {
                let header = UiColumn {
                    name: required.name.clone(),
                    synonyms: required.synonyms.clone(),
                    ty: required.ty,
                    ty_locked: required.ty_locked,
                    default_value: required.default_value.clone(),
                    col_uid: required_col_idx,
                    skip: false,
                    kind: CellKind::Static(StaticCellKind::Plain),
                    recognized: true,
                    width: 0.0,
                    custom_ui: None,
                    custom_edit_ui: None,
                    is_tool: false,
                    is_required: true,
                };
                columns.push((required.name.as_str(), header));
            }
        }
        // Put all additional columns to the right of required ones
        for (data_col_id, col) in data_columns.iter().sorted_by_key(|(col_id, _)| **col_id) {
            if !matched_headers.contains(data_col_id) {
                columns.push((
                    col.name.as_str(),
                    UiColumn {
                        name: col.name.clone(),
                        synonyms: Vec::new(),
                        ty: col.ty,
                        ty_locked: false,
                        default_value: col.default_value.clone(),
                        col_uid: *data_col_id,
                        skip: false,
                        kind: col.kind,
                        recognized: false,
                        width: 0.0,
                        custom_ui: None,
                        custom_edit_ui: None,
                        is_tool: false,
                        is_required: false,
                    },
                ));
            }
        }
        self.state.columns.push(UiColumn {
            name: "Tool".to_string(),
            synonyms: Vec::new(),
            ty: VariantTy::Empty,
            ty_locked: false,
            default_value: None,
            col_uid: 0,
            skip: false,
            kind: CellKind::Static(StaticCellKind::AutoGenerated),
            recognized: false,
            width: 0.0,
            custom_ui: None,
            custom_edit_ui: None,
            is_tool: true,
            is_required: false,
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
}
