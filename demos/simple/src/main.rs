use egui_tabular::backends::variant::VariantBackend;
use egui_tabular::rvariant::{Number, NumberTy, Variant, VariantTy};
use egui_tabular::table_view::TableViewConfig;
use egui_tabular::{ColumnUid, TableView};

struct SimpleApp {
    backend: VariantBackend,
    viewer: TableView,
    config: TableViewConfig,
}

impl Default for SimpleApp {
    fn default() -> Self {
        let mut backend = VariantBackend::new([
            (
                "Name",
                VariantTy::Str,
                Some(Variant::Str("Default name".into())),
            ),
            (
                "Count",
                VariantTy::Number(NumberTy::U32),
                Some(Variant::Number(Number::U32(0))),
            ),
        ]);
        let mut rng = fastrand::Rng::new();
        let mut name_gen = names::Generator::with_naming(names::Name::Numbered);
        for _ in 0..10_000 {
            backend.insert_row([
                (ColumnUid(0), Variant::Str(name_gen.next().unwrap())),
                (
                    ColumnUid(1),
                    Variant::Number(Number::U32(rng.u32(0..=1000))),
                ),
            ]);
        }
        Self {
            backend,
            viewer: TableView::new(),
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
            self.viewer
                .show(&mut self.backend, &mut self.config, None, ui, ui.id());
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
