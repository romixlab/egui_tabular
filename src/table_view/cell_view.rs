use std::rc::Rc;

use egui::text::LayoutJob;
use egui::{Color32, Label, TextFormat, Ui};

use rvariant::Variant;

use crate::table_view::Lint;

#[derive(Default)]
pub(crate) struct CellMetadata {
    pub(crate) lints: Vec<Lint>,
    // TODO: allow tooltips for individuals elements?
    pub(crate) tooltips: Vec<Rc<String>>,
    pub(crate) text_format: Option<TextFormat>,
}

pub(super) fn show_cell(
    metadata: Option<&CellMetadata>,
    ui: &mut Ui,
    cell_value: &Variant,
    is_ty_correct: bool,
) {
    let mut job = LayoutJob::default();
    let text_format = metadata
        .and_then(|m| m.text_format.clone())
        .unwrap_or_default();
    match cell_value {
        Variant::Str(s) => {
            job.append(s, 0.0, text_format);
        }
        Variant::StrList(list) => {
            for (idx, s) in list.iter().enumerate() {
                let format = metadata
                    .and_then(|m| {
                        m.lints.iter().find_map(|l| match l {
                            Lint::HighlightIndex {
                                idx: lint_idx,
                                text_format,
                            } => {
                                if *lint_idx == idx {
                                    Some(text_format.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_default();
                job.append(s, 0.0, format);
                if idx < list.len() - 1 {
                    let separator = if s.len() > 10 { "\n" } else { ", " };
                    job.append(separator, 0.0, text_format.clone());
                }
            }
        }
        // Variant::List(list) => {
        //     for s in list.iter().map(|v| v.to_string()) {
        //         job.append(&s, 0.0, TextFormat::default());
        //         job.append("\n", 0.0, TextFormat::default());
        //     }
        // }
        other => {
            job.append(other.to_string().as_str(), 0.0, text_format);
        }
    }
    ui.horizontal_wrapped(|ui| {
        // ui.label(job);
        if !is_ty_correct {
            ui.colored_label(Color32::RED, egui_phosphor::regular::WARNING_CIRCLE)
                .on_hover_text("Incorrect value for the required data type");
        }
        ui.add(Label::new(job).selectable(false)).on_hover_ui(|ui| {
            ui.label(cell_value.to_string());
        });
        if let Some(m) = metadata {
            for (color, icon) in m.lints.iter().filter_map(|l| {
                if let Lint::AddIcon { color, icon } = l {
                    Some((*color, *icon))
                } else {
                    None
                }
            }) {
                ui.colored_label(color, icon);
            }
        }
    });
    if let Some(m) = metadata {
        if !m.tooltips.is_empty() && ui.rect_contains_pointer(ui.max_rect()) {
            egui::show_tooltip(ui.ctx(), egui::Id::new("show_cell_tooltip"), |ui| {
                for t in &m.tooltips {
                    ui.label(t.as_str());
                }
            });
        }
    }
    // let warnings = metadata.map(|m| &m.warnings);
    // if let Some(warnings) = warnings {
    //     let mut warnings = warnings.clone();
    //     warnings.sort_by(|a, b| a.0.start.cmp(&b.0.start));
    //     let mut last_char_idx = 0;
    //     for w in warnings.iter() {
    //         if w.0.start > cell_text.len() || w.0.end > cell_text.len() {
    //             warn!("Malformed warning range");
    //             break;
    //         }
    //         if w.0.start >= last_char_idx {
    //             job.append(
    //                 &cell_text[last_char_idx..w.0.start],
    //                 0.0,
    //                 TextFormat::default(),
    //             );
    //         }
    //         job.append(
    //             &cell_text[w.0.clone()],
    //             0.0,
    //             TextFormat {
    //                 underline: Stroke {
    //                     color: Color32::RED,
    //                     width: 2.0,
    //                 },
    //                 ..Default::default()
    //             },
    //         );
    //         last_char_idx = w.0.end;
    //     }
    //     if last_char_idx != cell_text.len() {
    //         job.append(
    //             &cell_text[last_char_idx..cell_text.len()],
    //             0.0,
    //             TextFormat::default(),
    //         );
    //     }
    //     ui.label(job);
    // } else {
    // let mut job =
    //     LayoutJob::single_section(cell_text.clone(), TextFormat::default());
    // job.wrap = TextWrapping {
    //     break_anywhere: false,
    //     ..Default::default()
    // };
    //
    // ui.label(job); // `Label` overrides some of the wrapping settings, e.g. wrap width
    // ui.add(Label::new(cell_text).truncate(true));
    // }
}
