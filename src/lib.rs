//! spa-server is a library used to embed all the SPA web application files, and release as a single binary executable.  
//! it based-on [actix-web](https://crates.io/crates/actix-web) and [rust-embed](https://crates.io/crates/rust-embed)
//!  
//! works in proc macro way, example:
//! ```
//! #[derive(SPAServer)]
//! #[spa_server(
//!     static_files = "ui/dist/ui",    # SPA dist dir, all the files will be packad into binary
//!     apis(                           # define apis that SPA application interacting with
//!         api(                        # define a api group
//!             prefix = "/api/v1",     # prefix for this api group
//!             v1::foo,                # api function
//!             v1::bar,
//!         ),
//!         api(
//!             prefix = "/api/v2",
//!             v2::foo,
//!             v2::bar,
//!         ),
//!         api(test),                  # api without prefix
//!     ),
//!     cors,                           # enable cors permissive for debug
//!     identity(name = "a", age = 30)  # identity support, cookie name and age in minutes
//! )]
//! pub struct Server {
//!     data: String,
//!     num:  u32,
//! }
//!    
//! mod v1 {
//!     use spa_server::re_export::*;   # use actix-web symbol directly
//!
//!     #[get("foo")]                   # match route /api/v1/foo
//!     async fn foo(s: web::Data<Server>) -> Result<HttpResponse> {
//!         let data = s.data;          # web context stored in struct
//!         let num = s.num;
//!         Ok(HttpResponse::Ok().finish())
//!     }
//! }
//!
//! #[spa_server::main]                 # replace actix_web::main with spa_server::main
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Server {
//!         data: String::new(), num: 1234
//!     }
//!     .run(8080)                      # listen at 0.0.0.0::8080
//!     .await?;
//!     Ok(())
//! }
//!
//! ```
//! access http://localhost:8080 will show the SPA index.html page

/// re-export all the pub symbols from actix-web, no need to add additional
/// [actix-web](https://crates.io/crates/actix-web) dependency in Cargo.toml.
pub mod re_export {
    pub use actix_cors::*;
    pub use actix_files::*;
    pub use actix_identity::*;
    pub use actix_web::*;
    pub use spa_server_derive::connect;
    pub use spa_server_derive::delete;
    pub use spa_server_derive::get;
    pub use spa_server_derive::head;
    pub use spa_server_derive::main;
    pub use spa_server_derive::options;
    pub use spa_server_derive::patch;
    pub use spa_server_derive::post;
    pub use spa_server_derive::put;
    pub use spa_server_derive::trace;
}

#[doc(hidden)]
pub use include_flate::flate;

/// use spa_server::main replaced actix_web::main
pub use spa_server_derive::main;

/// convert actix_web error to 200 OK and json body
/// ```
/// #[error_to_json]
/// #[get("/index")]
/// async fn test() -> Result<HttpResponse> {
///     Err(ErrorInternalServerError("some error"))
/// }
/// ```
/// will convert to 
/// ```
/// #[get("index")]
/// async fn test() -> Result<HttpResponse> {
///     Ok(HttpResponse::Ok().body(r#"{"errors":[{"detail": "some error"}]}"#))
/// }
/// ```
pub use spa_server_derive::error_to_json;
pub use spa_server_derive::SPAServer;

#[doc(hidden)]
pub use time::Duration;

use log::{debug, warn};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use re_export::*;
use serde::Serialize;
use std::{
    borrow::{Borrow, Cow},
    collections::HashMap,
    env::temp_dir,
    fs::create_dir_all,
    path::{Path, PathBuf},
};

#[doc(hidden)]
#[actix_web::get("/{tail:[^\\.]+}")]
async fn index(path: web::Data<PathBuf>) -> actix_web::Result<NamedFile> {
    Ok(NamedFile::open(path.join("index.html"))?)
}


#[doc(hidden)]
pub fn release_asset<T>() -> Result<PathBuf>
where
    T: Embed,
{
    let target_dir = temp_dir().join(
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>(),
    );
    if !target_dir.exists() {
        create_dir_all(&target_dir)?;
    }

    debug!("release asset target dir: {}", target_dir.to_string_lossy());

    for file in T::iter() {
        match T::get(file.borrow()) {
            None => {
                warn!("assert file {} not found", file);
            }
            Some(p) => {
                if let Some(i) = Path::new(file.as_ref()).parent() {
                    let sub_dir = target_dir.join(i);
                    create_dir_all(sub_dir)?;
                } else {
                    warn!("no parent part for file {}", file);
                    continue;
                }

                let path = target_dir.join(file.as_ref());
                debug!("release asset file: {}", path.to_string_lossy());
                if let Err(e) = std::fs::write(path, p) {
                    warn!("asset file {} write failed: {}", file, e);
                }
            }
        }
    }

    Ok(target_dir)
}
#[doc(hidden)]
pub trait Embed {
    /// Given a relative path from the assets folder, returns the bytes if found.
    ///
    /// If the feature `debug-embed` is enabled or the binary is compiled in
    /// release mode, the bytes have been embeded in the binary and a
    /// `Cow::Borrowed(&'static [u8])` is returned.
    ///
    /// Otherwise, the bytes are read from the file system on each call and a
    /// `Cow::Owned(Vec<u8>)` is returned.
    fn get(file_path: &str) -> Option<Cow<'static, [u8]>>;

    /// Iterates the files in this assets folder.
    ///
    /// If the feature `debug-embed` is enabled or the binary is compiled in
    /// release mode, a static array to the list of relative paths to the files
    /// is used.
    ///
    /// Otherwise, the files are listed from the file system on each call.
    fn iter() -> Filenames;
}
#[doc(hidden)]
pub struct Filenames(pub std::slice::Iter<'static, &'static str>);

impl Iterator for Filenames {
    type Item = Cow<'static, str>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| Cow::from(*x))
    }
}
#[doc(hidden)]
#[derive(Serialize)]
pub struct ErrorMsg {
    errors: Vec<HashMap<String, String>>,
}

#[doc(hidden)]
pub fn quick_err(msg: impl Into<String>) -> ErrorMsg {
    let hm = [("detail".to_string(), msg.into())]
        .iter()
        .cloned()
        .collect();
    ErrorMsg { errors: vec![hm] }
}
