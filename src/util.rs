use crate::CellUi;
use encoding_rs::Encoding;
use rvariant::Variant;
use std::io::{BufReader, Read, Seek, SeekFrom};
use tabular_core::{CellCoord, ColumnUid, RowUid, TableModel};

pub fn base_26(mut num: u32) -> String {
    let mut result = String::new();
    while num > 0 {
        num -= 1; // Adjust for 1-based indexing
        let remainder = (num % 26) as u8;
        let letter = (b'A' + remainder) as char; // Convert to letter A-Z
        result.insert(0, letter); // Prepend letter
        num /= 26;
    }
    result
}

pub fn detect_encoding<R: Read + Seek>(
    rdr: &mut BufReader<R>,
    max_bytes: Option<usize>,
) -> std::io::Result<&'static Encoding> {
    const MAX_CHUNK_SIZE: usize = 1_048_576;
    rdr.seek(SeekFrom::Start(0))?;
    let mut buf = Vec::with_capacity(MAX_CHUNK_SIZE);
    let mut read = 0;
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    loop {
        let n = rdr.read(&mut buf)?;
        if n == 0 {
            break;
        }
        read += n;
        detector.feed(&buf[..n], false); // TODO: correctly pass last=true?
        if let Some(max) = max_bytes {
            if read >= max {
                break;
            }
        }
    }

    let encoding = detector.guess(None, chardetng::Utf8Detection::Allow);
    Ok(encoding)
}

/// Text of the cells `cols` of `row` for copy and export: the model's values, or
/// [`CellUi::text`] where the model has none. Looks the row up once.
pub(crate) fn row_texts<M: TableModel, C: CellUi<M>>(
    model: &M,
    cell_ui: &C,
    row: RowUid,
    cols: &[ColumnUid],
) -> Vec<String> {
    let mut values = Vec::with_capacity(cols.len());
    model.row_values(row, cols, &mut values);
    values
        .into_iter()
        .zip(cols)
        .map(|(value, col)| match value {
            Some(Variant::Str(s)) => s,
            Some(v) => v.to_string(),
            None => cell_ui
                .text(
                    model,
                    CellCoord {
                        row_uid: row,
                        col_uid: *col,
                    },
                )
                .unwrap_or_default(),
        })
        .collect()
}

pub(crate) fn export_csv<M: TableModel, C: CellUi<M>>(model: &M, cell_ui: &C, cols: &[ColumnUid]) {
    let Some(path) = rfd::FileDialog::new().save_file() else {
        return;
    };
    let Ok(mut file) = std::fs::File::create(path) else {
        return;
    };
    let column_names = cols
        .iter()
        .map(|col| model.column(*col).map(|c| c.name.as_str()).unwrap_or(""));
    let mut wtr = csv::Writer::from_writer(&mut file);
    wtr.write_record(column_names).unwrap();
    for row_uid in model.un_skipped_rows() {
        wtr.write_record(row_texts(model, cell_ui, row_uid, cols))
            .unwrap();
    }
}
