#[cfg(test)]
mod tests {
    use egui_tabular::rvariant::Variant;
    use egui_tabular::{CellCoord, ColumnUid, RowUid, TableModel};

    mod rows {
        use tabular_derive::TabularRow;

        #[derive(Debug)]
        #[allow(dead_code)] // only read through Debug
        pub struct Flags(pub u8);

        // `pub` so that the generated table is usable from the parent module (DERIVE-2).
        #[derive(TabularRow)]
        pub struct Section {
            pub name: String,
            #[format = "0x{:08x}"]
            pub address: u64,
            pub size: u64,
            pub index: usize,
            pub flags: Flags,
        }

        #[derive(TabularRow)]
        pub struct Pair(pub String, pub u32);
    }
    use rows::*;

    fn cell(model: &impl TableModel, row: u64, col: u32) -> Option<Variant> {
        model
            .get(CellCoord {
                row_uid: RowUid(row),
                col_uid: ColumnUid(col),
            })
            .map(|v| v.into_owned())
    }

    fn sections() -> SectionTable {
        SectionTable::new(vec![Section {
            name: ".text".into(),
            address: 0x1000,
            size: 64,
            index: 3,
            flags: Flags(5),
        }])
    }

    #[test]
    fn columns_are_named_after_fields() {
        let t = sections();
        let names: Vec<&str> = t
            .columns()
            .map(|c| t.column(c).unwrap().name.as_str())
            .collect();
        assert_eq!(names, ["Name", "Address", "Size", "Index", "Flags"]);
        assert_eq!(
            t.column(ColumnUid(2)).unwrap().type_label.as_deref(),
            Some("u64")
        );
    }

    #[test]
    fn values_use_into_variant_or_debug() {
        let t = sections();
        assert_eq!(cell(&t, 0, 0), Some(Variant::Str(".text".into())));
        assert_eq!(cell(&t, 0, 2), Some(Variant::u64(64)));
        // No `Into<Variant>` for these: Debug text.
        assert_eq!(cell(&t, 0, 3), Some(Variant::Str("3".into())));
        assert_eq!(cell(&t, 0, 4), Some(Variant::Str("Flags(5)".into())));
        assert_eq!(cell(&t, 1, 0), None);
    }

    #[test]
    fn format_attribute_has_no_quotes() {
        // DERIVE-1
        assert_eq!(
            cell(&sections(), 0, 1),
            Some(Variant::Str("0x00001000".into()))
        );
    }

    #[test]
    fn tuple_structs_use_field_indices() {
        // DERIVE-5
        let t = PairTable::new(vec![Pair("a".into(), 7)]);
        assert_eq!(cell(&t, 0, 0), Some(Variant::Str("a".into())));
        assert_eq!(cell(&t, 0, 1), Some(Variant::u32(7)));
    }

    #[test]
    fn row_values_match_get() {
        let t = sections();
        let cols: Vec<ColumnUid> = t.columns().collect();
        let mut out = vec![];
        t.row_values(RowUid(0), &cols, &mut out);
        let expected: Vec<Option<Variant>> = (0..5).map(|c| cell(&t, 0, c)).collect();
        assert_eq!(out, expected);
    }
}
