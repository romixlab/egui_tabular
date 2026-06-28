use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{ItemStruct, LitInt, Meta};

pub(super) fn tabular_row(struct_def: TokenStream) -> TokenStream {
    let struct_def = match syn::parse2::<ItemStruct>(struct_def) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error().into(),
    };

    let backend_ident = Ident::new(
        format!("{}TabularBackend", struct_def.ident).as_str(),
        struct_def.ident.span(),
    );
    let backend_struct_def = backend_struct(&backend_ident, &struct_def.ident);
    let impl_new = impl_new(&struct_def, &backend_ident);
    let impl_backend = impl_table_backend(&struct_def, &backend_ident);
    let impl_frontend = impl_table_frontend(&struct_def, &backend_ident);

    quote! {
        #backend_struct_def
        #impl_new
        #impl_backend
        #impl_frontend
    }
}

fn backend_struct(backend_ident: &Ident, row_ident: &Ident) -> TokenStream {
    quote! {
        struct #backend_ident {
            data: Vec<#row_ident>,

            persistent_flags: egui_tabular::PersistentFlags,
            one_shot_flags: egui_tabular::OneShotFlags,
            one_shot_flags_delay: egui_tabular::OneShotFlags,
            columns: Vec<(egui_tabular::ColumnUid, egui_tabular::BackendColumn)>,
        }
    }
}

fn impl_new(struct_def: &ItemStruct, backend_ident: &Ident) -> TokenStream {
    let row_ident = &struct_def.ident;
    let columns = struct_def.fields.iter().enumerate().map(|(idx, field)| {
        let name = field
            .ident
            .as_ref()
            .map(|ident| ident.to_string().to_case(Case::Sentence))
            .unwrap_or(String::new());
        let ty = &field.ty;
        let ty = quote! { #ty }.to_string();
        let id = usize_to_lit(idx, field.ident.as_ref().map(|i| i.span()));
        quote! {
            (egui_tabular::ColumnUid(#id), egui_tabular::BackendColumn::new(#name, #ty))
        }
    });
    quote! {
        impl #backend_ident {
            pub fn new(data: Vec<#row_ident>) -> Self {
                Self {
                    data,
                    persistent_flags: egui_tabular::PersistentFlags {
                        is_read_only: true,
                        are_cols_skippable: false,
                        are_rows_skippable: false,
                        column_info_present: true,
                        row_set_present: true,
                        ..egui_tabular::PersistentFlags::default()
                    },
                    one_shot_flags: Default::default(),
                    one_shot_flags_delay: Default::default(),
                    columns: vec![#(#columns),*],
                }
            }
        }
    }
}

fn impl_table_backend(_struct_def: &ItemStruct, backend_ident: &Ident) -> TokenStream {
    quote! {
        impl egui_tabular::TableBackend for #backend_ident {
            fn clear(&mut self) {
                self.one_shot_flags.cleared = true;
            }

            fn is_clearable(&self) -> bool {
                false
            }

            fn persistent_flags(&self) -> &egui_tabular::PersistentFlags {
                &self.persistent_flags
            }

            fn one_shot_flags(&self) -> &egui_tabular::OneShotFlags {
                &self.one_shot_flags_delay
            }

            fn one_shot_flags_internal(&self) -> &egui_tabular::OneShotFlags {
                &self.one_shot_flags
            }

            fn one_shot_flags_archive(&mut self) {
                self.one_shot_flags_delay = self.one_shot_flags.clone();
            }

            fn one_shot_flags_internal_mut(&mut self) -> &mut egui_tabular::OneShotFlags {
                &mut self.one_shot_flags
            }

            fn available_columns(&self) -> impl Iterator<Item = egui_tabular::ColumnUid> {
                self.columns.iter().map(|(uid, _)| *uid)
            }

            fn column_info(&self, col_uid: egui_tabular::ColumnUid) -> Option<&egui_tabular::BackendColumn> {
                self.columns
                    .iter()
                    .find(|(uid, _)| *uid == col_uid)
                    .map(|(_, b)| b)
            }

            fn col_uid(&self, _col_idx: egui_tabular::VisualColIdx) -> Option<egui_tabular::ColumnUid> {
                None
            }

            fn row_count(&self) -> usize {
                self.data.len()
            }

            fn row_uid(&self, row_idx: egui_tabular::VisualRowIdx) -> Option<egui_tabular::RowUid> {
                Some(egui_tabular::RowUid(row_idx.0 as u32))
            }

            fn rows(&self) -> impl Iterator<Item = egui_tabular::RowUid> {
                (0..self.row_count() as u32).map(egui_tabular::RowUid)
            }
        }
    }
}

fn impl_table_frontend(struct_def: &ItemStruct, backend_ident: &Ident) -> TokenStream {
    let handle_col_ids = struct_def.fields.iter().enumerate().map(|(idx, field)| {
        let id = usize_to_lit(idx, field.ident.as_ref().map(|i| i.span()));
        let field_name = field
            .ident
            .as_ref()
            .map(|ident| ident.clone())
            .unwrap_or(Ident::new(format!("_{idx}").as_str(), Span::call_site()));
        let format_str = field
            .attrs
            .iter()
            .find_map(|attr| {
                let Meta::NameValue(name_value) = &attr.meta else {
                    return None;
                };
                if name_value.path.is_ident("format") {
                    let mut tokens = TokenStream::new();
                    name_value.value.to_tokens(&mut tokens);
                    Some(tokens.to_string())
                } else {
                    None
                }
            })
            .unwrap_or("{:?}".to_owned());
        quote! {
            #id => {
                ui.label(format!(#format_str, row.#field_name));
            }
        }
    });
    quote! {
        impl egui_tabular::TableFrontend for #backend_ident {
            fn show_cell_view(&mut self, coord: egui_tabular::CellCoord, ui: &mut egui::Ui, _id: egui::Id) {
                let col: u32 = coord.col_uid.0;
                let row_idx: usize = coord.row_uid.0 as usize;
                let Some(row) = self.data.get(row_idx) else {
                    return;
                };

                match col {
                    #(#handle_col_ids),*
                    _ => {}
                }
            }
        }
    }
}

fn usize_to_lit(num: usize, span: Option<Span>) -> LitInt {
    LitInt::new(format!("{num}").as_str(), span.unwrap_or(Span::call_site()))
}
