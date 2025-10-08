use egui_tabular::table_view::TableViewConfig;
use egui_tabular::{TableView, TabularRow};

#[derive(TabularRow)]
struct UserRow {
    name: String,
    x: u32,
    y: u32,
    #[format = "{:.02}"]
    strength: f32,
}

struct DeriveRowApp {
    backend: UserRowTabularBackend,
    viewer: TableView,
    config: TableViewConfig,
}

impl Default for DeriveRowApp {
    fn default() -> Self {
        let backend = UserRowTabularBackend::new(vec![
            UserRow {
                name: "Point A".to_string(),
                x: 1,
                y: 3,
                strength: 1.0,
            },
            UserRow {
                name: "Point B".to_string(),
                x: 5,
                y: 7,
                strength: 2.0,
            },
            UserRow {
                name: "Point C".to_string(),
                x: 9,
                y: 11,
                strength: 3.0,
            },
        ]);
        Self {
            backend,
            viewer: TableView::new(),
            config: Default::default(),
        }
    }
}

impl eframe::App for DeriveRowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("MenuBar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.viewer
                .show(&mut self.backend, &mut self.config, None, ui, ui.id());
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::App;

    eframe::run_simple_native(
        "Derive TabularRow Demo",
        eframe::NativeOptions {
            // default_theme: eframe::Theme::Dark,
            centered: true,

            ..Default::default()
        },
        {
            let mut app = DeriveRowApp::default();
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
                Box::new(|_cc| Ok(Box::new(DeriveRowApp::default()))),
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
