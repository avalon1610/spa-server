use proc_macro2::Span;
use std::{
    fmt::Display,
    fs::canonicalize,
    path::{Path, MAIN_SEPARATOR},
    str::FromStr,
};
use syn::{Error, Lit, Result};

pub struct FileEntry {
    pub rel_path: String,
    pub full_canonical_path: String,
}

pub fn get_files(folder_path: impl Into<String>) -> impl Iterator<Item = FileEntry> {
    let folder_path = folder_path.into();
    walkdir::WalkDir::new(&folder_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(move |e| {
            let rel_path = path_to_str(e.path().strip_prefix(&folder_path).unwrap());
            let full_canonical_path =
                path_to_str(canonicalize(e.path()).expect("Could not get canonicalize path"));

            let rel_path = if MAIN_SEPARATOR == '\\' {
                rel_path.replace('\\', "/")
            } else {
                rel_path
            };

            FileEntry {
                rel_path,
                full_canonical_path,
            }
        })
}

fn path_to_str<P: AsRef<Path>>(p: P) -> String {
    p.as_ref()
        .to_str()
        .expect("Path does not have a string representation")
        .to_owned()
}

pub(crate) struct LitWrap<'a> {
    pub inner: &'a Lit,
}

impl<'a> LitWrap<'a> {
    pub fn parse<T: FromLit>(&self) -> Result<T> {
        T::from_lit(self.inner)
    }
}

pub(crate) trait FromLit {
    fn from_lit(lit: &Lit) -> Result<Self>
    where
        Self: Sized;
}

impl FromLit for bool {
    fn from_lit(lit: &Lit) -> Result<Self> {
        if let Lit::Bool(b) = lit {
            Ok(b.value)
        } else {
            Err(Error::new(Span::call_site(), "parse to bool failed"))
        }
    }
}

impl FromLit for String {
    fn from_lit(lit: &Lit) -> Result<Self> {
        if let Lit::Str(s) = lit {
            Ok(s.value())
        } else {
            Err(Error::new(Span::call_site(), "parse to string failed"))
        }
    }
}

pub(crate) trait Integer {}

macro_rules! impl_integer {
    ($int: ident) => {
        impl Integer for $int {}
    };
}
impl_integer!(u8);
impl_integer!(u16);
impl_integer!(u32);
impl_integer!(u64);
impl_integer!(i8);
impl_integer!(i16);
impl_integer!(i32);
impl_integer!(i64);

impl<N> FromLit for N
where
    N: Integer + FromStr,
    N::Err: Display,
{
    fn from_lit(lit: &Lit) -> Result<Self> {
        if let Lit::Int(i) = lit {
            i.base10_parse()
        } else {
            Err(Error::new(Span::call_site(), "parse to integer failed"))
        }
    }
}
