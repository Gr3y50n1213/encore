//! Errors in words a person can act on (boards 18, 20 and 21).

use std::fmt;

/// Why a server could not be reached (board 18, item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreachable {
    /// The name does not resolve: usually a wrong address.
    NameNotFound,
    /// Nothing is listening on that port.
    Refused,
    /// No answer in time: the server may be asleep or off.
    TimedOut,
    /// The secure connection could not be set up.
    Tls(String),
    Other(String),
}

impl fmt::Display for Unreachable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unreachable::NameNotFound => write!(f, "the address wasn't found; check it for typos"),
            Unreachable::Refused => write!(f, "the server refused the connection; check the port"),
            Unreachable::TimedOut => write!(f, "the server didn't answer; it may be asleep or off"),
            Unreachable::Tls(d) => write!(f, "a secure connection couldn't be set up ({d})"),
            Unreachable::Other(d) => write!(f, "{d}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error("That address isn't valid: {0}")]
    InvalidAddress(String),
    /// Plain http to an address outside the local network (board 21, item 3).
    #[error("This address is not secure. Use https")]
    InsecureAddress,
    #[error("Couldn't reach the server: {0}")]
    Unreachable(Unreachable),
    /// A certificate the system does not trust; the user may trust this exact one.
    #[error("The server's certificate isn't trusted")]
    CertificateUntrusted { sha256: String },
    #[error("This isn't a Jellyfin server")]
    NotJellyfin,
    #[error("This server runs Jellyfin {found}; Encore needs {minimum} or later")]
    UnsupportedVersion { found: String, minimum: String },
    #[error("Wrong username or password")]
    WrongCredentials,
    #[error("Your sign-in expired. Sign in again")]
    SessionExpired,
    #[error("Quick Connect is turned off on this server")]
    QuickConnectDisabled,
    #[error("The Quick Connect code expired")]
    QuickConnectExpired,
    #[error("Not found on the server")]
    NotFound,
    /// Any other error status, with the server's own text for the Details disclosure.
    #[error("The server returned an error ({status})")]
    Server { status: u16, message: String },
    #[error("The server's reply wasn't understood: {0}")]
    Protocol(String),
}

pub type Result<T> = std::result::Result<T, SourceError>;
