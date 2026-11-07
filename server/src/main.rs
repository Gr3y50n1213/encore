//! `encore-server`: the companion server.
//!
//! - `encore-server` serves. Settings come from the environment:
//!   `ENCORE_JELLYFIN_URLS` (required), `ENCORE_SERVER_BIND` (127.0.0.1:8097),
//!   `ENCORE_DATA_DIR` (/data).
//! - `encore-server backup <file>` writes a consistent copy of the database, even
//!   while the server runs.
//! - `encore-server health` asks the running server's `/health` (the container's
//!   health check; the image has no curl).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use encore_server::{AppState, Config, JellyfinCheck};

fn config() -> Result<Config, String> {
    Config::from_env(|k| std::env::var(k).ok())
}

fn health(bind: SocketAddr) -> ExitCode {
    // Ask over loopback when the server listens on every address.
    let addr = if bind.ip().is_unspecified() {
        SocketAddr::from(([127, 0, 0, 1], bind.port()))
    } else {
        bind
    };
    let reply = (|| -> std::io::Result<String> {
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3))?;
        s.set_read_timeout(Some(Duration::from_secs(3)))?;
        write!(
            s,
            "GET /health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        )?;
        let mut out = String::new();
        s.read_to_string(&mut out)?;
        Ok(out)
    })();
    match reply {
        Ok(r) if r.starts_with("HTTP/1.1 200") => ExitCode::SUCCESS,
        Ok(r) => {
            eprintln!(
                "encore-server: unhealthy: {}",
                r.lines().next().unwrap_or("")
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("encore-server: no answer on {addr}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The health check needs only the address, not the rest of the settings.
    if args.first().map(String::as_str) == Some("health") {
        return match encore_server::bind_address(
            std::env::var(encore_server::BIND_ENV).ok().as_deref(),
        ) {
            Ok(bind) => health(bind),
            Err(e) => {
                eprintln!("encore-server: {e}");
                ExitCode::from(2)
            }
        };
    }
    let config = match config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("encore-server: {e}");
            return ExitCode::from(2);
        }
    };
    match args.first().map(String::as_str) {
        None | Some("serve") => {}
        Some("backup") => {
            let Some(dest) = args.get(1) else {
                eprintln!("encore-server: usage: encore-server backup <file>");
                return ExitCode::from(2);
            };
            return match encore_server::backup_database(&config.data_dir, dest.as_ref()) {
                Ok(()) => {
                    eprintln!("encore-server: backed up to {dest}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("encore-server: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        Some(other) => {
            eprintln!("encore-server: unknown command {other:?} (serve, health or backup)");
            return ExitCode::from(2);
        }
    }

    let transport = match encore_sources::http::HttpTransport::new(None, Duration::from_secs(10)) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            eprintln!("encore-server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let check = Arc::new(JellyfinCheck::new(config.jellyfin_urls.clone(), transport));
    let bind = config.bind;
    let state = match AppState::open(config, check) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("encore-server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("encore-server: cannot listen on {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "encore-server {} listening on http://{bind}",
        env!("CARGO_PKG_VERSION")
    );
    let shutdown = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            match signal(SignalKind::terminate()) {
                Ok(mut term) => {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {},
                        _ = term.recv() => {},
                    }
                }
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    };
    match axum::serve(
        listener,
        encore_server::app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
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
