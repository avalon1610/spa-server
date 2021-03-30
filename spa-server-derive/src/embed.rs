use crate::utils;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::path::Path;
use syn::Error;

pub(crate) fn impl_embed(ident: &Ident, folder_path: &str, prefix: Option<&str>) -> TokenStream {
    if !Path::new(&folder_path).exists() {
        Error::new(
            Span::call_site(),
            format!(
                "static_files folder {} doest not exist. cwd: {:?}",
                folder_path,
                std::env::current_dir()
            ),
        )
        .into_compile_error();
    }

    let mut match_values = Vec::new();
    let mut list_values = Vec::new();

    for utils::FileEntry {
        rel_path,
        full_canonical_path,
    } in utils::get_files(folder_path)
    {
        match_values.push(embed_file(&rel_path, &full_canonical_path));
        list_values.push(if let Some(prefix) = prefix {
            format!("{}{}", prefix, rel_path)
        } else {
            rel_path
        });
    }

    let array_len = list_values.len();

    let handle_prefix = if let Some(prefix) = prefix {
        quote! {
            let file_path = file_path.strip_prefix(#prefix)?;
        }
    } else {
        TokenStream::new()
    };

    quote! {
        impl #ident {
            pub fn get(file_path: &str) -> Option<Cow<'static, [u8]>> {
                #handle_prefix
                match file_path.replace("\\", "/").as_str() {
                    #(#match_values)*
                    _ => None,
                }
            }

            fn names() -> std::slice::Iter<'static, &'static str> {
                const ITEMS: [&str; #array_len] = [#(#list_values),*];
                ITEMS.iter()
            }

            pub fn iter() -> impl Iterator<Item = Cow<'static, str>> {
                Self::names().map(|x| Cow::from(*x))
            }
        }

        impl Embed for #ident {
            fn get(file_path: &str) -> Option<Cow<'static, [u8]>> {
                #ident::get(file_path)
            }

            fn iter() -> Filenames {
                Filenames(#ident::names())
            }
        }
    }
}

fn embed_file(rel_path: &str, full_canonical_path: &str) -> TokenStream {
    quote! {
        #rel_path => {
            spa_server::flate!(static FILE: [u8] from #full_canonical_path);

            let bytes = &FILE[..];
            Some(Cow::from(bytes))
        }
    }
}
