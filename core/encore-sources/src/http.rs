//! The real HTTP transport: reqwest over rustls on ring (docs/adr/core-0005).
//!
//! Certificates are checked by the operating system's verifier, so a private CA the
//! user installed works. A self-signed certificate is refused with its SHA-256
//! fingerprint; once the user trusts that exact fingerprint (board 21, item 3) it is
//! pinned for that server, and the handshake signature is still verified.
//!
//! Android: the platform verifier calls into the JVM, so the app calls
//! `EncoreTls.initialize(context)` once at start (docs/requests/core-0001-android-tls-init.md).
//! Until it has, a certificate check fails with a message that says so; it never
//! panics, and plain http on the local network keeps working.

/// Android's one-time setup for certificate checks.
#[cfg(target_os = "android")]
pub mod android {
    use std::sync::atomic::{AtomicBool, Ordering};

    static READY: AtomicBool = AtomicBool::new(false);

    /// Hand the verifier the JVM and an Android `Context`. Called by the core's JNI
    /// entry point `EncoreTls.initialize`; calling it again is harmless.
    pub fn init(
        env: &mut jni::Env<'_>,
        context: jni::objects::JObject<'_>,
    ) -> Result<(), jni::errors::Error> {
        rustls_platform_verifier::android::init_with_env(env, context)?;
        READY.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Whether `init` has run.
    pub fn ready() -> bool {
        READY.load(Ordering::SeqCst)
    }
}

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::DigitallySignedStruct;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::{Digest, Sha256};

use crate::error::{Result, SourceError, Unreachable};
use crate::transport::{BoxFuture, Method, Request, Response, Transport};

/// Lower-case hex SHA-256 of a certificate (the fingerprint the UI shows).
pub fn sha256_hex(der: &[u8]) -> String {
    Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug)]
struct PinningVerifier {
    platform: rustls_platform_verifier::Verifier,
    pinned: Option<String>,
    provider: Arc<CryptoProvider>,
    rejected: Arc<Mutex<Option<String>>>,
}

impl ServerCertVerifier for PinningVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fingerprint = sha256_hex(end_entity.as_ref());
        if self.pinned.as_deref() == Some(fingerprint.as_str()) {
            return Ok(ServerCertVerified::assertion());
        }
        // Without the app's init, Android's verifier would panic: say what is missing.
        #[cfg(target_os = "android")]
        if !android::ready() {
            return Err(rustls::Error::General(
                "Android certificate checks are not set up: call EncoreTls.initialize(context) when the app starts".into(),
            ));
        }
        self.platform
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
            .inspect_err(|_| {
                if let Ok(mut r) = self.rejected.lock() {
                    *r = Some(fingerprint);
                }
            })
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// The TLS settings every connection uses: the operating system's verifier, plus a
/// fingerprint the user chose to trust. The second value holds the fingerprint of a
/// certificate the verifier refused (for "Trust this certificate?").
pub fn tls_config(
    pinned_sha256: Option<&str>,
) -> Result<(rustls::ClientConfig, Arc<Mutex<Option<String>>>)> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_err = |e: rustls::Error| SourceError::Unreachable(Unreachable::Tls(e.to_string()));
    let platform = rustls_platform_verifier::Verifier::new(provider.clone()).map_err(tls_err)?;
    let rejected = Arc::new(Mutex::new(None));
    let verifier = Arc::new(PinningVerifier {
        platform,
        pinned: pinned_sha256.map(str::to_ascii_lowercase),
        provider: provider.clone(),
        rejected: rejected.clone(),
    });
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok((config, rejected))
}

#[derive(Debug)]
pub struct HttpTransport {
    client: reqwest::Client,
    rejected: Arc<Mutex<Option<String>>>,
}

impl HttpTransport {
    /// `pinned_sha256`: a certificate fingerprint the user chose to trust for this
    /// server, if any.
    pub fn new(pinned_sha256: Option<&str>, connect_timeout: Duration) -> Result<HttpTransport> {
        let (config, rejected) = tls_config(pinned_sha256)?;
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(config)
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|e| {
                SourceError::Unreachable(Unreachable::Other(e.without_url().to_string()))
            })?;
        Ok(HttpTransport { client, rejected })
    }

    fn map_error(&self, e: reqwest::Error) -> SourceError {
        if let Some(sha256) = self.rejected.lock().ok().and_then(|mut r| r.take()) {
            return SourceError::CertificateUntrusted { sha256 };
        }
        if e.is_timeout() {
            return SourceError::Unreachable(Unreachable::TimedOut);
        }
        // Walk the causes for the kind of failure; never print the URL (it can hold
        // the ApiKey parameter).
        let mut source: Option<&dyn std::error::Error> = Some(&e);
        while let Some(err) = source {
            // A TLS failure says why (an expired certificate, or Android's
            // EncoreTls.initialize not called yet), instead of "error sending request".
            if let Some(reason) = tls_reason(err) {
                return SourceError::Unreachable(Unreachable::Tls(reason));
            }
            if let Some(io) = err.downcast_ref::<std::io::Error>() {
                match io.kind() {
                    std::io::ErrorKind::ConnectionRefused => {
                        return SourceError::Unreachable(Unreachable::Refused);
                    }
                    std::io::ErrorKind::TimedOut => {
                        return SourceError::Unreachable(Unreachable::TimedOut);
                    }
                    _ => {}
                }
            }
            let text = err.to_string().to_ascii_lowercase();
            if text.contains("dns error")
                || text.contains("failed to lookup address")
                || text.contains("no such host")
            {
                return SourceError::Unreachable(Unreachable::NameNotFound);
            }
            source = err.source();
        }
        SourceError::Unreachable(Unreachable::Other(e.without_url().to_string()))
    }
}

/// The TLS library's own words, when `err` is (or wraps, as I/O errors do) one of
/// its errors. An I/O error's `source()` skips the error it wraps, so look inside.
fn tls_reason(err: &(dyn std::error::Error + 'static)) -> Option<String> {
    // The client nests them: an I/O error holding an I/O error holding the TLS
    // error, so unwrap I/O errors to any depth.
    let mut cur = err;
    loop {
        if let Some(t) = cur.downcast_ref::<rustls::Error>() {
            return Some(t.to_string());
        }
        cur = cur.downcast_ref::<std::io::Error>()?.get_ref()?;
    }
}

impl Transport for HttpTransport {
    fn send(&self, req: Request) -> BoxFuture<'_, Result<Response>> {
        Box::pin(async move {
            let mut b = match req.method {
                Method::Get => self.client.get(req.url),
                Method::Post => self.client.post(req.url),
                Method::Put => self.client.put(req.url),
                Method::Delete => self.client.delete(req.url),
                Method::Patch => self.client.patch(req.url),
            };
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            if let Some(body) = req.body {
                b = b.body(body);
            }
            let resp = b
                .timeout(req.timeout)
                .send()
                .await
                .map_err(|e| self.map_error(e))?;
            let status = resp.status().as_u16();
            let server_date_utc_ms = resp
                .headers()
                .get("date")
                .and_then(|v| v.to_str().ok())
                .and_then(crate::jellyfin::time::parse_http_date);
            let body = resp.bytes().await.map_err(|e| self.map_error(e))?.to_vec();
            Ok(Response {
                status,
                body,
                server_date_utc_ms,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_are_lower_case_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

#[cfg(test)]
mod tls_reason_tests {
    use super::tls_reason;

    #[test]
    fn a_tls_failure_keeps_its_reason_even_inside_an_io_error() {
        let guard = rustls::Error::General(
            "Android certificate checks are not set up: call EncoreTls.initialize(context) when the app starts".into(),
        );
        let wrapped = std::io::Error::new(std::io::ErrorKind::InvalidData, guard);
        // As the HTTP client hands it over: wrapped once more.
        let wrapped = std::io::Error::other(wrapped);
        let reason = tls_reason(&wrapped).expect("found inside the I/O errors");
        assert!(reason.contains("EncoreTls.initialize"), "{reason}");
        let plain = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        assert_eq!(tls_reason(&plain), None);
    }
}

#[cfg(test)]
mod tls_failure_tests {
    use std::time::Duration;

    use crate::error::{SourceError, Unreachable};
    use crate::transport::{Method, Request, Transport};

    /// A server that reads the TLS hello and answers with something that isn't
    /// TLS: the transport must say it was a TLS failure, with its reason, not
    /// "error sending request" (found on Android phones, 7 November 2026).
    #[tokio::test(flavor = "current_thread")]
    async fn a_real_tls_failure_says_so() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for mut s in l.incoming().flatten() {
                let mut hello = [0u8; 512];
                let _ = s.read(&mut hello);
                let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        let t = super::HttpTransport::new(None, Duration::from_secs(5)).unwrap();
        let err = t
            .send(Request {
                method: Method::Get,
                url: url::Url::parse(&format!("https://127.0.0.1:{port}/")).unwrap(),
                headers: Vec::new(),
                body: None,
                timeout: Duration::from_secs(5),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, SourceError::Unreachable(Unreachable::Tls(_))),
            "{err:?}"
        );
    }
}
