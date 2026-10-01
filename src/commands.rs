//! Changes the view makes to a model. The view queues them while handling input and rendering,
//! and applies them in one place, never in the middle of rendering.

use rvariant::Variant;
use std::borrow::Cow;
use tabular_core::{CellCoord, ColumnUid, ModelError, RowPosition, RowUid, TableModel};

#[derive(Clone, Debug, PartialEq)]
pub enum TableCommand {
    Set {
        coord: CellCoord,
        value: Variant,
    },
    /// Write a block of text into `rows` × `columns`, after creating `create_rows` rows at the
    /// end and `create_columns` columns. With `repeat`, the block is repeated to fill the
    /// target; otherwise extra block cells are dropped.
    Paste {
        rows: Vec<RowUid>,
        create_rows: usize,
        columns: Vec<ColumnUid>,
        create_columns: usize,
        block: Vec<Vec<String>>,
        repeat: bool,
    },
    CreateRows {
        at: RowPosition,
        count: usize,
    },
    CreateColumn,
    RemoveRows(Vec<RowUid>),
    RemoveColumns(Vec<ColumnUid>),
    Clear,
    SkipRows {
        rows: Vec<RowUid>,
        skipped: bool,
    },
    SkipColumn {
        col: ColumnUid,
        skipped: bool,
    },
    /// Applied in order. Inverses of compound commands are batches.
    Batch(Vec<TableCommand>),
}

/// What applying a command did.
#[derive(Default, Debug)]
pub struct Applied {
    /// Applying this restores the previous state. `None` if the command can't be undone
    /// (removing rows or columns, clear).
    pub inverse: Option<TableCommand>,
    pub created_rows: Vec<RowUid>,
    pub created_columns: Vec<ColumnUid>,
}

impl TableCommand {
    /// Apply to `model`. On error the command may be partially applied.
    pub fn apply<M: TableModel>(&self, model: &mut M) -> Result<Applied, ModelError> {
        let mut applied = Applied::default();
        applied.inverse = match self {
            TableCommand::Set { coord, value } => {
                let old = old_value(model, *coord);
                model.set(*coord, value.clone())?;
                Some(TableCommand::Set {
                    coord: *coord,
                    value: old,
                })
            }
            TableCommand::Paste {
                rows,
                create_rows,
                columns,
                create_columns,
                block,
                repeat,
            } => {
                let mut columns = columns.clone();
                for _ in 0..*create_columns {
                    let col = model.create_column()?;
                    applied.created_columns.push(col);
                    columns.push(col);
                }
                let mut rows = rows.clone();
                for _ in 0..*create_rows {
                    let row = model.create_row(RowPosition::Append, vec![])?;
                    applied.created_rows.push(row);
                    rows.push(row);
                }
                let mut restore = vec![];
                let mut write = |row: RowUid, col: ColumnUid, text: &String| {
                    let coord = CellCoord {
                        row_uid: row,
                        col_uid: col,
                    };
                    let is_new = applied.created_rows.contains(&row)
                        || applied.created_columns.contains(&col);
                    if !is_new {
                        restore.push(TableCommand::Set {
                            coord,
                            value: old_value(model, coord),
                        });
                    }
                    model.set(coord, Variant::Str(text.clone()))
                };
                if *repeat {
                    for (row, line) in rows.iter().zip(block.iter().cycle()) {
                        for (col, text) in columns.iter().zip(line.iter().cycle()) {
                            write(*row, *col, text)?;
                        }
                    }
                } else {
                    for (row, line) in rows.iter().zip(block) {
                        for (col, text) in columns.iter().zip(line) {
                            write(*row, *col, text)?;
                        }
                    }
                }
                if !applied.created_rows.is_empty() {
                    restore.push(TableCommand::RemoveRows(applied.created_rows.clone()));
                }
                if !applied.created_columns.is_empty() {
                    restore.push(TableCommand::RemoveColumns(applied.created_columns.clone()));
                }
                Some(TableCommand::Batch(restore))
            }
            TableCommand::CreateRows { at, count } => {
                let mut at = *at;
                for _ in 0..*count {
                    let row = model.create_row(at, vec![])?;
                    applied.created_rows.push(row);
                    // Keep the new rows in creation order.
                    if let RowPosition::After(_) = at {
                        at = RowPosition::After(row);
                    }
                }
                Some(TableCommand::RemoveRows(applied.created_rows.clone()))
            }
            TableCommand::CreateColumn => {
                let col = model.create_column()?;
                applied.created_columns.push(col);
                Some(TableCommand::RemoveColumns(vec![col]))
            }
            TableCommand::RemoveRows(rows) => {
                model.remove_rows(rows)?;
                None
            }
            TableCommand::RemoveColumns(cols) => {
                model.remove_columns(cols)?;
                None
            }
            TableCommand::Clear => {
                model.clear()?;
                None
            }
            TableCommand::SkipRows { rows, skipped } => {
                let changed: Vec<RowUid> = rows
                    .iter()
                    .copied()
                    .filter(|row| model.is_row_skipped(*row) != *skipped)
                    .collect();
                model.skip_rows(rows, *skipped)?;
                Some(TableCommand::SkipRows {
                    rows: changed,
                    skipped: !*skipped,
                })
            }
            TableCommand::SkipColumn { col, skipped } => {
                let was = model.column(*col).ok_or(ModelError::NotFound)?.is_skipped;
                model.skip_column(*col, *skipped)?;
                Some(TableCommand::SkipColumn {
                    col: *col,
                    skipped: was,
                })
            }
            TableCommand::Batch(commands) => {
                let mut inverses = Some(vec![]);
                for command in commands {
                    let mut a = command.apply(model)?;
                    applied.created_rows.append(&mut a.created_rows);
                    applied.created_columns.append(&mut a.created_columns);
                    inverses = inverses.zip(a.inverse).map(|(mut all, inverse)| {
                        all.push(inverse);
                        all
                    });
                }
                inverses.map(|mut all| {
                    all.reverse();
                    TableCommand::Batch(all)
                })
            }
        };
        Ok(applied)
    }
}

/// Value to restore. A model without a value for the cell gets `Empty` back.
fn old_value<M: TableModel>(model: &M, coord: CellCoord) -> Variant {
    model
        .get(coord)
        .map(Cow::into_owned)
        .unwrap_or(Variant::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::variant::{ColumnDef, VariantTable};
    use rvariant::VariantTy;

    fn table() -> (VariantTable, Vec<RowUid>) {
        let mut t = VariantTable::new([
            ColumnDef::new("A", VariantTy::Str),
            ColumnDef::new("B", VariantTy::u32()),
        ]);
        let rows = (0..3u32)
            .map(|i| {
                t.insert_row([
                    (ColumnUid(0), Variant::Str(format!("a{i}"))),
                    (ColumnUid(1), Variant::u32(i)),
                ])
            })
            .collect();
        (t, rows)
    }

    /// Every cell, row by row, as text.
    fn snapshot(t: &VariantTable) -> Vec<Vec<String>> {
        t.rows()
            .map(|row| {
                t.columns()
                    .map(|col| {
                        t.get(CellCoord {
                            row_uid: row,
                            col_uid: col,
                        })
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                    })
                    .collect()
            })
            .collect()
    }

    fn assert_inverse_restores(command: TableCommand) {
        let (mut t, _) = table();
        let before = snapshot(&t);
        let applied = command.apply(&mut t).unwrap();
        assert_ne!(snapshot(&t), before, "{command:?} changed nothing");
        applied.inverse.unwrap().apply(&mut t).unwrap();
        assert_eq!(snapshot(&t), before);
    }

    #[test]
    fn set_inverse_restores() {
        let (_, rows) = table();
        assert_inverse_restores(TableCommand::Set {
            coord: (rows[1], ColumnUid(0)).into(),
            value: Variant::Str("new".into()),
        });
    }

    #[test]
    fn paste_inverse_restores() {
        let (_, rows) = table();
        assert_inverse_restores(TableCommand::Paste {
            rows: rows[1..].to_vec(),
            create_rows: 1,
            columns: vec![ColumnUid(1)],
            create_columns: 1,
            block: vec![
                vec!["7".into(), "x".into()],
                vec!["8".into(), "y".into()],
                vec!["9".into(), "z".into()],
            ],
            repeat: false,
        });
    }

    #[test]
    fn create_rows_inverse_restores() {
        let (_, rows) = table();
        assert_inverse_restores(TableCommand::CreateRows {
            at: RowPosition::After(rows[0]),
            count: 2,
        });
    }

    #[test]
    fn create_rows_after_keeps_creation_order() {
        let (mut t, rows) = table();
        let applied = TableCommand::CreateRows {
            at: RowPosition::After(rows[0]),
            count: 2,
        }
        .apply(&mut t)
        .unwrap();
        let order: Vec<RowUid> = t.rows().collect();
        assert_eq!(
            order,
            [
                rows[0],
                applied.created_rows[0],
                applied.created_rows[1],
                rows[1],
                rows[2]
            ]
        );
    }

    #[test]
    fn skip_inverse_restores_only_changed_rows() {
        let (mut t, rows) = table();
        t.skip_rows(&rows[..1], true).unwrap();
        let applied = TableCommand::SkipRows {
            rows: rows.clone(),
            skipped: true,
        }
        .apply(&mut t)
        .unwrap();
        applied.inverse.unwrap().apply(&mut t).unwrap();
        let skipped: Vec<bool> = rows.iter().map(|r| t.is_row_skipped(*r)).collect();
        assert_eq!(skipped, [true, false, false]);
    }

    #[test]
    fn paste_converts_to_column_type() {
        let (mut t, rows) = table();
        TableCommand::Paste {
            rows: vec![rows[0]],
            create_rows: 0,
            columns: vec![ColumnUid(1)],
            create_columns: 0,
            block: vec![vec!["42".into()]],
            repeat: false,
        }
        .apply(&mut t)
        .unwrap();
        let value = t.get((rows[0], ColumnUid(1)).into()).unwrap();
        assert_eq!(*value, Variant::u32(42));
    }
}
