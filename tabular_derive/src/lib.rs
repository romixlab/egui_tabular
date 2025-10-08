mod tabular_row;

use proc_macro::TokenStream;

#[proc_macro_derive(TabularRow, attributes(format))]
pub fn tabular_row(struct_def: TokenStream) -> TokenStream {
    tabular_row::tabular_row(struct_def.into()).into()
}
