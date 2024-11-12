pub struct TableViewConfig {
    pub minimum_row_height: f32,
}

impl Default for TableViewConfig {
    fn default() -> Self {
        TableViewConfig {
            minimum_row_height: 15.0,
        }
    }
}