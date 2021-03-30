# spa-server

spa-server is a library used to embed all the SPA web application files, and release as a single binary executable.
it based-on [actix-web](https://crates.io/crates/actix-web) and [rust-embed](https://crates.io/crates/rust-embed)

works in proc macro way, example:
```rust
#[derive(SPAServer)]
#[spa_server(
    static_files = "ui/dist/ui",    # SPA dist dir, all the files will be packad into binary
    apis(                           # define apis that SPA application interacting with
        api(                        # define a api group
            prefix = "/api/v1",     # prefix for this api group
            v1::foo,                # api function
            v1::bar,
        ),
        api(
            prefix = "/api/v2",
            v2::foo,
            v2::bar,
        ),
        api(test),                  # api without prefix
    ),
    cors,                           # enable cors permissive for debug
    identity(name = "a", age = 30)  # identity support, cookie name and age in minutes
)]
pub struct Server {
    data: String,
    num:  u32,
}

mod v1 {
    use spa_server::re_export::*;   # use actix-web symbol directly

    #[get("foo")]                   # match route /api/v1/foo
    async fn foo(s: web::Data<Server>) -> Result<HttpResponse> {
        let data = s.data;          # web context stored in struct
        let num = s.num;
        Ok(HttpResponse::Ok().finish())
    }
}

#[spa_server::main]                 # replace actix_web::main with spa_server::main
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Server {
        data: String::new(), num: 1234
    }
    .run(8080)                      # listen at 0.0.0.0::8080
    .await?;
    Ok(())
}

```
access http://localhost:8080 will show the SPA index.html page
