//! `encore-server`: run with `cargo run` in `server/`.
//! Set `ENCORE_SERVER_BIND` to listen somewhere other than 127.0.0.1:8097.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let addr =
        match encore_server::bind_address(std::env::var(encore_server::BIND_ENV).ok().as_deref()) {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("encore-server: {e}");
                return ExitCode::from(2);
            }
        };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("encore-server: cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "encore-server {} listening on http://{addr}",
        env!("CARGO_PKG_VERSION")
    );
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    match axum::serve(listener, encore_server::app())
        .with_graceful_shutdown(shutdown)
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("encore-server: {e}");
            ExitCode::FAILURE
        }
    }
}
