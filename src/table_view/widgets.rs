use egui::{Color32, FontFamily, Label, Rect, RichText, Vec2};

pub fn draw_shadow_rect(ui: &mut egui::Ui, rect: Rect, thickness: f32, color_dark: Color32) {
    // let color_dark = Color32::BLACK;
    let color_bright = Color32::TRANSPARENT;
    let inner_rect = rect.shrink2(Vec2::new(thickness, thickness));

    use egui::epaint::Vertex;
    let shadow = egui::Mesh {
        indices: vec![
            0, 1, 5, 0, 5, 4, 0, 4, 2, 4, 6, 2, 2, 6, 3, 3, 6, 7, 3, 7, 1, 1, 7, 5,
        ],
        vertices: vec![
            Vertex {
                pos: rect.left_top(),
                uv: egui::epaint::WHITE_UV,
                color: color_dark,
            },
            Vertex {
                pos: rect.right_top(),
                uv: egui::epaint::WHITE_UV,
                color: color_dark,
            },
            Vertex {
                pos: rect.left_bottom(),
                uv: egui::epaint::WHITE_UV,
                color: color_dark,
            },
            Vertex {
                pos: rect.right_bottom(),
                uv: egui::epaint::WHITE_UV,
                color: color_dark,
            },
            Vertex {
                pos: inner_rect.left_top(),
                uv: egui::epaint::WHITE_UV,
                color: color_bright,
            },
            Vertex {
                pos: inner_rect.right_top(),
                uv: egui::epaint::WHITE_UV,
                color: color_bright,
            },
            Vertex {
                pos: inner_rect.left_bottom(),
                uv: egui::epaint::WHITE_UV,
                color: color_bright,
            },
            Vertex {
                pos: inner_rect.right_bottom(),
                uv: egui::epaint::WHITE_UV,
                color: color_bright,
            },
        ],
        texture_id: Default::default(),
    };
    ui.painter().add(shadow);
}

pub fn flag_label(selected: bool) -> Label {
    let (icon, font, color) = if selected {
        (
            egui_phosphor::fill::FLAG_PENNANT,
            FontFamily::Name("phosphor-fill".into()),
            Color32::RED,
        )
    } else {
        (
            egui_phosphor::regular::FLAG_PENNANT,
            FontFamily::default(),
            Color32::GRAY,
        )
    };
    Label::new(RichText::new(icon).size(14.0).color(color).family(font))
}
