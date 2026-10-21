//! Date, time and date-time cells (EDIT-13): formats, relative days, the parsing editor, and the
//! sort key and range filter for date columns.

use crate::fixture::Table;
use chrono::NaiveDate;
use egui::Key;
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    ColumnDef, ColumnUid, DateCellUi, DateFormats, DateRange, VariantTable, date_sort_key,
};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn table(days: &[NaiveDate], formats: DateFormats) -> Table<VariantTable, DateCellUi> {
    let mut model = VariantTable::new([ColumnDef::new("D", VariantTy::Date)]);
    for d in days {
        model.insert_row([(ColumnUid(0), Variant::Date(*d))]);
    }
    Table::custom(model, DateCellUi { formats })
}

fn german() -> DateFormats {
    DateFormats {
        date: "%d.%m.%Y".into(),
        today: Some(date(2026, 10, 21)),
        ..DateFormats::default()
    }
}

#[test]
fn date_uses_the_format() {
    let t = table(&[date(2026, 10, 18)], german());
    assert!(t.has("18.10.2026"));
}

#[test]
fn relative_days_show_days_from_today() {
    let formats = DateFormats {
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
    let formats = DateFormats {
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
fn sort_key_orders_dates() {
    let a = date_sort_key(&Variant::Date(date(2026, 10, 18))).unwrap();
    let b = date_sort_key(&Variant::Date(date(2026, 10, 21))).unwrap();
    assert!(a < b);
    assert_eq!(date_sort_key(&Variant::Str("x".into())), None);
}

#[test]
fn range_filter_includes_both_ends() {
    let range = DateRange {
        from: Some(Variant::Date(date(2026, 10, 18))),
        to: Some(Variant::Date(date(2026, 10, 21))),
    };
    let m = |d| range.matches(Some(&Variant::Date(d)));
    assert!(m(date(2026, 10, 18)));
    assert!(m(date(2026, 10, 21)));
    assert!(!m(date(2026, 10, 17)));
    assert!(!m(date(2026, 10, 22)));
    assert!(!range.matches(None));
    let open = DateRange {
        from: Some(Variant::Date(date(2026, 10, 18))),
        to: None,
    };
    assert!(open.matches(Some(&Variant::Date(date(2030, 1, 1)))));
}
