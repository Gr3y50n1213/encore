//! The real HTTP transport: reqwest over rustls on ring (docs/adr/core-0005).
//!
//! Certificates are checked by the operating system's verifier, so a private CA the
//! user installed works. A self-signed certificate is refused with its SHA-256
//! fingerprint; once the user trusts that exact fingerprint (board 21, item 3) it is
//! pinned for that server, and the handshake signature is still verified.
//!
//! Android: the platform verifier needs the app to call its init once with a
//! Context (docs/requests: core to android).

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

#[derive(Debug)]
pub struct HttpTransport {
    client: reqwest::Client,
    rejected: Arc<Mutex<Option<String>>>,
}

impl HttpTransport {
    /// `pinned_sha256`: a certificate fingerprint the user chose to trust for this
    /// server, if any.
    pub fn new(pinned_sha256: Option<&str>, connect_timeout: Duration) -> Result<HttpTransport> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls_err = |e: rustls::Error| SourceError::Unreachable(Unreachable::Tls(e.to_string()));
        let platform =
            rustls_platform_verifier::Verifier::new(provider.clone()).map_err(tls_err)?;
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

impl Transport for HttpTransport {
    fn send(&self, req: Request) -> BoxFuture<'_, Result<Response>> {
        Box::pin(async move {
            let mut b = match req.method {
                Method::Get => self.client.get(req.url),
                Method::Post => self.client.post(req.url),
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
