//! Date, time and date-time cells (EDIT-13): formats, relative days, the parsing editor, and the
//! sort and range filter of date columns.

use crate::fixture::Table;
use chrono::NaiveDate;
use egui::Key;
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    CellFormats, ColumnDef, ColumnFilter, ColumnUid, FormattedCellUi, RowUid, SortKey, VariantTable,
};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn table(days: &[NaiveDate], formats: CellFormats) -> Table<VariantTable, FormattedCellUi> {
    let mut model = VariantTable::new([ColumnDef::new("D", VariantTy::Date)]);
    for d in days {
        model.insert_row([(ColumnUid(0), Variant::Date(*d))]);
    }
    Table::custom(model, FormattedCellUi { formats })
}

fn german() -> CellFormats {
    CellFormats {
        date: "%d.%m.%Y".into(),
        today: Some(date(2026, 10, 21)),
        ..CellFormats::default()
    }
}

#[test]
fn date_uses_the_format() {
    let t = table(&[date(2026, 10, 18)], german());
    assert!(t.has("18.10.2026"));
}

#[test]
fn relative_days_show_days_from_today() {
    let formats = CellFormats {
        relative_days: true,
        ..german()
    };
    let t = table(
        &[date(2026, 10, 18), date(2026, 10, 21), date(2026, 10, 23)],
        formats,
    );
    assert!(t.has("3 d ago"));
    assert!(t.has("today"));
    assert!(t.has("in 2 d"));
}

#[test]
fn editor_parses_the_format_and_commits() {
    let mut t = table(&[date(2026, 10, 18)], german());
    t.start_edit("18.10.2026");
    t.type_replace("05.11.2026");
    t.press(Key::Enter);
    assert!(t.has("05.11.2026"));
}

#[test]
fn editor_keeps_the_old_date_on_invalid_text() {
    let mut t = table(&[date(2026, 10, 18)], german());
    t.start_edit("18.10.2026");
    t.type_replace("not a date");
    t.press(Key::Enter);
    assert!(t.has("18.10.2026"));
}

#[test]
fn editor_accepts_relative_days() {
    let formats = CellFormats {
        relative_days: true,
        ..german()
    };
    let mut t = table(&[date(2026, 10, 18)], formats);
    t.start_edit("3 d ago");
    t.type_replace("in 5 d");
    t.press(Key::Enter);
    assert!(t.has("in 5 d"));
}

#[test]
fn view_sorts_mixed_dates_and_date_times_by_time() {
    let mut model = VariantTable::new([ColumnDef::new("D", VariantTy::DateTime)]);
    let at = |d: NaiveDate, h| {
        Variant::DateTime(d.and_hms_opt(h, 0, 0).unwrap().and_utc().fixed_offset())
    };
    // Kept as given: a Date among DateTimes still sorts by its point in time.
    for v in [
        at(date(2026, 10, 21), 9),
        Variant::Date(date(2026, 10, 20)),
        at(date(2026, 10, 19), 23),
    ] {
        model.insert_row([(ColumnUid(0), v)]);
    }
    let mut t = Table::custom(model, FormattedCellUi::default());
    t.config(|c| c.sort = vec![SortKey::ascending(ColumnUid(0))]);
    assert_eq!(
        t.h.state().view.visible_rows(),
        [RowUid(2), RowUid(1), RowUid(0)]
    );
}

#[test]
fn range_filter_on_dates_includes_both_ends() {
    let days = [17, 18, 21, 22].map(|d| date(2026, 10, d));
    let mut t = table(&days, CellFormats::default());
    t.config(|c| {
        c.filter.columns.insert(
            ColumnUid(0),
            ColumnFilter::Range {
                min: Some(Variant::Date(date(2026, 10, 18))),
                max: Some(Variant::Date(date(2026, 10, 21))),
            },
        );
    });
    assert_eq!(t.h.state().view.visible_rows(), [RowUid(1), RowUid(2)]);
}

#[test]
fn string_lists_show_inline_with_a_separator() {
    let mut model = VariantTable::new([ColumnDef::new("L", VariantTy::StrList)]);
    model.insert_row([(
        ColumnUid(0),
        Variant::StrList(vec!["hw".into(), "fw".into()]),
    )]);
    let formats = CellFormats {
        list_separator: Some(", ".into()),
        ..Default::default()
    };
    let t = Table::custom(model, FormattedCellUi { formats });
    assert!(t.has("hw, fw"));
}
