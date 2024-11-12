use std::collections::HashMap;
use egui::{Response, Ui};
use egui_tabular::backend::{BackendColumn, ColumnUid, OneShotFlags, PersistentFlags, TableBackend, CellCoord, VisualRowIdx, RowUid};
use egui_tabular::TableView;

struct TableVecData {
    data: Vec<Vec<String>>,
    available_columns: HashMap<ColumnUid, BackendColumn>,
}

impl TableBackend for TableVecData {
    fn clear(&mut self) {}

    fn persistent_flags(&self) -> &PersistentFlags {
        todo!()
    }

    fn one_shot_flags(&self) -> &OneShotFlags {
        todo!()
    }

    fn one_shot_flags_mut(&mut self) -> &mut OneShotFlags {
        todo!()
    }

    fn available_columns(&self) -> &[ColumnUid] {
        todo!()
    }

    fn used_columns(&self) -> impl Iterator<Item=ColumnUid> {
        self.available_columns.keys().copied()
    }

    fn column_info(&self, col_uid: ColumnUid) -> Option<&BackendColumn> {
        self.available_columns.get(&col_uid)
    }

    fn row_count(&self) -> usize {
        self.data.len()
    }

    fn row_uid(&self, row_idx: VisualRowIdx) -> Option<RowUid> {
        Some(RowUid(row_idx.0 as u32))
    }

    fn show_cell_view(&self, coord: CellCoord, ui: &mut Ui) {
        if let Some(row) = self.data.get(coord.row_uid.0 as usize) {
            if let Some(cell) = row.get(coord.col_uid.0 as usize) {
                if coord.row_uid.0 == 2 {
                    return;
                }
                ui.label(cell);

                if coord.row_uid.0 == 3 && coord.col_uid.0 == 1 {
                    ui.label("Label 2");
                }
            }
        }
    }

    fn show_cell_editor(&mut self, coord: CellCoord, ui: &mut Ui) -> Option<Response> {
        todo!()
    }
}

impl TableVecData {
    pub fn new() -> Self {
        TableVecData {
            data: vec![
                vec!["Abc............".into(), "Def".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Row2 Col0".into(), "Ghj".into()],
                vec!["Row3 Col0".into(), "Row3 Col1".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Zyx".into(), "Ghj".into()],
                vec!["Zyx".into(), "Ghj".into()],
            ],
            available_columns: [
                (ColumnUid(0), BackendColumn {
                    name: "Col A".to_string(),
                    ty: "String".to_string(),
                    is_sortable: false,
                }),
                (ColumnUid(1), BackendColumn {
                    name: "Col B".to_string(),
                    ty: "String".to_string(),
                    is_sortable: false,
                }),
            ].into(),
        }
    }
}

struct SimpleApp {
    // table: egui_data_table::DataTable<Row>,
    // viewer: Viewer,
    data: TableVecData,
    viewer: TableView,
}

impl Default for SimpleApp {
    fn default() -> Self {
        Self {
            data: TableVecData::new(),
            viewer: TableView::new(),
        }
    }
}

impl eframe::App for SimpleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("MenuBar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.hyperlink_to("Abc", "Def");

                ui.hyperlink_to("(source)", "https://github.com/...");

                ui.separator();

                egui::widgets::global_theme_preference_buttons(ui);

                ui.separator();
            })
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.viewer.show(&mut self.data, ui);
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::App;

    eframe::run_simple_native(
        "Simple Demo",
        eframe::NativeOptions {
            // default_theme: eframe::Theme::Dark,
            centered: true,

            ..Default::default()
        },
        {
            let mut app = SimpleApp::default();
            move |ctx, frame| {
                app.update(ctx, frame);
            }
        },
    )
    .unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let start_result = eframe::WebRunner::new()
            .start(
                "the_canvas_id",
                web_options,
                Box::new(|_cc| Ok(Box::new(SimpleApp::default()))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading_text"));
        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
