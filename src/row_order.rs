//! The view's row order: the filter and sort stage between the model's natural order and the
//! screen (DESIGN-13, SORT-1, FILTER-1).
//!
//! The pipeline is `model.rows()` (or [`TableModel::sorted_rows`]) → filter → stable sort. It
//! never changes the model; the result is the order [`TableView`](crate::TableView) shows.

use crate::CellUi;
use crate::util::row_texts;
use rvariant::Variant;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tabular_core::{CellCoord, ColumnUid, RowUid, SortDirection, SortKey, TableModel};

/// Which rows the view shows. Part of [`TableViewConfig`](crate::TableViewConfig), so it is
/// saved with the rest of the view state. The app may change it at any time (e.g. from its own
/// filter toolbar); the view re-applies it on the next frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowFilter {
    /// Quick filter over the text of every shown column. Empty = off. See
    /// [`RowOrderOptions::quick_filter_terms`] for how the text is split.
    pub quick: String,
    /// Per-column filters; a row is shown only if it passes all of them. Filters of columns the
    /// model doesn't have are ignored and kept.
    pub columns: BTreeMap<ColumnUid, ColumnFilter>,
}

impl RowFilter {
    /// True if this filter can hide rows.
    pub fn is_active(&self) -> bool {
        !self.quick.trim().is_empty() || !self.columns.is_empty()
    }
}

/// A filter on one column.
///
/// Built-in kinds work on the cell's value ([`TableModel::get`]) or its text (the value's
/// `to_string()`, or [`CellUi::text`] where the model has no value). [`CellUi::matches`] can
/// replace the evaluation for any kind, and is the only meaning of [`ColumnFilter::Custom`].
// A few filters per table: the size of `Range` doesn't matter, a plain `Variant` is nicer to use.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ColumnFilter {
    /// The cell text contains this text.
    Contains(String),
    /// The cell text equals one of these. For a `StrList` value: any item equals one of these.
    OneOf(BTreeSet<String>),
    /// The value lies within `min..=max` (each bound optional), compared like sorting does.
    /// Cells without a value don't pass.
    Range {
        min: Option<Variant>,
        max: Option<Variant>,
    },
    /// `true`: only empty cells (no value, `Variant::Empty` or empty text); `false`: only
    /// non-empty ones.
    Empty(bool),
    /// Defined by the app: its [`CellUi::matches`] gets the string (e.g. serialized parameters).
    /// Without such a hook every row passes.
    Custom(String),
}

/// When the view re-applies sort and filter after cell values change. Changes of the row set,
/// of [`TableViewConfig::sort`](crate::TableViewConfig::sort) and of the filter always re-apply.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortPolicy {
    /// Edits made in this view never move or hide rows (the row stays where the user is
    /// looking); changes from outside the view (`revision.cells` bumped without a view
    /// command) re-apply.
    #[default]
    OnExternalChange,
    /// Re-apply on every cell change, including this view's edits (live "top N" views).
    Live,
    /// Only when the user or the app asks ([`TableView::refresh_rows`](crate::TableView::refresh_rows)).
    /// [`TableView::is_row_order_stale`](crate::TableView::is_row_order_stale) tells whether
    /// values changed since.
    OnDemand,
}

/// Tunables of the filter and sort stage, chosen by the app
/// ([`TableViewOptions::row_order`](crate::TableViewOptions::row_order)).
#[derive(Clone, Debug, PartialEq)]
pub struct RowOrderOptions {
    /// When cell value changes re-apply sort and filter. Default: [`SortPolicy::OnExternalChange`].
    pub policy: SortPolicy,
    /// Sort text case-insensitively ("apple" before "Banana"). Default: true.
    pub sort_case_insensitive: bool,
    /// Empty cells sort after all values in both directions. Default: true.
    pub empty_last: bool,
    /// Match filter text (quick filter, [`ColumnFilter::Contains`], [`ColumnFilter::OneOf`])
    /// case-insensitively. Default: true.
    pub filter_case_insensitive: bool,
    /// Split the quick filter at whitespace into terms that must all match, each in any column
    /// ("cli open" finds rows with "cli" in one column and "open" in another). Off: the whole
    /// text is one term. Default: true.
    pub quick_filter_terms: bool,
}

impl Default for RowOrderOptions {
    fn default() -> Self {
        RowOrderOptions {
            policy: SortPolicy::default(),
            sort_case_insensitive: true,
            empty_last: true,
            filter_case_insensitive: true,
            quick_filter_terms: true,
        }
    }
}

/// What a click on a column's sort control does to `sort`: cycles the column ascending →
/// descending → unsorted. A plain click makes it the only key; with `add` (Shift+click) the
/// other keys stay and a new column is appended as the next key. Returns true if `sort` changed.
pub fn click_sort(sort: &mut Vec<SortKey>, col: ColumnUid, add: bool) -> bool {
    let before = sort.clone();
    let current = sort.iter().position(|k| k.col == col);
    let next = match current.map(|i| sort[i].direction) {
        None => Some(SortDirection::Ascending),
        Some(SortDirection::Ascending) => Some(SortDirection::Descending),
        Some(SortDirection::Descending) => None,
    };
    if add {
        match (current, next) {
            (Some(i), Some(direction)) => sort[i].direction = direction,
            (Some(i), None) => {
                sort.remove(i);
            }
            (None, Some(direction)) => sort.push(SortKey { col, direction }),
            (None, None) => {}
        }
    } else {
        sort.clear();
        sort.extend(next.map(|direction| SortKey { col, direction }));
    }
    *sort != before
}

/// Result of [`compute`].
pub(crate) struct RowOrder {
    /// Shown rows in display order.
    pub(crate) rows: Vec<RowUid>,
    /// Position of each row in the model's natural order, `None` when the order is natural and
    /// nothing is filtered out (index = display index).
    pub(crate) natural: Option<HashMap<RowUid, usize>>,
    /// Rows in the model.
    pub(crate) total: usize,
}

/// Run the pipeline: natural (or delegated) order → filter → stable sort.
/// `shown_columns` are the columns the quick filter looks at.
pub(crate) fn compute<M: TableModel, C: CellUi<M>>(
    model: &M,
    cell_ui: &C,
    shown_columns: &[ColumnUid],
    sort: &[SortKey],
    filter: &RowFilter,
    options: &RowOrderOptions,
) -> RowOrder {
    let natural_rows: Vec<RowUid> = model.rows().collect();
    let total = natural_rows.len();
    let sort: Vec<SortKey> = sort
        .iter()
        .filter(|k| model.column(k.col).is_some())
        .copied()
        .collect();
    let column_filters: Vec<(ColumnUid, &ColumnFilter)> = filter
        .columns
        .iter()
        .filter(|(col, _)| model.column(**col).is_some())
        .map(|(col, f)| (*col, f))
        .collect();
    let terms = quick_terms(&filter.quick, options);
    if sort.is_empty() && column_filters.is_empty() && terms.is_empty() {
        return RowOrder {
            rows: natural_rows,
            natural: None,
            total,
        };
    }
    let natural: HashMap<RowUid, usize> = natural_rows
        .iter()
        .enumerate()
        .map(|(idx, row)| (*row, idx))
        .collect();

    let delegated = if sort.is_empty() {
        None
    } else {
        model.sorted_rows(&sort)
    };
    let sorted_by_model = delegated.is_some();
    let mut rows = delegated.unwrap_or(natural_rows);

    if !terms.is_empty() || !column_filters.is_empty() {
        rows.retain(|row| {
            column_filters
                .iter()
                .all(|(col, f)| cell_matches(model, cell_ui, (*row, *col).into(), f, options))
                && quick_matches(model, cell_ui, *row, shown_columns, &terms, options)
        });
    }

    if !sorted_by_model && !sort.is_empty() {
        rows = sort_rows(model, cell_ui, rows, &sort, options);
    }

    RowOrder {
        rows,
        natural: Some(natural),
        total,
    }
}

fn quick_terms(quick: &str, options: &RowOrderOptions) -> Vec<String> {
    let quick = if options.filter_case_insensitive {
        quick.to_lowercase()
    } else {
        quick.to_owned()
    };
    if options.quick_filter_terms {
        quick.split_whitespace().map(str::to_owned).collect()
    } else if quick.trim().is_empty() {
        vec![]
    } else {
        vec![quick]
    }
}

fn quick_matches<M: TableModel, C: CellUi<M>>(
    model: &M,
    cell_ui: &C,
    row: RowUid,
    columns: &[ColumnUid],
    terms: &[String],
    options: &RowOrderOptions,
) -> bool {
    if terms.is_empty() {
        return true;
    }
    let texts: Vec<String> = row_texts(model, cell_ui, row, columns)
        .into_iter()
        .map(|t| fold_case(t, options.filter_case_insensitive))
        .collect();
    terms
        .iter()
        .all(|term| texts.iter().any(|text| text.contains(term.as_str())))
}

fn fold_case(text: String, case_insensitive: bool) -> String {
    if case_insensitive {
        text.to_lowercase()
    } else {
        text
    }
}

/// Cell text as used by filters: the value's text, or [`CellUi::text`] without a value.
fn cell_text<M: TableModel, C: CellUi<M>>(
    value: Option<&Variant>,
    model: &M,
    cell_ui: &C,
    coord: CellCoord,
) -> String {
    match value {
        Some(Variant::Str(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => cell_ui.text(model, coord).unwrap_or_default(),
    }
}

fn cell_matches<M: TableModel, C: CellUi<M>>(
    model: &M,
    cell_ui: &C,
    coord: CellCoord,
    filter: &ColumnFilter,
    options: &RowOrderOptions,
) -> bool {
    if let Some(matches) = cell_ui.matches(model, coord, filter) {
        return matches;
    }
    let value = model.get(coord);
    let value = value.as_deref();
    let ci = options.filter_case_insensitive;
    match filter {
        ColumnFilter::Contains(needle) => {
            let text = fold_case(cell_text(value, model, cell_ui, coord), ci);
            text.contains(fold_case(needle.clone(), ci).as_str())
        }
        ColumnFilter::OneOf(set) => {
            let eq = |text: &str| {
                set.iter().any(|s| {
                    if ci {
                        s.to_lowercase() == text.to_lowercase()
                    } else {
                        s == text
                    }
                })
            };
            match value {
                Some(Variant::StrList(items)) => items.iter().any(|item| eq(item)),
                _ => eq(&cell_text(value, model, cell_ui, coord)),
            }
        }
        ColumnFilter::Range { min, max } => {
            let Some(value) = value.filter(|v| !is_empty(Some(v))) else {
                return false;
            };
            min.as_ref()
                .is_none_or(|min| compare_values(value, min, options).is_ge())
                && max
                    .as_ref()
                    .is_none_or(|max| compare_values(value, max, options).is_le())
        }
        ColumnFilter::Empty(empty) => {
            let is_empty = match value {
                Some(v) => is_empty(Some(v)),
                None => cell_ui.text(model, coord).is_none_or(|t| t.is_empty()),
            };
            is_empty == *empty
        }
        ColumnFilter::Custom(_) => true,
    }
}

fn is_empty(value: Option<&Variant>) -> bool {
    match value {
        None | Some(Variant::Empty) => true,
        Some(Variant::Str(s)) => s.is_empty(),
        _ => false,
    }
}

/// Order of two non-empty values: text by [`RowOrderOptions::sort_case_insensitive`], dates and
/// date-times by their point in time, anything else by [`Variant::sort_cmp`] (numbers by value
/// across widths, ...).
fn compare_values(a: &Variant, b: &Variant, options: &RowOrderOptions) -> Ordering {
    match (a, b) {
        (Variant::Str(a), Variant::Str(b)) if options.sort_case_insensitive => a
            .to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b)),
        _ => match (date_key(a), date_key(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => a.sort_cmp(b),
        },
    }
}

/// Point in time of a `Date` (midnight UTC) or `DateTime`, so that a column or a filter bound
/// may mix the two.
fn date_key(value: &Variant) -> Option<i64> {
    match value {
        Variant::Date(d) => Some(d.and_time(chrono::NaiveTime::MIN).and_utc().timestamp()),
        Variant::DateTime(dt) => Some(dt.timestamp()),
        _ => None,
    }
}

fn sort_rows<M: TableModel, C: CellUi<M>>(
    model: &M,
    cell_ui: &C,
    rows: Vec<RowUid>,
    sort: &[SortKey],
    options: &RowOrderOptions,
) -> Vec<RowUid> {
    // One row lookup for all key columns, then compare the extracted values.
    let cols: Vec<ColumnUid> = sort.iter().map(|k| k.col).collect();
    let mut keyed: Vec<(RowUid, Vec<Option<Variant>>)> = rows
        .into_iter()
        .map(|row| {
            let mut values = Vec::with_capacity(cols.len());
            model.row_values(row, &cols, &mut values);
            (row, values)
        })
        .collect();
    // `sort_by` is stable: rows equal by all keys keep their natural (or delegated) order.
    keyed.sort_by(|(ra, va), (rb, vb)| {
        for (i, key) in sort.iter().enumerate() {
            let ord = match cell_ui.compare(model, key.col, *ra, *rb) {
                Some(ord) => direct(ord, key.direction),
                None => {
                    let (a, b) = (
                        va.get(i).and_then(Option::as_ref),
                        vb.get(i).and_then(Option::as_ref),
                    );
                    match (is_empty(a), is_empty(b)) {
                        (true, true) => Ordering::Equal,
                        (true, false) if options.empty_last => Ordering::Greater,
                        (false, true) if options.empty_last => Ordering::Less,
                        (true, false) => direct(Ordering::Less, key.direction),
                        (false, true) => direct(Ordering::Greater, key.direction),
                        (false, false) => direct(
                            compare_values(a.unwrap(), b.unwrap(), options),
                            key.direction,
                        ),
                    }
                }
            };
            if ord.is_ne() {
                return ord;
            }
        }
        Ordering::Equal
    });
    keyed.into_iter().map(|(row, _)| row).collect()
}

fn direct(ord: Ordering, direction: SortDirection) -> Ordering {
    match direction {
        SortDirection::Ascending => ord,
        SortDirection::Descending => ord.reverse(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariantCellUi;
    use rvariant::Number;
    use std::borrow::Cow;
    use tabular_core::{Capabilities, ColumnInfo, Revision};

    /// Rows of values; row uid = index. `delegate` makes `sorted_rows` return natural order
    /// reversed, to tell delegated from local sorting.
    struct Rows {
        rows: Vec<Vec<Variant>>,
        columns: Vec<ColumnInfo>,
        delegate: bool,
    }

    impl Rows {
        fn new(rows: Vec<Vec<Variant>>) -> Self {
            let width = rows.first().map_or(0, Vec::len);
            Rows {
                rows,
                columns: (0..width)
                    .map(|c| ColumnInfo::new(format!("c{c}"), None))
                    .collect(),
                delegate: false,
            }
        }
    }

    impl TableModel for Rows {
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
            (0..self.rows.len() as u64).map(RowUid)
        }
        fn get(&self, coord: CellCoord) -> Option<Cow<'_, Variant>> {
            self.rows
                .get(coord.row_uid.0 as usize)?
                .get(coord.col_uid.0 as usize)
                .map(Cow::Borrowed)
        }
        fn sorted_rows(&self, _keys: &[SortKey]) -> Option<Vec<RowUid>> {
            self.delegate
                .then(|| (0..self.rows.len() as u64).rev().map(RowUid).collect())
        }
    }

    fn s(text: &str) -> Variant {
        Variant::Str(text.into())
    }

    fn n(v: u32) -> Variant {
        Variant::Number(Number::U32(v))
    }

    fn order(model: &Rows, sort: &[SortKey], filter: &RowFilter) -> Vec<u64> {
        let cols: Vec<ColumnUid> = model.columns().collect();
        compute(
            model,
            &VariantCellUi,
            &cols,
            sort,
            filter,
            &RowOrderOptions::default(),
        )
        .rows
        .into_iter()
        .map(|r| r.0)
        .collect()
    }

    const C0: ColumnUid = ColumnUid(0);
    const C1: ColumnUid = ColumnUid(1);

    #[test]
    fn no_sort_no_filter_is_natural_order() {
        let m = Rows::new(vec![vec![s("b")], vec![s("a")]]);
        let o = compute(
            &m,
            &VariantCellUi,
            &[C0],
            &[],
            &RowFilter::default(),
            &Default::default(),
        );
        assert_eq!(o.rows, [RowUid(0), RowUid(1)]);
        assert!(o.natural.is_none());
        assert_eq!(o.total, 2);
    }

    #[test]
    fn text_sorts_case_insensitively_with_empty_last() {
        let m = Rows::new(vec![
            vec![s("banana")],
            vec![Variant::Empty],
            vec![s("Apple")],
            vec![s("cherry")],
            vec![s("")],
        ]);
        let f = RowFilter::default();
        assert_eq!(order(&m, &[SortKey::ascending(C0)], &f), [2, 0, 3, 1, 4]);
        assert_eq!(order(&m, &[SortKey::descending(C0)], &f), [3, 0, 2, 1, 4]);
    }

    #[test]
    fn numbers_sort_by_value_across_widths() {
        let m = Rows::new(vec![
            vec![Variant::Number(Number::I64(-3))],
            vec![n(10)],
            vec![Variant::Number(Number::U64(2))],
        ]);
        assert_eq!(
            order(&m, &[SortKey::ascending(C0)], &RowFilter::default()),
            [0, 2, 1]
        );
    }

    #[test]
    fn multi_key_sort_is_stable() {
        let m = Rows::new(vec![
            vec![s("x"), n(2)],
            vec![s("y"), n(1)],
            vec![s("x"), n(1)],
            vec![s("y"), n(1)],
        ]);
        let f = RowFilter::default();
        // Primary c0, secondary c1; rows 1 and 3 are equal by both and keep their order.
        assert_eq!(
            order(&m, &[SortKey::ascending(C0), SortKey::ascending(C1)], &f),
            [2, 0, 1, 3]
        );
        assert_eq!(
            order(&m, &[SortKey::descending(C1), SortKey::ascending(C0)], &f),
            [0, 2, 1, 3]
        );
    }

    #[test]
    fn delegated_sort_is_used_and_filtered() {
        let mut m = Rows::new(vec![vec![s("a")], vec![s("b")], vec![s("c")]]);
        m.delegate = true;
        let mut f = RowFilter::default();
        assert_eq!(order(&m, &[SortKey::ascending(C0)], &f), [2, 1, 0]);
        f.columns
            .insert(C0, ColumnFilter::OneOf(["a".into(), "c".into()].into()));
        assert_eq!(order(&m, &[SortKey::ascending(C0)], &f), [2, 0]);
    }

    #[test]
    fn quick_filter_terms_must_all_match_any_column() {
        let m = Rows::new(vec![
            vec![s("cli"), s("open")],
            vec![s("cli"), s("done")],
            vec![s("dash"), s("Open")],
        ]);
        let f = RowFilter {
            quick: "OPEN cli".into(),
            ..Default::default()
        };
        assert_eq!(order(&m, &[], &f), [0]);
        let f = RowFilter {
            quick: "open".into(),
            ..Default::default()
        };
        assert_eq!(order(&m, &[], &f), [0, 2]);
    }

    #[test]
    fn column_filters() {
        let m = Rows::new(vec![
            vec![s("alpha"), n(1)],
            vec![s(""), n(5)],
            vec![Variant::StrList(vec!["x".into(), "beta".into()]), n(9)],
        ]);
        let only = |col, f| RowFilter {
            columns: [(col, f)].into(),
            ..Default::default()
        };
        assert_eq!(
            order(&m, &[], &only(C0, ColumnFilter::Contains("LPH".into()))),
            [0]
        );
        assert_eq!(
            order(
                &m,
                &[],
                &only(C0, ColumnFilter::OneOf(["Beta".into()].into()))
            ),
            [2]
        );
        assert_eq!(order(&m, &[], &only(C0, ColumnFilter::Empty(true))), [1]);
        assert_eq!(
            order(&m, &[], &only(C0, ColumnFilter::Empty(false))),
            [0, 2]
        );
        let range = ColumnFilter::Range {
            min: Some(n(2)),
            max: Some(Variant::Number(Number::I64(9))),
        };
        assert_eq!(order(&m, &[], &only(C1, range)), [1, 2]);
        // Custom filters need a CellUi hook; without one every row passes.
        assert_eq!(
            order(&m, &[], &only(C1, ColumnFilter::Custom("odd".into()))),
            [0, 1, 2]
        );
        // Filters on columns the model doesn't have are ignored.
        assert_eq!(
            order(&m, &[], &only(ColumnUid(7), ColumnFilter::Empty(true))),
            [0, 1, 2]
        );
    }

    struct OddUi;

    impl CellUi<Rows> for OddUi {
        fn matches(&self, model: &Rows, coord: CellCoord, filter: &ColumnFilter) -> Option<bool> {
            match filter {
                ColumnFilter::Custom(name) if name == "odd" => {
                    let v = model.get(coord)?;
                    Some(matches!(v.as_ref(), Variant::Number(Number::U32(x)) if x % 2 == 1))
                }
                _ => None,
            }
        }

        // Sorts c0 by text length.
        fn compare(&self, model: &Rows, col: ColumnUid, a: RowUid, b: RowUid) -> Option<Ordering> {
            if col != C0 {
                return None;
            }
            let len = |row| {
                model
                    .get((row, col).into())
                    .map_or(0, |v| v.to_string().len())
            };
            Some(len(a).cmp(&len(b)))
        }
    }

    #[test]
    fn cell_ui_hooks_filter_and_compare() {
        let m = Rows::new(vec![
            vec![s("ccc"), n(1)],
            vec![s("a"), n(2)],
            vec![s("bb"), n(3)],
        ]);
        let f = RowFilter {
            columns: [(C1, ColumnFilter::Custom("odd".into()))].into(),
            ..Default::default()
        };
        let rows = |sort: &[SortKey], f: &RowFilter| -> Vec<u64> {
            compute(&m, &OddUi, &[C0, C1], sort, f, &Default::default())
                .rows
                .into_iter()
                .map(|r| r.0)
                .collect()
        };
        assert_eq!(rows(&[], &f), [0, 2]);
        assert_eq!(
            rows(&[SortKey::ascending(C0)], &RowFilter::default()),
            [1, 2, 0]
        );
        assert_eq!(
            rows(&[SortKey::descending(C0)], &RowFilter::default()),
            [0, 2, 1]
        );
    }

    #[test]
    fn click_sort_cycles_and_adds_keys() {
        let mut sort = vec![];
        assert!(click_sort(&mut sort, C0, false));
        assert_eq!(sort, [SortKey::ascending(C0)]);
        click_sort(&mut sort, C0, false);
        assert_eq!(sort, [SortKey::descending(C0)]);
        click_sort(&mut sort, C1, true);
        assert_eq!(sort, [SortKey::descending(C0), SortKey::ascending(C1)]);
        click_sort(&mut sort, C1, true);
        assert_eq!(sort, [SortKey::descending(C0), SortKey::descending(C1)]);
        click_sort(&mut sort, C0, true);
        assert_eq!(sort, [SortKey::descending(C1)]);
        // A plain click on another column replaces the sort.
        click_sort(&mut sort, C0, false);
        assert_eq!(sort, [SortKey::ascending(C0)]);
        click_sort(&mut sort, C0, false);
        assert!(click_sort(&mut sort, C0, false));
        assert!(sort.is_empty());
    }

    #[test]
    fn natural_positions_are_reported() {
        let m = Rows::new(vec![vec![s("b")], vec![s("a")]]);
        let o = compute(
            &m,
            &VariantCellUi,
            &[C0],
            &[SortKey::ascending(C0)],
            &RowFilter::default(),
            &Default::default(),
        );
        let natural = o.natural.unwrap();
        assert_eq!(natural[&RowUid(1)], 1);
        assert_eq!(o.rows, [RowUid(1), RowUid(0)]);
    }
}
