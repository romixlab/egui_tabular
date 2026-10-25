//! Row spans from the model (VIEW-16): a day's schedule with resources as columns and 15-minute
//! slots as rows. A booking is one cell over all its slots.

use egui_tabular::rvariant::Variant;
use egui_tabular::{
    Capabilities, CellCoord, CellLevel, CellMetadata, ColumnInfo, ColumnUid, Revision, RowSpan,
    RowUid, TableModel, TableView, TableViewConfig, TableViewOptions, VariantCellUi,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

const SLOTS: usize = 96;
const RESOURCES: [&str; 5] = ["Mill", "Lathe", "Printer", "Oven", "Laser"];

struct Booking {
    first: usize,
    slots: usize,
    name: String,
    meta: CellMetadata,
}

/// Column 0 is the time of the slot, columns 1.. the resources.
struct Schedule {
    columns: Vec<ColumnInfo>,
    bookings: Vec<Booking>,
    /// Booking covering (column, slot).
    covering: HashMap<(u32, usize), usize>,
}

impl Schedule {
    fn new() -> Self {
        let mut columns = vec![ColumnInfo::new("Time", None)];
        columns.extend(RESOURCES.iter().map(|r| ColumnInfo::new(*r, None)));
        let mut schedule = Schedule {
            columns,
            bookings: vec![],
            covering: HashMap::new(),
        };
        // A small deterministic generator: bookings of 1 to 16 slots with gaps between them.
        let mut seed = 0x2545_f491_u32;
        let mut next = |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % n) as usize
        };
        for col in 1..=RESOURCES.len() as u32 {
            let mut slot = next(4);
            let mut job = 1;
            while slot < SLOTS {
                let slots = (1 + next(16)).min(SLOTS - slot);
                let level = match next(6) {
                    0 => Some(CellLevel::Warning),
                    1 => Some(CellLevel::Info),
                    _ => None,
                };
                let name = format!("{} job {job}", RESOURCES[col as usize - 1]);
                let tooltip = format!("{} to {}", time(slot), time(slot + slots));
                for s in slot..slot + slots {
                    schedule.covering.insert((col, s), schedule.bookings.len());
                }
                schedule.bookings.push(Booking {
                    first: slot,
                    slots,
                    name,
                    meta: CellMetadata::new()
                        .level_opt(level)
                        .tooltip(Arc::new(tooltip)),
                });
                slot += slots + next(5);
                job += 1;
            }
        }
        schedule
    }

    fn booking(&self, coord: CellCoord) -> Option<&Booking> {
        let idx = self
            .covering
            .get(&(coord.col_uid.0, coord.row_uid.0 as usize))?;
        Some(&self.bookings[*idx])
    }
}

fn time(slot: usize) -> String {
    format!("{:02}:{:02}", slot / 4, slot % 4 * 15)
}

impl TableModel for Schedule {
    fn revision(&self) -> Revision {
        Revision::default()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn columns(&self) -> impl Iterator<Item = ColumnUid> {
        (0..self.columns.len() as u32).map(ColumnUid)
    }

    fn column(&self, col: ColumnUid) -> Option<&ColumnInfo> {
        self.columns.get(col.0 as usize)
    }

    fn rows(&self) -> impl Iterator<Item = RowUid> {
        (0..SLOTS as u64).map(RowUid)
    }

    fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
        if coord.col_uid.0 == 0 {
            return Some(Cow::Owned(Variant::Str(time(coord.row_uid.0 as usize))));
        }
        let booking = self
            .booking(coord)
            .filter(|b| b.first == coord.row_uid.0 as usize)?;
        Some(Cow::Owned(Variant::Str(booking.name.clone())))
    }

    fn metadata(&self, coord: CellCoord) -> Option<Cow<'_, CellMetadata>> {
        self.booking(coord).map(|b| Cow::Borrowed(&b.meta))
    }

    fn row_span(&self, coord: CellCoord) -> Option<RowSpan> {
        self.booking(coord).map(|b| RowSpan {
            first: RowUid(b.first as u64),
            rows: b.slots,
        })
    }
}

struct RowSpansApp {
    schedule: Schedule,
    view: TableView,
    config: TableViewConfig,
}

impl eframe::App for RowSpansApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("MenuBar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                ui.separator();
                ui.checkbox(
                    &mut self.config.use_heterogeneous_row_heights,
                    "Heterogeneous row heights",
                );
                ui.label(
                    "Click a booking, move with the arrow keys; sorting draws every slot alone.",
                );
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.view
                .show(ui, &mut self.schedule, &mut VariantCellUi, &mut self.config);
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Row spans demo",
        eframe::NativeOptions {
            centered: true,
            ..Default::default()
        },
        Box::new(|_cc| {
            Ok(Box::new(RowSpansApp {
                schedule: Schedule::new(),
                view: TableView::new(TableViewOptions::default()),
                config: TableViewConfig::default(),
            }))
        }),
    )
}
