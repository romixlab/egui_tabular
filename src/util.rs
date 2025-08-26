use encoding_rs::Encoding;
use std::io::{BufReader, Read, Seek, SeekFrom};
use tabular_core::backend::TableBackend;

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
    let mut detector = chardetng::EncodingDetector::new();
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

    let encoding = detector.guess(None, true);
    Ok(encoding)
}

pub fn export_csv(table: &impl TableBackend) {
    let Some(path) = rfd::FileDialog::new().save_file() else {
        return;
    };
    let Ok(mut file) = std::fs::File::create(path) else {
        return;
    };
    let mut column_names = vec![];
    for col_uid in table.used_columns() {
        let Some(col) = table.column_info(col_uid) else {
            continue
        };
        column_names.push(col.name.as_str());
    }
    let mut wtr = csv::Writer::from_writer(&mut file);
    wtr.write_record(column_names).unwrap();
    for row_uid in table.un_skipped_rows() {
        let mut record = vec![];
        for col_uid in table.used_columns() {
            if let Some(cell) = table.get((row_uid, col_uid).into()) {
                record.push(cell.to_string());
            } else {
                record.push(String::new());
            }
        }
        wtr.write_record(&record).unwrap();
    }
}