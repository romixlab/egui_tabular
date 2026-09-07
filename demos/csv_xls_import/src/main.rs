use egui_tabular::importers::tabular_importer::TabularImporterConfig;
use egui_tabular::rvariant::VariantTy;
use egui_tabular::{RequiredColumn, RequiredColumns, TabularImporter};

struct SimpleApp {
    importer: TabularImporter,
    config: TabularImporterConfig,
}

impl Default for SimpleApp {
    fn default() -> Self {
        let required_columns = RequiredColumns::new([
            RequiredColumn::new("key", VariantTy::Str).synonyms(["parameter", "parameter_name"]),
            RequiredColumn::new("value", VariantTy::u32()),
        ]);
        let importer = TabularImporter::new(required_columns);
        // importer.set_max_lines(1000);
        Self {
            importer,
            config: Default::default(),
        }
    }
}

impl eframe::App for SimpleApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("MenuBar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.hyperlink_to("Abc", "Def");

                ui.hyperlink_to("(source)", "https://github.com/...");

                ui.separator();

                egui::widgets::global_theme_preference_buttons(ui);

                ui.separator();
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.importer.show(&mut self.config, None, ui, ui.id());
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "Simple Demo",
        eframe::NativeOptions {
            // default_theme: eframe::Theme::Dark,
            centered: true,

            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(SimpleApp::default()))),
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
