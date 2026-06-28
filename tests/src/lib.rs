#[cfg(test)]
mod tests {
    use tabular_derive::TabularRow;

    #[test]
    fn tabular_row_compiles() {
        #[derive(TabularRow)]
        struct Section {
            name: String,
            #[format = "0x{:08x}"]
            address: u64,
            size: u64,
            index: usize,
        }
    }
}
