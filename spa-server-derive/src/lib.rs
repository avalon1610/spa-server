mod embed;
mod utils;

use embed::impl_embed;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, DeriveInput, Error, FnArg, Meta, NestedMeta, Pat, Path,
    Result,
};
use utils::{FromLit, LitWrap};

#[proc_macro_derive(SPAServer, attributes(spa_server))]
pub fn derive_spa_server(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

fn get_name_value<'a, T, I, S>(metas: I, key: S) -> Option<T>
where
    T: FromLit,
    I: Iterator<Item = &'a Meta>,
    S: AsRef<str>,
{
    for m in metas {
        if let Meta::NameValue(nv) = m {
            if let Some(ident) = nv.path.get_ident() {
                if ident == key.as_ref() {
                    let lw = LitWrap { inner: &nv.lit };
                    if let Ok(r) = lw.parse() {
                        return Some(r);
                    }
                }
            }
        }
    }

    None
}

fn get_path<'a>(metas: impl Iterator<Item = &'a Meta>, key: impl AsRef<str>) -> bool {
    for m in metas {
        if let Meta::Path(p) = m {
            if let Some(ident) = p.get_ident() {
                if ident == key.as_ref() {
                    return true;
                }
            }
        }
    }

    false
}

fn expand(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let attrs = &input.attrs;
    let mut opt = Options::default();
    for attr in attrs {
        if let Meta::List(l) = attr.parse_meta()? {
            if let Some(id) = l.path.get_ident() {
                if id != "spa_server" {
                    return Err(Error::new(l.span(), "only support attribute spa_server"));
                }
            }

            let metas = l.nested.iter().filter_map(|x| {
                if let NestedMeta::Meta(m) = x {
                    Some(m)
                } else {
                    None
                }
            });

            opt.static_files = get_name_value(metas.clone(), "static_files").ok_or(Error::new(
                Span::call_site(),
                "must set static files path in attribute",
            ))?;

            opt.cors = get_path(metas.clone(), "cors");
            if !opt.cors {
                if let Some(cors) = get_name_value(metas.clone(), "cors") {
                    opt.cors = cors;
                }
            }

            for m in metas {
                match m {
                    Meta::List(l) => {
                        if let Some(id) = l.path.get_ident() {
                            if id == "apis" {
                                for api in &l.nested {
                                    if let NestedMeta::Meta(meta) = api {
                                        if let Meta::List(pl) = meta {
                                            if let Some(iid) = pl.path.get_ident() {
                                                if iid == "api" {
                                                    let mut api_path = Vec::new();
                                                    let mut prefix = None;
                                                    for ppl in &pl.nested {
                                                        if let NestedMeta::Meta(mm) = ppl {
                                                            match mm {
                                                                Meta::Path(p) => {
                                                                    api_path.push(p.clone())
                                                                }
                                                                Meta::NameValue(nv) => {
                                                                    if let Some(iiid) =
                                                                        nv.path.get_ident()
                                                                    {
                                                                        if iiid == "prefix" {
                                                                            let lw = LitWrap {
                                                                                inner: &nv.lit,
                                                                            };
                                                                            if let Ok(r) =
                                                                                lw.parse::<String>()
                                                                            {
                                                                                prefix = Some(r);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                    }

                                                    opt.apis.push(Api {
                                                        path: api_path,
                                                        prefix,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if id == "identity" {
                                let mut identity = Identity::default();
                                for nm in &l.nested {
                                    if let NestedMeta::Meta(meta) = nm {
                                        if let Meta::NameValue(nv) = meta {
                                            if let Some(iid) = nv.path.get_ident() {
                                                if iid == "name" {
                                                    let lit = LitWrap { inner: &nv.lit };
                                                    if let Ok(name) = lit.parse() {
                                                        identity.name = name;
                                                    }
                                                } else if iid == "age" {
                                                    let lit = LitWrap { inner: &nv.lit };
                                                    if let Ok(age) = lit.parse() {
                                                        identity.age = age;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if !identity.name.is_empty() && identity.age != 0 {
                                    opt.identity = Some(identity);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut services = Vec::new();
    for api in opt.apis {
        let api_list = api.path;
        match api.prefix {
            Some(p) => {
                services.push(quote! {
                    .service(
                        web::scope(#p)
                        #(.service(#api_list))*
                        .app_data(data.clone())
                    )
                });
            }
            None => {
                services.push(quote! {
                    #(.service(#api_list))*
                    .app_data(data.clone())
                });
            }
        }
    }

    let cors = if opt.cors {
        quote! { .wrap(spa_server::re_export::Cors::permissive()) }
    } else {
        TokenStream::new()
    };

    let identity = if let Some(id) = opt.identity {
        let name = id.name;
        let age = id.age;
        quote! {
            .wrap(spa_server::re_export::IdentityService::new(
                spa_server::re_export::CookieIdentityPolicy::new(&[0; 32])
                    .name(#name)
                    .max_age_time(spa_server::Duration::minutes(#age))
                    .http_only(true)
                    .secure(false)
            ))
        }
    } else {
        TokenStream::new()
    };

    let embed_tokens = impl_embed(name, &opt.static_files, None);

    Ok(quote! {
        use spa_server::re_export::{
            App, HttpServer, rt::System, web, Files
        };
        use spa_server::{Embed, Filenames};
        use std::borrow::Cow;

        impl #name {
            pub async fn run(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
                let root_path = spa_server::release_asset::<#name>()?;
                let data = web::Data::new(self);

                HttpServer::new(move || {
                    App::new()
                        #identity
                        #cors
                        #(#services)*
                        .data(root_path.clone())
                        .service(spa_server::index)
                        .service(Files::new("/", root_path.clone()).index_file("index.html"))
                })
                .bind(format!("0.0.0.0:{}", port))?
                .run()
                .await?;

                Ok(())
            }
        }

        #embed_tokens
    })
}

#[derive(Default)]
struct Options {
    apis: Vec<Api>,
    static_files: String,
    cors: bool,
    identity: Option<Identity>,
}

#[derive(Default)]
struct Api {
    path: Vec<Path>,
    prefix: Option<String>,
}

#[allow(dead_code)]
#[derive(Default)]
struct Identity {
    name: String,
    age: i64,
}

#[proc_macro_attribute]
pub fn main(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &mut input.sig;
    let body = &input.block;

    if sig.asyncness.is_none() {
        return Error::new_spanned(sig.fn_token, "only async fn is supported")
            .to_compile_error()
            .into();
    }

    sig.asyncness = None;

    (quote! {
        #(#attrs)*
        #vis #sig {
            spa_server::re_export::rt::System::new()
                .block_on(async move { #body })
        }
    })
    .into()
}

mod route;

macro_rules! method_macro {
    (
        $($variant:ident, $method:ident,)+
    ) => {
        $(
            #[proc_macro_attribute]
            pub fn $method(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
                route::with_method(Some(route::MethodType::$variant), args, input)
            }
        )+
    };
}

method_macro! {
    Get,       get,
    Post,      post,
    Put,       put,
    Delete,    delete,
    Head,      head,
    Connect,   connect,
    Options,   options,
    Trace,     trace,
    Patch,     patch,
}

#[proc_macro_attribute]
pub fn error_to_json(
    _: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::ItemFn);
    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &mut input.sig;
    let body = &input.block;

    let mut sig_impl = sig.clone();
    sig_impl.ident = syn::Ident::new(&format!("_{}_impl", sig.ident), sig.span());
    let sig_impl_ident = &sig_impl.ident;

    let mut args = Vec::new();
    for i in &sig.inputs {
        if let FnArg::Typed(p) = i {
            if let Pat::Ident(id) = &*p.pat {
                args.push(id.ident.clone());
            }
        }
    }

    (quote! {
        #[allow(unused_mut)]
        #(#attrs)*
        #vis #sig {
            Ok(match #sig_impl_ident(#(#args),*).await {
                Ok(a) => a,
                Err(e) => spa_server::re_export::HttpResponse::Ok().json(spa_server::quick_err(format!("{:?}", e)))
            })
        }

        #vis #sig_impl {
            #body
        }
    })
    .into()
}
