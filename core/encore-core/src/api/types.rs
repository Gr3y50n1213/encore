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
    /// The name set in Manage profiles, or the media-server user name.
    pub display_name: String,
    /// Where each source's token is kept in the keychain (`source_id` 0 is the
    /// companion server's).
    pub credentials: Vec<CredentialKey>,
    /// Up to two letters for the avatar's fallback (board 14).
    pub initials: String,
    /// The media server's user image, square; load it like artwork (the server
    /// serves user images without sign-in). None: show the initials.
    pub avatar_url: Option<String>,
    /// A PIN is set: `unlock_profile` before `switch_profile` or `open_profile`.
    pub locked: bool,
    /// The open profile (listed first).
    pub active: bool,
    /// The media server ended this profile's sign-in: the row shows "Sign in again"
    /// and its data stays until removed.
    pub needs_sign_in: bool,
}

/// A Guest session on this device (board 14, item 5): nothing is saved. It ends
/// with `end_guest` or by itself after `profiles.guestIdleHours` idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct GuestSession {
    pub started_utc_ms: i64,
    /// When it ends if nothing plays before then.
    pub ends_if_idle_utc_ms: i64,
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
    /// Songs found by a line of their lyrics, not already in `songs`: show them in
    /// the Songs section after the title matches, with the line under the title.
    pub lyrics: Vec<LyricMatch>,
}

/// What went into an export, for the "Export finished" row and the share sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ExportSummary {
    pub plays: u64,
    pub likes: u64,
    pub playlists: u64,
    pub playlist_songs: u64,
    pub bytes: u64,
}

/// A song found by its lyrics. `line_index` and `start_ms` open the lyrics view
/// at that line.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LyricMatch {
    pub song: Song,
    pub line: String,
    pub line_index: u32,
    pub start_ms: Option<i64>,
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
    /// The song is downloaded: `url` is a `file://` path on this device, with no
    /// headers (Phase 8).
    pub local: bool,
    /// A name for the player's rolling cache: the server, the song and the
    /// quality, with no token in it. Report fully cached ones to
    /// `set_connectivity`, so they play offline.
    pub cache_key: String,
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

// ---------------------------------------------------------------- skips

/// What moved on from a song (board 42). Every origin runs the same code; the
/// origin is stored with the play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SkipOrigin {
    Button,
    /// The mini player or Now Playing artwork. Back always goes back.
    Swipe,
    Headset,
    Car,
    LockScreen,
    Keyboard,
}

impl From<SkipOrigin> for crate::signal::SkipOrigin {
    fn from(o: SkipOrigin) -> Self {
        match o {
            SkipOrigin::Button => crate::signal::SkipOrigin::Button,
            SkipOrigin::Swipe => crate::signal::SkipOrigin::Swipe,
            SkipOrigin::Headset => crate::signal::SkipOrigin::Headset,
            SkipOrigin::Car => crate::signal::SkipOrigin::Car,
            SkipOrigin::LockScreen => crate::signal::SkipOrigin::LockScreen,
            SkipOrigin::Keyboard => crate::signal::SkipOrigin::Keyboard,
        }
    }
}

/// Things that stop or move the music without the listener choosing to skip
/// (section 21, "Interruptions").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum InterruptionKind {
    HeadphonesUnplugged,
    /// Bluetooth, AirPlay or another output switched.
    OutputChanged,
    Call,
    Assistant,
}

/// Where a swipe would land. `end_of_queue`: nothing there, so the content follows
/// the finger at a third of the speed, stops after 40 pt and springs back with
/// "End of queue" (board 42, rule 8).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SkipTarget {
    pub song: Option<QueueEntry>,
    pub end_of_queue: bool,
}

// ---------------------------------------------------------------- playlist writes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DropTargetKind {
    /// `id` is the playlist.
    Playlist,
    /// A new playlist named after the album or the first artist.
    NewPlaylist,
    LikedSongs,
    /// `index` (in `add_to`) is the line in Up Next; None is the end.
    UpNext,
    PlayNext,
    /// `id` is the album. Always refused: albums come from the server.
    Album,
}

/// Where songs are dropped or sent (board 43). A record, not an enum with data,
/// so Dart gets the same shape.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DropTarget {
    pub kind: DropTargetKind,
    pub id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DropRefusal {
    GuestCannotSave,
    AlbumsTakeNoSongs,
    PlaylistReadOnly,
    NothingToAdd,
}

/// What a drop would do, before it is done: the badge count, the "already there"
/// count, whether to ask first (over 100), and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DropPreview {
    pub count: u32,
    pub already_there: u32,
    pub needs_confirm: bool,
    pub suggested_name: Option<String>,
    pub refusal: Option<DropRefusal>,
    /// The sentence to show after the refusal delay (board 43, rule 15).
    pub refusal_reason: Option<String>,
}

/// What one edit did. `edit_id` is what `undo` takes; None when nothing changed
/// (every song was already there).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct EditResult {
    pub edit_id: Option<String>,
    pub added: u32,
    pub already_there: u32,
    pub playlist_id: Option<i64>,
    /// Undo is accepted until then (`gestures.dragAndDrop.shortcutUndoMs`); the
    /// UI's toast may close sooner.
    pub undo_until_utc_ms: Option<i64>,
}

/// What sending the waiting edits did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SentEdits {
    pub sent: u32,
    /// The server said no; the next sync brings back its version.
    pub refused: u32,
    /// Still waiting for the server.
    pub waiting: u32,
}

impl From<crate::edits::Refusal> for DropRefusal {
    fn from(r: crate::edits::Refusal) -> Self {
        match r {
            crate::edits::Refusal::GuestCannotSave => DropRefusal::GuestCannotSave,
            crate::edits::Refusal::AlbumsTakeNoSongs => DropRefusal::AlbumsTakeNoSongs,
            crate::edits::Refusal::PlaylistReadOnly => DropRefusal::PlaylistReadOnly,
            crate::edits::Refusal::NothingToAdd => DropRefusal::NothingToAdd,
        }
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
    /// How finely the lyrics are timed (`sync` is a reserved word in Dart).
    pub sync_level: LyricsSync,
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

// ---------------------------------------------------------------- palettes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum PaletteScheme {
    /// A dark field with white text.
    Dark,
    /// Light art: a light field with dark text.
    Light,
}

/// One colour sampled from the cover and its share of it (0 to 1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PaletteSwatch {
    pub colour: String,
    pub share: f64,
}

/// The colours of an album's or playlist's gradient (board 04). Colours are
/// `RRGGBB` hex. Every colour reaches `lowest_contrast` against `text` or better.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ArtworkPalette {
    pub scheme: PaletteScheme,
    /// The field's base, tinted with the leading hue.
    pub base: String,
    /// Four stops for the moving field, leading colour first; the mini player and
    /// album pages take their tint from the first.
    pub stops: Vec<String>,
    pub text: String,
    pub lowest_contrast: f64,
    /// The five clusters as sampled, largest first.
    pub sampled: Vec<PaletteSwatch>,
}

impl From<crate::palette::Palette> for ArtworkPalette {
    fn from(p: crate::palette::Palette) -> Self {
        ArtworkPalette {
            scheme: match p.scheme {
                crate::palette::Scheme::Dark => PaletteScheme::Dark,
                crate::palette::Scheme::Light => PaletteScheme::Light,
            },
            base: p.base,
            stops: p.stops,
            text: p.text,
            lowest_contrast: p.lowest_contrast,
            sampled: p
                .sampled
                .into_iter()
                .map(|s| PaletteSwatch {
                    colour: s.colour,
                    share: s.share,
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------- companion sync

/// Where sync with the companion server stands (the sync status row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SyncState {
    /// No companion server for this profile: everything stays on this device.
    NotConnected,
    /// Up to date, listening for other devices' changes.
    Idle,
    /// Sending or receiving.
    Syncing,
    /// The server can't be reached; changes wait here and go when it's back.
    Offline,
    /// The server no longer accepts this device (it was signed out elsewhere, or
    /// the profile was deleted there). Local data stays; connect again to sync.
    SignedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SyncStatus {
    pub state: SyncState,
    /// Changes on this device not yet on the server ("Waiting to sync").
    pub pending: u32,
    pub last_synced_utc_ms: Option<i64>,
    /// The companion server's address, when one is set.
    pub server_url: Option<String>,
    /// Why, for Offline and SignedOut.
    pub reason: Option<String>,
}

/// What another device changed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RemoteChange {
    /// Songs whose like changed; refresh hearts and Liked Songs.
    pub liked_song_ids: Vec<i64>,
    pub unliked_song_ids: Vec<i64>,
    /// Playlists re-read from the media server.
    pub playlist_ids: Vec<i64>,
    /// Plays from other devices joined the history.
    pub history: bool,
    /// Another device made a playlist this one doesn't have yet: call
    /// `sync_library` when convenient.
    pub library_sync_suggested: bool,
}

/// A device signed in to this profile on the companion server (the Devices list).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CompanionDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub added_utc_ms: i64,
    pub last_seen_utc_ms: i64,
    pub signed_out: bool,
    pub this_device: bool,
}
// ---------------------------------------------------------------- smart picks

/// How many picks are new to the user (board 38): Familiar 5%, Balanced 10%,
/// Adventurous 20% of slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum Adventurousness {
    Familiar,
    Balanced,
    Adventurous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum TimeOfDay {
    /// 0:00 to 5:59.
    Night,
    Morning,
    Afternoon,
    /// 18:00 to 23:59.
    Evening,
}

/// What a reason says; the fields it uses are set (board 38: plain facts the user
/// can check, never mood, health or place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ReasonKind {
    /// `song`, `count` of `of`: "You finished Night Market 9 of the last 10 times, and
    /// this sounds like it."
    FinishedOften,
    /// `song`: "You often play this with Night Market."
    PlayedTogether,
    /// `time_of_day`: "You often play songs like this in the evening."
    TimeOfDay,
    /// "You liked this."
    YouLikedIt,
    /// `days`: "You loved this and haven't played it for 74 days."
    Rediscover,
    /// `artist`: "More from Ember Cove, whom you play often."
    SameArtist,
    /// `genre`: "Like the folk you play."
    SameGenre,
    /// `count`: "Played 14 times in your other apps."
    PlayedElsewhere,
    /// "New to you: picked so Encore keeps learning."
    NewToYou,
    /// "Added to your library recently."
    RecentlyAdded,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PickReason {
    pub kind: ReasonKind,
    pub song: Option<Song>,
    pub count: Option<u32>,
    pub of: Option<u32>,
    pub days: Option<u32>,
    pub time_of_day: Option<TimeOfDay>,
    pub artist: Option<String>,
    pub genre: Option<String>,
}

/// Why this song (board 38): one or two reasons, and up to two songs it sounds
/// like (they open on tap).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WhyThisSong {
    pub reasons: Vec<PickReason>,
    pub sounds_like: Vec<Song>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum NotInterestedReason {
    /// Lowers the song and its neighbours (also the default).
    NotMyTaste,
    /// Rests the song for 30 days only.
    HeardTooMuch,
    /// Lowers it at this time of day only.
    WrongMoment,
}

/// One entry of Settings, Not interested.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NotInterestedItem {
    /// Set for a song (the song still plays when the user picks it).
    pub song_id: Option<i64>,
    /// Set for an artist.
    pub artist: Option<String>,
    /// The song's title or the artist's name, for the list.
    pub label: String,
    pub reason: NotInterestedReason,
    pub added_utc_ms: i64,
}

/// A mix card's state (board 38); every state has a text label in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum MixState {
    Fresh,
    /// No good build for 48 hours: show the age and Refresh, never an empty tile.
    OutOfDate,
    /// Under `smartPicks.coldStart.fullFromPlays`: "Encore is still learning",
    /// with `plays_so_far` of `plays_needed`. Starter or family mixes may sit beside it.
    NotEnoughPlays,
    /// Learn from my listening is off: Recently added and Most played instead.
    LearningOff,
    /// Private listening or Guest is on: suggestions don't change from this session.
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum MixKind {
    Strongest,
    Second,
    TimeOfDay,
    /// Where taste has drifted in the last 30 days.
    Drift,
    /// About 30% new to the user.
    Stretch,
    /// While still learning: a mix by mood family.
    Family,
    /// No plays yet: from the media server's favourites and most played.
    Starter,
}

/// A mix tile (Home's Made for you, the Mixes tab).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MixSummary {
    pub mix_id: String,
    pub kind: MixKind,
    /// 2 to 4 plain words (the section 7.1 rules, enforced in the core).
    pub title: String,
    /// "Mostly A, B and 12 more".
    pub subtitle: String,
    /// The songs for the 2×2 cover mosaic (up to 4, different albums).
    pub cover_song_ids: Vec<i64>,
    pub song_count: u32,
    pub pinned: bool,
    pub made_utc_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MixShelf {
    pub state: MixState,
    /// Pinned first, then today's set.
    pub mixes: Vec<MixSummary>,
    pub plays_so_far: u32,
    pub plays_needed: u32,
    /// When the set was made; None before the first build.
    pub made_utc_ms: Option<i64>,
    /// Made by the companion server (else on this device).
    pub from_companion: bool,
}

/// Home (boards 05 and 23; the surfaces table of section 21).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Home {
    pub made_for_you: MixShelf,
    /// Loved songs not heard for 60+ days (12), else random liked songs.
    pub rediscover: Vec<Song>,
    /// Plain history order, no smarts.
    pub recently_played: Vec<Song>,
    /// Never played, matching the user's taste.
    pub deep_cuts: Vec<Song>,
    pub recently_added: Vec<Song>,
    /// The Learning-off fallback (also shown when there is little else).
    pub most_played: Vec<Song>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MixSong {
    pub song: Song,
    /// One short reason for the row (board 15, item 5).
    pub reason: Option<PickReason>,
    /// New to the user (the stretch mix's 30%).
    pub new_to_you: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MixPage {
    pub summary: MixSummary,
    pub songs: Vec<MixSong>,
}

/// One shelf of How suggestions are doing (board 38, note 7): the last 4 weeks.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ShelfStat {
    /// "autoplay", "mix", "radio", "smartShuffle".
    pub shelf: String,
    pub plays: u32,
    /// Per 1,000 plays (show as a percentage: 123 is 12.3%).
    pub early_skips_per_1000: u32,
    pub finished_per_1000: u32,
    /// Distinct artists in each run of 20 songs, times 10 (show 87 as 8.7).
    pub artists_per_20_x10: u32,
    pub repeats_per_1000: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SuggestionStats {
    pub since_utc_ms: i64,
    pub shelves: Vec<ShelfStat>,
}

/// What a radio is started from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum RadioSeed {
    Song,
    Album,
    Artist,
    Playlist,
}
// ---------------------------------------------------------------- downloads (Phase 8)

/// What can be kept on the device (board 13, item 3; board 17, item 3). Liked
/// songs and Smart downloads follow their lists by themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DownloadKind {
    Song,
    Album,
    Playlist,
    /// Follows the mix: when it changes, so do its downloads.
    Mix,
    /// Follows Liked songs (the "Download liked songs" setting).
    LikedSongs,
    /// Smart downloads (section 21): the app's picks from pinned mixes, inside a
    /// budget. Turned on and off in the settings, never downloaded by hand.
    Smart,
}

/// A thing to download or remove: `id` is the song's, album's or playlist's ID;
/// a mix is named by `mix_id`. Liked songs and Smart downloads need neither (0).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadTarget {
    pub kind: DownloadKind,
    pub id: i64,
    pub mix_id: Option<String>,
}

/// Where a downloaded thing stands (board 06, item 5: none, a ring, a badge, a
/// warning dot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DownloadState {
    /// Some songs are still to come; nothing is moving now (`stop` in the
    /// overview says why, for example waiting for Wi-Fi).
    Waiting,
    /// Moving now: a ring.
    Downloading,
    /// Every song is on the device: the badge.
    Saved,
    /// Paused by the person.
    Paused,
    /// Some songs couldn't be fetched after every try: the warning dot. Tap to
    /// retry.
    Failed,
}

/// One downloaded album, playlist, mix or song (the Library's Downloaded chip,
/// the Downloads page's list).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadedItem {
    pub target: DownloadTarget,
    pub title: String,
    /// The artist for an album or song, empty otherwise.
    pub subtitle: String,
    pub state: DownloadState,
    /// "7 of 12 saved" (board 08).
    pub saved: u32,
    pub total: u32,
    pub failed: u32,
    pub bytes: u64,
    /// Kept by the app (Liked songs, Smart downloads).
    pub auto: bool,
    pub added_utc_ms: i64,
    /// Why the last failure happened, when some failed.
    pub error: Option<String>,
}

/// Everything the lists need to draw badges and dim what can't play offline,
/// in one cheap call (`download_marks`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadMarks {
    pub saved_song_ids: Vec<i64>,
    /// Wanted and not saved yet (paused ones included).
    pub pending_song_ids: Vec<i64>,
    pub failed_song_ids: Vec<i64>,
    /// What plays with no connection: saved songs plus the ones the player has
    /// fully cached (`set_connectivity`). Offline, everything else dims.
    pub playable_offline_song_ids: Vec<i64>,
    pub items: Vec<DownloadedItem>,
    /// The connectivity the platform last reported.
    pub online: bool,
    /// The song being fetched right now, if any.
    pub active_song_id: Option<i64>,
}

/// Why `run_downloads` stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DownloadStop {
    /// Nothing left to fetch.
    Done,
    /// `stop_downloads` was called; it carries on next time where it was.
    Stopped,
    /// Another run is going; this one did nothing.
    AlreadyRunning,
    /// "Download only on Wi-Fi" is on and the network is cellular.
    WaitingForWifi,
    /// The device has no connection.
    Offline,
    /// The media server didn't answer; try later.
    ServerUnreachable,
    /// The media server's sign-in ended; sign in again.
    SignInNeeded,
    /// The storage limit in the settings is reached; nothing is deleted.
    StorageLimit,
    /// The device is nearly full (it keeps `downloads.keepFreeMB` free).
    LowSpace,
    /// No profile is open (or it is a Guest, which downloads nothing).
    NoProfile,
}

/// What one `run_downloads` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadRun {
    pub saved: u32,
    pub failed: u32,
    /// Still to fetch.
    pub remaining: u32,
    pub stop: DownloadStop,
}

/// The transfer under way, for the progress notification ("Downloading Road
/// trip, 15 of 24 songs", boards 32 and 34) and the Downloads page.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadProgress {
    /// The album, playlist or mix being fetched (the song's own title for a song).
    pub item_title: String,
    /// That item's songs on the device so far, and its songs in all ("15 of 24").
    pub item_saved: u32,
    pub item_total: u32,
    pub song_id: i64,
    pub song_title: String,
    /// Songs saved in this run, and the run's songs in all.
    pub songs_done: u32,
    pub songs_total: u32,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

/// The Downloads settings (board 17, item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadSettings {
    /// Original, High or Normal (Data saver is for streams only).
    pub quality: StreamQuality,
    pub wifi_only: bool,
    /// None: no limit. At the limit new songs wait; nothing is deleted.
    pub limit_bytes: Option<u64>,
    /// Remove an album or playlist not played for `downloads.removeUnplayedAfterDays`.
    pub remove_unplayed: bool,
    /// Keep Liked songs downloaded.
    pub liked_songs: bool,
    /// Smart downloads (section 21): pinned mixes inside the budget, nightly on
    /// Wi-Fi while charging.
    pub smart_downloads: bool,
    /// The player's rolling cache of streamed songs; 0 turns it off.
    pub cache_limit_bytes: u64,
}

/// The Downloads page (board 17, item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadsOverview {
    /// What this profile's downloads take on disk.
    pub used_bytes: u64,
    pub song_count: u32,
    pub items: Vec<DownloadedItem>,
    pub queued: u32,
    pub failed: u32,
    pub active: Option<DownloadProgress>,
    /// Why the last run stopped, while songs are still waiting.
    pub stop: Option<DownloadStop>,
    pub settings: DownloadSettings,
}

// ---------------------------------------------------------------- sleep timer (Phase 9, WP1)

/// How a sleep timer ends (PLAN-UX, "Sleep timer"; board 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SleepTimerMode {
    /// At a time: the music fades out over the last `fade_ms`, then pauses.
    Minutes,
    /// When the current song ends; no fade.
    EndOfSong,
    /// When the current album's last song in a row in the queue ends; no fade.
    EndOfAlbum,
}

/// What `set_sleep_timer` starts. `minutes` counts for Minutes only (the sheet
/// offers 5, 10, 15, 30, 45 and 60; 1 to 1,440 is accepted). A record, not an
/// enum with data, so Dart gets the same shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SleepTimerSpec {
    pub mode: SleepTimerMode,
    pub minutes: u32,
}

/// The sleep timer. One per device, not per profile: it carries on through a
/// profile switch, survives the app being closed, and ends when playback is
/// handed to another device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SleepTimer {
    pub mode: SleepTimerMode,
    /// When it was set; Add 15 min keeps it (the sheet's bar runs from here to
    /// `ends_at_utc_ms`).
    pub set_utc_ms: i64,
    /// When the music stops. Exact for Minutes. For EndOfSong and EndOfAlbum an
    /// estimate from the player's position and the songs' lengths, worked out
    /// again on every read.
    pub ends_at_utc_ms: i64,
    /// EndOfSong and EndOfAlbum: the queue entry after which playback pauses, so
    /// don't preload past it. None for Minutes, or when nothing is queued.
    pub stop_after_uid: Option<i64>,
    /// How long the fade lasts (`motion.timing.sleepTimerFadeOutMs`, 10 s).
    pub fade_ms: i64,
}

/// What the player does now with the volume (`sleep_fade_plan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SleepFadeAction {
    /// Full volume: no timer, a song or album timer (they never fade), or the end
    /// isn't near yet.
    KeepPlaying,
    /// This song ends inside the timer's last `fade_ms`: full volume until
    /// `fade_from_ms` into the song, then down to silence by its end. Don't start
    /// the next song: report this one `Finished` as usual, keep the next loaded
    /// and paused, then call `sleep_timer_ended(0)`.
    StopAtSongEnd,
    /// The timer ends while this song plays: fade to silence over `fade_ms`
    /// from now, pause, then call `sleep_timer_ended(listened_ms)`.
    FadeNow,
}

/// The fade rule, the same on every platform: what to do at this moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SleepFadePlan {
    pub action: SleepFadeAction,
    /// Where in the song the fade starts: StopAtSongEnd, possibly ahead of the
    /// position; FadeNow, the position given. 0 with KeepPlaying.
    pub fade_from_ms: i64,
    /// How long the fade lasts: the timer's `fade_ms` at most, less when the song
    /// or the timer ends sooner. 0 with KeepPlaying, and 0 (stop now) once the end
    /// has passed.
    pub fade_ms: i64,
}

// ---------------------------------------------------------------- widgets (Phase 9, WP2)

/// What a profile's widgets may show (PLAN-UX section 20; board 34, item 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum WidgetPrivacy {
    /// The song, the mixes and the recent albums.
    Normal,
    /// The profile has a PIN: "Profile is locked". No titles and no covers.
    Locked,
    /// A Guest session is on: the neutral "Open Encore" card. No titles and no
    /// covers.
    Guest,
}

/// The song a Now Playing widget shows: the one playing (or paused) now, else
/// the last one played.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WidgetSong {
    /// None when a last-played song has left the library.
    pub song_id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album_id: Option<i64>,
    pub album_title: Option<String>,
    /// A file name in the profile's `art/` folder; None: no cover saved.
    pub cover: Option<String>,
    pub duration_ms: i64,
    /// True: the current song of a play in progress (playing or paused). False:
    /// the last song played; show it with a Play button.
    pub now_playing: bool,
}

/// A mix tile of the Mixes widget and the Quick Settings tile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WidgetMix {
    pub mix_id: String,
    pub title: String,
    /// A file name in the profile's `art/` folder; None: no cover saved.
    pub cover: Option<String>,
    pub pinned: bool,
}

/// An album of the Recently played widget.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WidgetAlbum {
    pub album_id: i64,
    pub title: String,
    pub artist: String,
    /// A file name in the profile's `art/` folder; None: no cover saved.
    pub cover: Option<String>,
}

/// One profile's widget snapshot: what `<data_dir>/widgets/<profile_id>/
/// snapshot.json` holds (schema 1; the file format is in CORE-API.md, Widgets).
/// Under `Locked` and `Guest` everything but the profile ID, `reachable` and the
/// time is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WidgetSnapshot {
    /// 1. A widget that finds a higher number shows "Open Encore".
    pub schema: u32,
    pub profile_id: String,
    pub privacy: WidgetPrivacy,
    pub song: Option<WidgetSong>,
    /// The player's state when it last started, paused, resumed, seeked or
    /// changed song (never from progress ticks): the bar is an indicator.
    pub is_playing: bool,
    pub position_ms: i64,
    /// When `position_ms` was taken; while playing, add the time since. All
    /// three are 0 and false when no song plays now (`song.now_playing` false).
    pub position_at_utc_ms: i64,
    /// Newest pin first, up to 10.
    pub pinned_mixes: Vec<WidgetMix>,
    /// The first mix of today's set not played yet (see `play_mix`).
    pub next_up: Option<WidgetMix>,
    /// Up to 8, newest first, from this profile's play log.
    pub recent_albums: Vec<WidgetAlbum>,
    /// The media server could be reached when this was written: false shows
    /// "Saved mixes / Server not reachable".
    pub reachable: bool,
    pub written_at_utc_ms: i64,
}

/// Which mix `play_mix` starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum MixRefKind {
    /// The mix named by `mix_id`; Next up when it is gone.
    Id,
    /// Next up.
    NextUp,
}

/// A mix chosen on a widget or a tile: `mix_id` is set for `Id`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MixRef {
    pub kind: MixRefKind,
    pub mix_id: Option<String>,
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
    /// `sync_status` is set: the companion server's sync state changed.
    SyncStatusChanged,
    /// `remote_change` is set: another device changed likes, playlists or history;
    /// refresh what's on screen.
    RemoteChange,
    /// `credential` is set: store its token in the keychain under the profile's
    /// companion key (`source_id` 0), replacing the old one; an empty token means
    /// delete it. Store it before anything else: the old one no longer works.
    CredentialChanged,
    /// `output_devices` is set: the output picker's devices changed (board 12).
    DevicesChanged,
    /// `device` is set: offer "Continue from <name>" (board 12, item 1). Shown at
    /// most once per session per device, never while this device plays.
    ContinueAvailable,
    /// `handoff` is set: playback is coming here. Load `handoff.entry` (with
    /// `stream_for`), then call `handoff_ready` and start at the position it returns,
    /// or `handoff_failed` with why.
    HandoffIncoming,
    /// `handoff` is set: the other device has started. Fade out and pause now.
    HandoffConfirmed,
    /// `handoff` is set with `reason`: it didn't happen. A sender keeps playing and
    /// shows why (a toast, the error haptic).
    HandoffFailed,
    /// `remote_command` is set: another of your devices controls this one. Do it
    /// as if the button here was pressed (Play, Pause, Next, Previous, seek,
    /// volume). Queue edits are done already (`QueueChanged` follows).
    RemoteCommand,
    /// `device` is set: the device this one controls changed (now playing,
    /// position, volume): update the "Playing on…" capsule.
    RemoteState,
    /// What is downloaded changed (a song saved or removed, a collection added,
    /// connectivity changed): read `download_marks` again.
    DownloadsChanged,
    /// `download` is set: the transfer moved on (at most every
    /// `downloads.progressEveryMs`).
    DownloadProgress,
    // Phase 9, WP2 (widgets).
    /// `widget_profile_id` is set: that profile's widget snapshot was rewritten,
    /// or deleted with the profile: redraw its widgets from the file.
    WidgetSnapshotChanged,
    // Phase 9, WP1 (sleep timer).
    /// `sleep_timer` is set, or None when no timer runs any more: a timer was set,
    /// extended, cancelled, put back, ended (or went with the music to another
    /// device), or a song or album timer's stop point moved with the queue.
    SleepTimerChanged,
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
    pub sync_status: Option<SyncStatus>,
    pub remote_change: Option<RemoteChange>,
    pub credential: Option<SourceToken>,
    pub output_devices: Option<OutputDevices>,
    pub device: Option<OutputDevice>,
    pub handoff: Option<HandoffInfo>,
    pub remote_command: Option<RemoteCommand>,
    pub download: Option<DownloadProgress>,
    /// Phase 9, WP2: `WidgetSnapshotChanged`'s profile.
    pub widget_profile_id: Option<String>,
    /// Phase 9, WP1: `SleepTimerChanged`'s timer (None: no timer now).
    pub sleep_timer: Option<SleepTimer>,
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
            sync_status: None,
            remote_change: None,
            credential: None,
            output_devices: None,
            device: None,
            handoff: None,
            remote_command: None,
            download: None,
            widget_profile_id: None,
            sleep_timer: None,
        }
    }
}

// ---------------------------------------------------------------- handoff (Phase 7)

/// What another of your devices is doing (board 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DeviceState {
    Playing,
    Paused,
    /// Signed in and reachable, nothing playing.
    Idle,
    /// Not connected now: dimmed; tapping it shakes with the reason. Gone after
    /// 30 days.
    Offline,
}

/// A song another device plays, as it describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RemoteSong {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: i64,
    /// The same song in this device's library, when it has it (for artwork).
    pub song_id: Option<i64>,
}

/// One row of the output picker's "Your devices" (board 12, item 5).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct OutputDevice {
    /// The companion server's ID for it: pass it to `hand_off_to` and the rest.
    pub device_id: String,
    pub name: String,
    /// ios, ipados, macos, android, windows, linux.
    pub platform: String,
    pub this_device: bool,
    pub state: DeviceState,
    /// None when idle, offline, or under Private listening there.
    pub song: Option<RemoteSong>,
    /// Where it is in the song now (estimated between updates).
    pub position_ms: i64,
    /// 0 to 1.
    pub volume: f64,
    /// Private listening is on there: show "Private listening", never the song.
    pub private_listening: bool,
    pub last_seen_utc_ms: i64,
}

/// Whether handoff can be offered at all, and why not (board 12, "States").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum OutputState {
    /// Your devices are listed.
    Ready,
    /// "Sign in on another device to hand off music."
    NoOtherDevices,
    /// The companion can't be reached: the devices section is disabled with
    /// `reason` and a retry.
    Unreachable,
    /// No companion server: hide the devices section.
    NoCompanion,
    /// Guest or no profile: no handoff.
    NotAvailable,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct OutputDevices {
    pub state: OutputState,
    pub reason: Option<String>,
    /// This device first, then the others; offline ones last.
    pub devices: Vec<OutputDevice>,
}

/// A handoff in progress or just finished.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct HandoffInfo {
    pub transfer_id: String,
    /// The other device.
    pub device_id: String,
    pub device_name: String,
    /// HandoffIncoming: the song to load (the queue is in place already).
    pub entry: Option<QueueEntry>,
    /// HandoffIncoming: the position if it started now; `handoff_ready` gives the
    /// exact one.
    pub position_ms: i64,
    /// HandoffIncoming: start playing (false: load it paused).
    pub playing: bool,
    /// 0 to 1: the sender's volume.
    pub volume: f64,
    /// Songs of the queue this device's library doesn't have (left out).
    pub unknown_songs: u32,
    /// HandoffFailed: why, in words for a toast.
    pub reason: Option<String>,
}

/// Remote control (board 12, "Remote control"): what a controller asks the playing
/// device to do. Song IDs are the controller's; the playing device finds them in
/// its own library. `uid`s are the playing device's (from `remote_queue`).
///
/// One record for every command (so Dart reads it too): `kind`, plus the field
/// that kind uses: `position_ms` (SeekTo), `level` (SetVolume, 0 to 1), `song_ids`
/// (PlayNext, AddToUpNext), `uid` (Remove, Move, JumpTo), `to_index` (Move).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RemoteCommand {
    pub kind: RemoteCommandKind,
    pub position_ms: Option<i64>,
    pub level: Option<f64>,
    pub song_ids: Vec<i64>,
    pub uid: Option<i64>,
    pub to_index: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum RemoteCommandKind {
    Play,
    Pause,
    Next,
    Previous,
    SeekTo,
    SetVolume,
    PlayNext,
    AddToUpNext,
    Remove,
    Move,
    JumpTo,
}

impl RemoteCommand {
    /// A command with no other fields set (fill in the one its kind needs).
    pub fn of(kind: RemoteCommandKind) -> RemoteCommand {
        RemoteCommand {
            kind,
            position_ms: None,
            level: None,
            song_ids: Vec::new(),
            uid: None,
            to_index: None,
        }
    }
}

/// One song of the playing device's queue, as the controller sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RemoteQueueItem {
    /// The playing device's uid: pass it back in Remove, Move and JumpTo.
    pub uid: i64,
    pub song: RemoteSong,
    pub chosen_by: ChosenBy,
}

/// The playing device's queue (Now Playing's queue sheet while controlling).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RemoteQueue {
    pub current: Option<RemoteQueueItem>,
    pub up_next: Vec<RemoteQueueItem>,
    pub autoplay: Vec<RemoteQueueItem>,
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
    /// The action isn't allowed here: a Guest saving a playlist, a drop on an album,
    /// a read-only playlist, an Undo that came too late. `reason` says which.
    #[error("{reason}")]
    NotAllowed { reason: String },
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
    NotAllowed,
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
            EncoreError::NotAllowed { .. } => ErrorKind::NotAllowed,
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

impl From<crate::edits::EditError> for EncoreError {
    fn from(e: crate::edits::EditError) -> Self {
        use crate::edits::EditError;
        match e {
            EditError::Store(s) => s.into(),
            EditError::Refused(r) => EncoreError::NotAllowed {
                reason: r.sentence().into(),
            },
            EditError::NotFound(what) => EncoreError::NotFound {
                reason: format!("That {what} isn't in your library"),
            },
            EditError::UndoExpired => EncoreError::NotAllowed {
                reason: "It's too late to undo that".into(),
            },
            EditError::AlreadyUndone => EncoreError::NotAllowed {
                reason: "That was already undone".into(),
            },
            EditError::Poisoned => EncoreError::internal("store lock"),
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
            SourceError::Cancelled => EncoreError::NotAllowed { reason },
            SourceError::Storage(_) => EncoreError::Storage { reason },
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
            sync_level: match l.sync {
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
