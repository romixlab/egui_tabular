pub struct TableViewConfig {
    /// Row height will not be lower that this value.
    pub minimum_row_height: f32,
    /// Row height will be determined based on its contents.
    /// There might be some speed and memory penalty for doing this.
    pub use_heterogeneous_row_heights: bool,
}

impl Default for TableViewConfig {
    fn default() -> Self {
        TableViewConfig {
            minimum_row_height: 15.0,
            use_heterogeneous_row_heights: true,
        }
    }
}

impl super::TableView {
    pub fn config_mut(&mut self) -> &mut TableViewConfig {
        &mut self.config
    }
}
