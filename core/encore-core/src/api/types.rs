//! The records, enums and errors of the public API (docs/core/CORE-API.md).
//!
//! Every type here is plain data: records and unit enums only, so the same shapes
//! work in Swift and Kotlin (UniFFI) and in Dart (flutter_rust_bridge mirrors, which
//! cannot carry data inside enum variants). IDs and times are `i64` because Dart has
//! no unsigned 64-bit integer.

use encore_sources::SourceError;
use encore_store::{AlbumRow, ArtistRow, PlaylistRow, StoreError, TrackRow};

/// What the platform tells the core when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CoreConfig {
    /// A folder the app owns, for profile databases.
    pub data_dir: String,
    /// Stable per installation (stored by the app).
    pub device_id: String,
    /// Shown on the server's Devices page.
    pub device_name: String,
    /// "ios", "macos", "android", "windows" or "linux".
    pub platform: String,
}

// ---------------------------------------------------------------- connect

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ServerCheck {
    /// The address that answered; pass it to the sign-in calls.
    pub base_url: String,
    pub name: String,
    pub version: String,
    pub server_id: String,
    /// Plain http on the local network: show the quiet warning (board 21).
    pub plain_http_on_local_network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct QuickConnectStart {
    /// The 6 characters to approve on another signed-in device.
    pub code: String,
    pub expires_in_seconds: u32,
}

/// The result of signing in. Store `token` in the OS keychain or keystore under
/// `credential_key`; the core never stores it (plan section 12).
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SignedIn {
    pub profile_id: String,
    pub source_id: i64,
    pub credential_key: String,
    pub token: String,
    pub user_name: String,
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("profile_id", &self.profile_id)
            .field("source_id", &self.source_id)
            .finish_non_exhaustive()
    }
}

/// A token the app read from its keychain, handed back when a profile opens.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SourceToken {
    pub source_id: i64,
    pub token: String,
}

impl std::fmt::Debug for SourceToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceToken")
            .field("source_id", &self.source_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CredentialKey {
    pub source_id: i64,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ProfileSummary {
    pub profile_id: String,
    pub display_name: String,
    /// Where each source's token is kept in the keychain.
    pub credentials: Vec<CredentialKey>,
}

/// A row on the Servers and sources page (board 18).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ServerRow {
    pub source_id: i64,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub version: Option<String>,
    pub user_name: String,
    pub song_count: u32,
    pub last_sync_utc_ms: Option<i64>,
    /// False when no token was handed over for it: show "Sign in again".
    pub signed_in: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SyncResult {
    pub source_id: i64,
    pub artists: u32,
    pub albums: u32,
    pub songs: u32,
    pub playlists: u32,
    pub removed: u32,
}

// ---------------------------------------------------------------- library

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Song {
    pub id: i64,
    pub source_id: i64,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album_id: Option<i64>,
    pub album_title: Option<String>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration_ms: i64,
    pub year: Option<i32>,
    pub container: Option<String>,
    pub has_lyrics: bool,
    pub track_gain_db: Option<f64>,
    pub album_gain_db: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AlbumSummary {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub song_count: u32,
    pub duration_ms: i64,
    pub date_added_utc_ms: Option<i64>,
    pub has_artwork: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AlbumDetail {
    pub album: AlbumSummary,
    /// In disc and track order.
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ArtistSummary {
    pub id: i64,
    pub name: String,
    pub album_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ArtistDetail {
    pub artist: ArtistSummary,
    /// Newest first.
    pub albums: Vec<AlbumSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub song_count: u32,
    pub duration_ms: i64,
}

/// `song` is None when the song has left the library; the entry stays so nothing
/// vanishes silently (board 20).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PlaylistEntry {
    pub position: u32,
    pub song: Option<Song>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PlaylistDetail {
    pub playlist: PlaylistSummary,
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct GenreSummary {
    pub name: String,
    pub song_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum AlbumSort {
    Title,
    Artist,
    Year,
    RecentlyAdded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SongSort {
    Title,
    Artist,
    Album,
    RecentlyAdded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ResultKind {
    Song,
    Album,
    Artist,
    Playlist,
}

/// The best match; exactly one of the four is set, as `kind` says.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TopResult {
    pub kind: ResultKind,
    pub song: Option<Song>,
    pub album: Option<AlbumSummary>,
    pub artist: Option<ArtistSummary>,
    pub playlist: Option<PlaylistSummary>,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SearchResults {
    pub top: Option<TopResult>,
    pub songs: Vec<Song>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
}

// ---------------------------------------------------------------- playback

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum StartedFrom {
    Album,
    Playlist,
    Artist,
    Songs,
    Genre,
    Search,
    Radio,
    Mix,
    Queue,
    History,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ChosenBy {
    User,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ListeningMode {
    Normal,
    Private,
    Guest,
    LearningOff,
}

/// How a play ended, as the platform's player saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum EndReason {
    /// Played to the end: the queue moves on (Repeat One replays).
    Finished,
    /// Paused and never resumed.
    PausedAbandoned,
    /// Headphones out, output switch, call or assistant (never a skip).
    Interrupted,
    /// Failed to load or play: the queue moves on.
    Error,
    /// Sleep timer, app closed, connection lost.
    EndedBySystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum PreviousResult {
    /// Seek the current song to 0:00.
    Restarted,
    /// Play the new current song.
    Moved,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct QueueEntry {
    /// Tells two copies of the same song apart.
    pub uid: i64,
    pub song: Song,
    pub chosen_by: ChosenBy,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct QueueState {
    /// Oldest first.
    pub history: Vec<QueueEntry>,
    pub current: Option<QueueEntry>,
    pub up_next: Vec<QueueEntry>,
    pub autoplay: Vec<QueueEntry>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub autoplay_enabled: bool,
    /// Whether `undo_queue` has something to undo (the Undo toasts, board 11).
    pub can_undo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum Network {
    Wifi,
    Cellular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum StreamQuality {
    Original,
    High,
    Normal,
    DataSaver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum TranscodeCodec {
    Aac,
    Opus,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl std::fmt::Debug for HttpHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpHeader")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Where the player loads a song. Use `url` with `headers` when the player can send
/// headers (ExoPlayer, the desktop engine); `url_with_api_key` otherwise. Never log
/// either: both carry the token.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct StreamInfo {
    pub song_id: i64,
    pub url: String,
    pub url_with_api_key: String,
    pub headers: Vec<HttpHeader>,
    pub may_transcode: bool,
}

impl std::fmt::Debug for StreamInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamInfo")
            .field("song_id", &self.song_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ArtworkOf {
    Album,
    Song,
    Artist,
    Playlist,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ArtworkRequest {
    pub url: String,
    pub headers: Vec<HttpHeader>,
}

impl std::fmt::Debug for ArtworkRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtworkRequest").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------- lyrics

/// How finely a song's lyrics are timed (board 10's three states).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum LyricsSync {
    /// At least one line has per-word timing; lines without words light up whole.
    Word,
    /// Lines are timed, words are not.
    Line,
    /// No timing: a plain page that scrolls, with a "Not synced" tag.
    Unsynced,
}

/// Where lyrics came from. The UI never assumes the source of word timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum LyricsOrigin {
    MediaServer,
    EmbeddedTag,
    Sidecar,
    Provider,
    /// Word timing generated by the companion server (later; not built yet).
    Generated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum LyricsFormat {
    Jellyfin,
    Lrc,
    EnhancedLrc,
    Ttml,
    Plain,
}

/// Which side a line aligns to: a second singer's lines sit opposite (board 10, item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum LyricSide {
    Main,
    Opposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SingerKind {
    Person,
    /// Several singers together.
    Group,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Singer {
    pub id: String,
    pub name: Option<String>,
    pub kind: SingerKind,
}

/// A timed word or syllable. A line's text is its words' texts, with a space after
/// those whose `space_after` is set; syllables of one word have none between them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LyricWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub space_after: bool,
}

/// Background vocals under a line: smaller and dimmer (board 10, item 5).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LyricBackground {
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub words: Vec<LyricWord>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LyricLine {
    pub text: String,
    /// None only when the lyrics are unsynced. The core always fills `end_ms` for
    /// synced lines, and every word's end.
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    /// Empty: the line lights up whole.
    pub words: Vec<LyricWord>,
    /// Index into `SongLyrics::singers`.
    pub singer: Option<u32>,
    pub side: LyricSide,
    pub background: Vec<LyricBackground>,
    /// 0 to 1; 1 for lyrics read from a file or a server.
    pub confidence: f64,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
}

/// A silence long enough for the breathing dots (board 10, item 6;
/// `lyrics.instrumentalGapSeconds`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LyricGap {
    /// The line it follows; None before the first line.
    pub after_line: Option<u32>,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// A song's lyrics, ready to draw. Lines are in the order they are sung.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SongLyrics {
    pub song_id: i64,
    pub sync: LyricsSync,
    pub origin: LyricsOrigin,
    pub format: LyricsFormat,
    pub lines: Vec<LyricLine>,
    pub singers: Vec<Singer>,
    pub gaps: Vec<LyricGap>,
    /// The song is instrumental: the dots and the title only.
    pub instrumental: bool,
    pub language: Option<String>,
    pub fetched_utc_ms: i64,
}

// ---------------------------------------------------------------- events

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum EventKind {
    /// `queue` is set.
    QueueChanged,
    /// `now_playing` is set (None when the queue ended).
    NowPlaying,
    /// `preload` is set: load this song now, it plays next (20 s before the end).
    Preload,
    /// `sync_source_id`, `sync_done` and `sync_total` are set.
    SyncProgress,
}

/// One event. Only the fields its `kind` names are set.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CoreEvent {
    pub kind: EventKind,
    pub queue: Option<QueueState>,
    pub now_playing: Option<QueueEntry>,
    pub preload: Option<QueueEntry>,
    pub sync_source_id: Option<i64>,
    pub sync_done: i64,
    pub sync_total: i64,
}

impl CoreEvent {
    pub(crate) fn of(kind: EventKind) -> CoreEvent {
        CoreEvent {
            kind,
            queue: None,
            now_playing: None,
            preload: None,
            sync_source_id: None,
            sync_done: 0,
            sync_total: 0,
        }
    }
}

/// Implemented by the app to receive events. Called on a core thread: hop to the
/// UI thread before touching UI.
#[cfg_attr(feature = "uniffi", uniffi::export(with_foreign))]
pub trait CoreListener: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}

// ---------------------------------------------------------------- errors

/// Every failure, in words a person can act on (boards 18, 20 and 21).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
pub enum EncoreError {
    #[error("{reason}")]
    InvalidAddress { reason: String },
    #[error("{reason}")]
    InsecureAddress { reason: String },
    #[error("{reason}")]
    Unreachable { reason: String },
    /// Show the fingerprint and ask once whether to trust it (board 21).
    #[error("{reason}")]
    CertificateUntrusted { sha256: String, reason: String },
    #[error("{reason}")]
    NotJellyfin { reason: String },
    #[error("{reason}")]
    UnsupportedVersion { reason: String },
    #[error("{reason}")]
    WrongCredentials { reason: String },
    #[error("{reason}")]
    SessionExpired { reason: String },
    #[error("{reason}")]
    QuickConnectDisabled { reason: String },
    #[error("{reason}")]
    QuickConnectExpired { reason: String },
    #[error("{reason}")]
    QuickConnectCancelled { reason: String },
    #[error("{reason}")]
    NotFound { reason: String },
    /// `detail` is the server's own text, for a "Details" disclosure.
    #[error("{reason}")]
    Server {
        status: u16,
        reason: String,
        detail: String,
    },
    #[error("{reason}")]
    NoProfile { reason: String },
    #[error("{reason}")]
    Storage { reason: String },
    #[error("{reason}")]
    Internal { reason: String },
}

/// The kind of an `EncoreError`, for bindings that flatten errors (Dart).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ErrorKind {
    InvalidAddress,
    InsecureAddress,
    Unreachable,
    CertificateUntrusted,
    NotJellyfin,
    UnsupportedVersion,
    WrongCredentials,
    SessionExpired,
    QuickConnectDisabled,
    QuickConnectExpired,
    QuickConnectCancelled,
    NotFound,
    Server,
    NoProfile,
    Storage,
    Internal,
}

impl EncoreError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            EncoreError::InvalidAddress { .. } => ErrorKind::InvalidAddress,
            EncoreError::InsecureAddress { .. } => ErrorKind::InsecureAddress,
            EncoreError::Unreachable { .. } => ErrorKind::Unreachable,
            EncoreError::CertificateUntrusted { .. } => ErrorKind::CertificateUntrusted,
            EncoreError::NotJellyfin { .. } => ErrorKind::NotJellyfin,
            EncoreError::UnsupportedVersion { .. } => ErrorKind::UnsupportedVersion,
            EncoreError::WrongCredentials { .. } => ErrorKind::WrongCredentials,
            EncoreError::SessionExpired { .. } => ErrorKind::SessionExpired,
            EncoreError::QuickConnectDisabled { .. } => ErrorKind::QuickConnectDisabled,
            EncoreError::QuickConnectExpired { .. } => ErrorKind::QuickConnectExpired,
            EncoreError::QuickConnectCancelled { .. } => ErrorKind::QuickConnectCancelled,
            EncoreError::NotFound { .. } => ErrorKind::NotFound,
            EncoreError::Server { .. } => ErrorKind::Server,
            EncoreError::NoProfile { .. } => ErrorKind::NoProfile,
            EncoreError::Storage { .. } => ErrorKind::Storage,
            EncoreError::Internal { .. } => ErrorKind::Internal,
        }
    }

    /// The user-facing sentence.
    pub fn message(&self) -> String {
        self.to_string()
    }

    pub(crate) fn no_profile() -> EncoreError {
        EncoreError::NoProfile {
            reason: "No profile is open. Sign in or pick a profile".into(),
        }
    }

    pub(crate) fn internal(what: impl std::fmt::Display) -> EncoreError {
        EncoreError::Internal {
            reason: format!("Something went wrong inside the app ({what})"),
        }
    }
}

impl From<SourceError> for EncoreError {
    fn from(e: SourceError) -> Self {
        // `reason`, not `message`: Kotlin exceptions already have a `message`.
        let reason = e.to_string();
        match e {
            SourceError::InvalidAddress(_) => EncoreError::InvalidAddress { reason },
            SourceError::InsecureAddress => EncoreError::InsecureAddress { reason },
            SourceError::Unreachable(_) => EncoreError::Unreachable { reason },
            SourceError::CertificateUntrusted { sha256 } => {
                EncoreError::CertificateUntrusted { sha256, reason }
            }
            SourceError::NotJellyfin => EncoreError::NotJellyfin { reason },
            SourceError::UnsupportedVersion { .. } => EncoreError::UnsupportedVersion { reason },
            SourceError::WrongCredentials => EncoreError::WrongCredentials { reason },
            SourceError::SessionExpired => EncoreError::SessionExpired { reason },
            SourceError::QuickConnectDisabled => EncoreError::QuickConnectDisabled { reason },
            SourceError::QuickConnectExpired => EncoreError::QuickConnectExpired { reason },
            SourceError::NotFound => EncoreError::NotFound { reason },
            SourceError::Server {
                status,
                message: detail,
            } => EncoreError::Server {
                status,
                reason,
                detail,
            },
            SourceError::Protocol(_) => EncoreError::Server {
                status: 0,
                reason,
                detail: String::new(),
            },
        }
    }
}

impl From<StoreError> for EncoreError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(what) => EncoreError::NotFound {
                reason: format!("That {what} isn't in your library"),
            },
            StoreError::Backup(why) => EncoreError::Storage {
                reason: format!("That backup can't be used: {why}"),
            },
            StoreError::NewerSchema { .. } => EncoreError::Storage {
                reason: "This was saved by a newer version of Encore. Update the app to open it."
                    .into(),
            },
            other => EncoreError::Storage {
                reason: format!("Your library on this device couldn't be read ({other})"),
            },
        }
    }
}

impl From<crate::library::SyncError> for EncoreError {
    fn from(e: crate::library::SyncError) -> Self {
        match e {
            crate::library::SyncError::Source(s) => s.into(),
            crate::library::SyncError::Store(s) => s.into(),
            crate::library::SyncError::Poisoned => EncoreError::internal("store lock"),
        }
    }
}

// ---------------------------------------------------------------- conversions

fn lyric_words(words: Vec<encore_lyrics::Word>) -> Vec<LyricWord> {
    words
        .into_iter()
        .map(|w| LyricWord {
            text: w.text,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
            space_after: w.space_after,
        })
        .collect()
}

impl SongLyrics {
    pub(crate) fn from_model(song_id: i64, l: encore_lyrics::Lyrics, fetched_utc_ms: i64) -> Self {
        use encore_lyrics as m;
        SongLyrics {
            song_id,
            sync: match l.sync {
                m::SyncLevel::Word => LyricsSync::Word,
                m::SyncLevel::Line => LyricsSync::Line,
                m::SyncLevel::Unsynced => LyricsSync::Unsynced,
            },
            origin: match l.origin {
                m::Origin::MediaServer => LyricsOrigin::MediaServer,
                m::Origin::EmbeddedTag => LyricsOrigin::EmbeddedTag,
                m::Origin::Sidecar => LyricsOrigin::Sidecar,
                m::Origin::Provider => LyricsOrigin::Provider,
                m::Origin::Generated => LyricsOrigin::Generated,
            },
            format: match l.format {
                m::Format::Jellyfin => LyricsFormat::Jellyfin,
                m::Format::Lrc => LyricsFormat::Lrc,
                m::Format::EnhancedLrc => LyricsFormat::EnhancedLrc,
                m::Format::Ttml => LyricsFormat::Ttml,
                m::Format::Plain => LyricsFormat::Plain,
            },
            lines: l
                .lines
                .into_iter()
                .map(|x| LyricLine {
                    text: x.text,
                    start_ms: x.start_ms,
                    end_ms: x.end_ms,
                    words: lyric_words(x.words),
                    singer: x.agent,
                    side: match x.side {
                        m::Side::Main => LyricSide::Main,
                        m::Side::Opposite => LyricSide::Opposite,
                    },
                    background: x
                        .background
                        .into_iter()
                        .map(|b| LyricBackground {
                            text: b.text,
                            start_ms: b.start_ms,
                            end_ms: b.end_ms,
                            words: lyric_words(b.words),
                        })
                        .collect(),
                    confidence: f64::from(x.confidence),
                    translation: x.translation,
                    pronunciation: x.pronunciation,
                })
                .collect(),
            singers: l
                .agents
                .into_iter()
                .map(|a| Singer {
                    id: a.id,
                    name: a.name,
                    kind: match a.kind {
                        m::AgentKind::Person => SingerKind::Person,
                        m::AgentKind::Group => SingerKind::Group,
                        m::AgentKind::Other => SingerKind::Other,
                    },
                })
                .collect(),
            gaps: l
                .gaps
                .into_iter()
                .map(|g| LyricGap {
                    after_line: g.after_line,
                    start_ms: g.start_ms,
                    end_ms: g.end_ms,
                })
                .collect(),
            instrumental: l.instrumental,
            language: l.language,
            fetched_utc_ms,
        }
    }
}

impl From<TrackRow> for Song {
    fn from(t: TrackRow) -> Self {
        Song {
            id: t.id,
            source_id: t.source_id,
            title: t.title,
            artist: t.artist,
            album_artist: t.album_artist,
            album_id: t.album_id,
            album_title: t.album_title,
            disc_number: t.disc_number,
            track_number: t.track_number,
            duration_ms: t.duration_ms as i64,
            year: t.year,
            container: t.container,
            has_lyrics: t.has_lyrics,
            track_gain_db: t.track_gain_db,
            album_gain_db: t.album_gain_db,
        }
    }
}

impl From<AlbumRow> for AlbumSummary {
    fn from(a: AlbumRow) -> Self {
        AlbumSummary {
            id: a.id,
            title: a.title,
            artist: a.artist,
            year: a.year,
            song_count: a.track_count,
            duration_ms: a.duration_ms as i64,
            date_added_utc_ms: a.date_added_utc_ms,
            has_artwork: a.has_artwork,
        }
    }
}

impl From<ArtistRow> for ArtistSummary {
    fn from(a: ArtistRow) -> Self {
        ArtistSummary {
            id: a.id,
            name: a.name,
            album_count: a.album_count,
        }
    }
}

impl From<PlaylistRow> for PlaylistSummary {
    fn from(p: PlaylistRow) -> Self {
        PlaylistSummary {
            id: p.id,
            name: p.name,
            song_count: p.track_count,
            duration_ms: p.duration_ms as i64,
        }
    }
}

impl From<StartedFrom> for crate::queue::StartedFrom {
    fn from(s: StartedFrom) -> Self {
        use crate::queue::StartedFrom as Q;
        match s {
            StartedFrom::Album => Q::Album,
            StartedFrom::Playlist => Q::Playlist,
            StartedFrom::Artist => Q::Artist,
            StartedFrom::Songs => Q::Songs,
            StartedFrom::Genre => Q::Genre,
            StartedFrom::Search => Q::Search,
            StartedFrom::Radio => Q::Radio,
            StartedFrom::Mix => Q::Mix,
            StartedFrom::Queue => Q::Queue,
            StartedFrom::History => Q::History,
            StartedFrom::Other => Q::Other,
        }
    }
}

impl From<RepeatMode> for crate::queue::RepeatMode {
    fn from(r: RepeatMode) -> Self {
        match r {
            RepeatMode::Off => crate::queue::RepeatMode::Off,
            RepeatMode::All => crate::queue::RepeatMode::All,
            RepeatMode::One => crate::queue::RepeatMode::One,
        }
    }
}

impl From<crate::queue::RepeatMode> for RepeatMode {
    fn from(r: crate::queue::RepeatMode) -> Self {
        match r {
            crate::queue::RepeatMode::Off => RepeatMode::Off,
            crate::queue::RepeatMode::All => RepeatMode::All,
            crate::queue::RepeatMode::One => RepeatMode::One,
        }
    }
}

impl From<ListeningMode> for crate::playlog::ListeningMode {
    fn from(m: ListeningMode) -> Self {
        use crate::playlog::ListeningMode as L;
        match m {
            ListeningMode::Normal => L::Normal,
            ListeningMode::Private => L::Private,
            ListeningMode::Guest => L::Guest,
            ListeningMode::LearningOff => L::LearningOff,
        }
    }
}

impl From<EndReason> for crate::playlog::EndReason {
    fn from(r: EndReason) -> Self {
        use crate::playlog::EndReason as E;
        match r {
            EndReason::Finished => E::Finished,
            EndReason::PausedAbandoned => E::PausedAbandoned,
            EndReason::Interrupted => E::Interrupted,
            EndReason::Error => E::Error,
            EndReason::EndedBySystem => E::EndedBySystem,
        }
    }
}
