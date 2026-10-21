use egui_tabular::rvariant::{Number, NumberTy, Variant, VariantTy};
use egui_tabular::{
    CellCoord, ColumnDef, ColumnFilter, ColumnUid, DateCellUi, TableEvent, TableModel, TableView,
    TableViewConfig, TableViewOptions, VariantTable,
};
use std::collections::BTreeSet;

const NAME: ColumnUid = ColumnUid(0);
const STATUS: ColumnUid = ColumnUid(1);
const COUNT: ColumnUid = ColumnUid(2);
const DUE: ColumnUid = ColumnUid(3);

const STATUSES: [&str; 4] = ["Open", "In progress", "Blocked", "Done"];
const CONFIG_KEY: &str = "table_view_config";

struct SimpleApp {
    table: VariantTable,
    viewer: TableView,
    cell_ui: DateCellUi,
    config: TableViewConfig,
    detail_open: bool,
}

impl SimpleApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        if let Some(config) = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, CONFIG_KEY))
        {
            app.config = config;
        }
        app
    }

    /// The status picked in the toolbar, `None` for "All".
    fn status_filter(&self) -> Option<&str> {
        match self.config.filter.columns.get(&STATUS) {
            Some(ColumnFilter::OneOf(set)) if set.len() == 1 => set.iter().next().map(|s| &**s),
            _ => None,
        }
    }

    fn set_status_filter(&mut self, status: Option<&str>) {
        match status {
            Some(s) => {
                let set = BTreeSet::from([s.to_string()]);
                self.config
                    .filter
                    .columns
                    .insert(STATUS, ColumnFilter::OneOf(set));
            }
            None => {
                self.config.filter.columns.remove(&STATUS);
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(
                egui::TextEdit::singleline(&mut self.config.filter.quick)
                    .hint_text("quick filter, all columns")
                    .desired_width(200.0),
            );
            ui.label("Status:");
            let mut picked = self.status_filter().map(str::to_string);
            egui::ComboBox::from_id_salt("status_filter")
                .selected_text(picked.as_deref().unwrap_or("All"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut picked, None, "All");
                    for status in STATUSES {
                        ui.selectable_value(&mut picked, Some(status.to_string()), status);
                    }
                });
            if picked.as_deref() != self.status_filter() {
                self.set_status_filter(picked.as_deref());
            }
            if ui
                .add_enabled(self.config.filter.is_active(), egui::Button::new("Clear"))
                .clicked()
            {
                self.config.filter = Default::default();
            }
            ui.separator();
            let (shown, total) = self.viewer.row_counts();
            ui.label(format!("{shown} of {total} rows"));
        });
    }

    fn detail_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("details")
            .default_size(220.0)
            .show(ui, |ui| {
                ui.heading("Row details");
                let Some(row) = self.viewer.active_row() else {
                    ui.label("Click a row.");
                    return;
                };
                egui::Grid::new("detail_grid").show(ui, |ui| {
                    for col in self.table.columns().collect::<Vec<_>>() {
                        let name = self.table.column(col).map(|c| c.name.clone());
                        let value = self.table.get(CellCoord::from((row, col)));
                        ui.label(name.unwrap_or_default());
                        ui.label(value.map(|v| v.to_string()).unwrap_or_default());
                        ui.end_row();
                    }
                });
            });
    }
}

impl Default for SimpleApp {
    fn default() -> Self {
        let mut table = VariantTable::new([
            ColumnDef::new("Name", VariantTy::Str).default(Variant::Str("Default name".into())),
            ColumnDef::new("Status", VariantTy::Str).default(Variant::Str("Open".into())),
            ColumnDef::new("Count", VariantTy::Number(NumberTy::U32))
                .default(Variant::Number(Number::U32(0))),
            ColumnDef::new("Due", VariantTy::Date),
        ]);
        let mut rng = fastrand::Rng::new();
        let mut name_gen = names::Generator::with_naming(names::Name::Numbered);
        let first_day = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        for _ in 0..10_000 {
            let due = first_day + chrono::Days::new(rng.u64(0..365));
            table.insert_row([
                (NAME, Variant::Str(name_gen.next().unwrap())),
                (
                    STATUS,
                    Variant::Str(STATUSES[rng.usize(0..STATUSES.len())].into()),
                ),
                (COUNT, Variant::Number(Number::U32(rng.u32(0..=1000)))),
                (DUE, Variant::Date(due)),
            ]);
        }
        Self {
            table,
            viewer: TableView::new(TableViewOptions::default()),
            cell_ui: DateCellUi::default(),
            config: Default::default(),
            detail_open: false,
        }
    }
}

impl eframe::App for SimpleApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, CONFIG_KEY, &self.config);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("MenuBar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.hyperlink_to("Abc", "Def");

                ui.hyperlink_to("(source)", "https://github.com/...");

                ui.separator();

                egui::widgets::global_theme_preference_buttons(ui);

                ui.separator();

                ui.checkbox(
                    &mut self.config.use_heterogeneous_row_heights,
                    "Variable row heights",
                );
                ui.checkbox(&mut self.detail_open, "Row details");
            });
        });
        egui::Panel::top("Toolbar").show(ui, |ui| self.toolbar(ui));

        if self.detail_open {
            self.detail_panel(ui);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            let output = self
                .viewer
                .show(ui, &mut self.table, &mut self.cell_ui, &mut self.config);
            for event in output.events {
                if let TableEvent::RowClicked { .. } = event {
                    self.detail_open = true;
                }
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "Simple Demo",
        eframe::NativeOptions {
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(SimpleApp::new(cc)))),
    )
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
                Box::new(|cc| Ok(Box::new(SimpleApp::new(cc)))),
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
