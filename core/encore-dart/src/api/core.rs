//! The Encore core for Dart (Windows and Linux): the same calls as `EncoreCore`
//! in Swift and Kotlin (docs/core/CORE-API.md), wrapped for flutter_rust_bridge.
//!
//! Types are the core's own, re-exported, with `#[frb(mirror)]` placeholders that
//! flutter_rust_bridge reads to generate the Dart classes. A placeholder that does
//! not match its type exactly fails to compile, so the two cannot drift apart.
//! Errors arrive as one `ApiError` with a `kind`, because Dart mirrors cannot carry
//! data inside enum variants.

#![allow(dead_code)] // the mirror placeholders are read by the code generator only

use std::sync::Arc;

use flutter_rust_bridge::frb;

pub use encore_core::api::{
    Adventurousness, ArtworkPalette, CompanionDevice, DropPreview, DropRefusal, DropTarget,
    DropTargetKind, EditResult, ExportSummary, GuestSession, Home, InterruptionKind,
    LyricBackground, LyricGap, LyricLine, LyricMatch, LyricSide, LyricWord, LyricsFormat,
    LyricsOrigin, LyricsSync, MixKind, MixPage, MixShelf, MixSong, MixState, MixSummary,
    NotInterestedItem, NotInterestedReason, PaletteScheme, PaletteSwatch, PickReason, RadioSeed,
    ReasonKind, SentEdits, ShelfStat, Singer, SingerKind, SkipOrigin, SkipTarget, SongLyrics,
    SuggestionStats, TimeOfDay, WhyThisSong,
};
pub use encore_core::api::{
    AlbumDetail, AlbumSort, AlbumSummary, ArtistDetail, ArtistSummary, ArtworkOf, ArtworkRequest,
    ChosenBy, CoreConfig, CoreEvent, CredentialKey, EncoreError, EndReason, ErrorKind, EventKind,
    GenreSummary, HttpHeader, ListeningMode, Network, PlaylistDetail, PlaylistEntry,
    PlaylistSummary, PreviousResult, ProfileSummary, QueueEntry, QueueState, QuickConnectStart,
    RemoteChange, RepeatMode, ResultKind, SearchResults, ServerCheck, ServerRow, SignedIn, Song,
    SongSort, SourceToken, StartedFrom, StreamInfo, StreamQuality, SyncResult, SyncState,
    SyncStatus, TopResult, TranscodeCodec,
};
use encore_core::api::{CoreListener, EncoreCore};

use crate::frb_generated::StreamSink;

// ---------------------------------------------------------------- mirrors

#[frb(mirror(CoreConfig))]
pub struct _CoreConfig {
    pub data_dir: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
}

#[frb(mirror(ServerCheck))]
pub struct _ServerCheck {
    pub base_url: String,
    pub name: String,
    pub version: String,
    pub server_id: String,
    pub plain_http_on_local_network: bool,
}

#[frb(mirror(QuickConnectStart))]
pub struct _QuickConnectStart {
    pub code: String,
    pub expires_in_seconds: u32,
}

#[frb(mirror(SignedIn))]
pub struct _SignedIn {
    pub profile_id: String,
    pub source_id: i64,
    pub credential_key: String,
    pub token: String,
    pub user_name: String,
}

#[frb(mirror(SourceToken))]
pub struct _SourceToken {
    pub source_id: i64,
    pub token: String,
}

#[frb(mirror(CredentialKey))]
pub struct _CredentialKey {
    pub source_id: i64,
    pub key: String,
}

#[frb(mirror(ProfileSummary))]
pub struct _ProfileSummary {
    pub profile_id: String,
    pub display_name: String,
    pub credentials: Vec<CredentialKey>,
    pub initials: String,
    pub avatar_url: Option<String>,
    pub locked: bool,
    pub active: bool,
    pub needs_sign_in: bool,
}

#[frb(mirror(Adventurousness))]
pub enum _Adventurousness {
    Familiar,
    Balanced,
    Adventurous,
}

#[frb(mirror(TimeOfDay))]
pub enum _TimeOfDay {
    Night,
    Morning,
    Afternoon,
    Evening,
}

#[frb(mirror(ReasonKind))]
pub enum _ReasonKind {
    FinishedOften,
    PlayedTogether,
    TimeOfDay,
    YouLikedIt,
    Rediscover,
    SameArtist,
    SameGenre,
    PlayedElsewhere,
    NewToYou,
    RecentlyAdded,
}

#[frb(mirror(PickReason))]
pub struct _PickReason {
    pub kind: ReasonKind,
    pub song: Option<Song>,
    pub count: Option<u32>,
    pub of: Option<u32>,
    pub days: Option<u32>,
    pub time_of_day: Option<TimeOfDay>,
    pub artist: Option<String>,
    pub genre: Option<String>,
}

#[frb(mirror(WhyThisSong))]
pub struct _WhyThisSong {
    pub reasons: Vec<PickReason>,
    pub sounds_like: Vec<Song>,
}

#[frb(mirror(NotInterestedReason))]
pub enum _NotInterestedReason {
    NotMyTaste,
    HeardTooMuch,
    WrongMoment,
}

#[frb(mirror(NotInterestedItem))]
pub struct _NotInterestedItem {
    pub song_id: Option<i64>,
    pub artist: Option<String>,
    pub label: String,
    pub reason: NotInterestedReason,
    pub added_utc_ms: i64,
}

#[frb(mirror(MixState))]
pub enum _MixState {
    Fresh,
    OutOfDate,
    NotEnoughPlays,
    LearningOff,
    Private,
}

#[frb(mirror(MixKind))]
pub enum _MixKind {
    Strongest,
    Second,
    TimeOfDay,
    Drift,
    Stretch,
    Family,
    Starter,
}

#[frb(mirror(MixSummary))]
pub struct _MixSummary {
    pub mix_id: String,
    pub kind: MixKind,
    pub title: String,
    pub subtitle: String,
    pub cover_song_ids: Vec<i64>,
    pub song_count: u32,
    pub pinned: bool,
    pub made_utc_ms: i64,
}

#[frb(mirror(MixShelf))]
pub struct _MixShelf {
    pub state: MixState,
    pub mixes: Vec<MixSummary>,
    pub plays_so_far: u32,
    pub plays_needed: u32,
    pub made_utc_ms: Option<i64>,
}

#[frb(mirror(Home))]
pub struct _Home {
    pub made_for_you: MixShelf,
    pub rediscover: Vec<Song>,
    pub recently_played: Vec<Song>,
    pub deep_cuts: Vec<Song>,
    pub recently_added: Vec<Song>,
    pub most_played: Vec<Song>,
}

#[frb(mirror(MixSong))]
pub struct _MixSong {
    pub song: Song,
    pub reason: Option<PickReason>,
    pub new_to_you: bool,
}

#[frb(mirror(MixPage))]
pub struct _MixPage {
    pub summary: MixSummary,
    pub songs: Vec<MixSong>,
}

#[frb(mirror(ShelfStat))]
pub struct _ShelfStat {
    pub shelf: String,
    pub plays: u32,
    pub early_skips_per_1000: u32,
    pub finished_per_1000: u32,
    pub artists_per_20_x10: u32,
    pub repeats_per_1000: u32,
}

#[frb(mirror(SuggestionStats))]
pub struct _SuggestionStats {
    pub since_utc_ms: i64,
    pub shelves: Vec<ShelfStat>,
}

#[frb(mirror(RadioSeed))]
pub enum _RadioSeed {
    Song,
    Album,
    Artist,
    Playlist,
}

#[frb(mirror(GuestSession))]
pub struct _GuestSession {
    pub started_utc_ms: i64,
    pub ends_if_idle_utc_ms: i64,
}

#[frb(mirror(ServerRow))]
pub struct _ServerRow {
    pub source_id: i64,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub version: Option<String>,
    pub user_name: String,
    pub song_count: u32,
    pub last_sync_utc_ms: Option<i64>,
    pub signed_in: bool,
}

#[frb(mirror(SyncResult))]
pub struct _SyncResult {
    pub source_id: i64,
    pub artists: u32,
    pub albums: u32,
    pub songs: u32,
    pub playlists: u32,
    pub removed: u32,
}

#[frb(mirror(Song))]
pub struct _Song {
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

#[frb(mirror(AlbumSummary))]
pub struct _AlbumSummary {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub song_count: u32,
    pub duration_ms: i64,
    pub date_added_utc_ms: Option<i64>,
    pub has_artwork: bool,
}

#[frb(mirror(AlbumDetail))]
pub struct _AlbumDetail {
    pub album: AlbumSummary,
    pub songs: Vec<Song>,
}

#[frb(mirror(ArtistSummary))]
pub struct _ArtistSummary {
    pub id: i64,
    pub name: String,
    pub album_count: u32,
}

#[frb(mirror(ArtistDetail))]
pub struct _ArtistDetail {
    pub artist: ArtistSummary,
    pub albums: Vec<AlbumSummary>,
}

#[frb(mirror(PlaylistSummary))]
pub struct _PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub song_count: u32,
    pub duration_ms: i64,
}

#[frb(mirror(PlaylistEntry))]
pub struct _PlaylistEntry {
    pub position: u32,
    pub song: Option<Song>,
}

#[frb(mirror(PlaylistDetail))]
pub struct _PlaylistDetail {
    pub playlist: PlaylistSummary,
    pub entries: Vec<PlaylistEntry>,
}

#[frb(mirror(GenreSummary))]
pub struct _GenreSummary {
    pub name: String,
    pub song_count: u32,
}

#[frb(mirror(TopResult))]
pub struct _TopResult {
    pub kind: ResultKind,
    pub song: Option<Song>,
    pub album: Option<AlbumSummary>,
    pub artist: Option<ArtistSummary>,
    pub playlist: Option<PlaylistSummary>,
}

#[frb(mirror(SearchResults))]
pub struct _SearchResults {
    pub top: Option<TopResult>,
    pub songs: Vec<Song>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
    pub lyrics: Vec<LyricMatch>,
}

#[frb(mirror(ExportSummary))]
pub struct _ExportSummary {
    pub plays: u64,
    pub likes: u64,
    pub playlists: u64,
    pub playlist_songs: u64,
    pub bytes: u64,
}

#[frb(mirror(LyricMatch))]
pub struct _LyricMatch {
    pub song: Song,
    pub line: String,
    pub line_index: u32,
    pub start_ms: Option<i64>,
}

#[frb(mirror(QueueEntry))]
pub struct _QueueEntry {
    pub uid: i64,
    pub song: Song,
    pub chosen_by: ChosenBy,
}

#[frb(mirror(QueueState))]
pub struct _QueueState {
    pub history: Vec<QueueEntry>,
    pub current: Option<QueueEntry>,
    pub up_next: Vec<QueueEntry>,
    pub autoplay: Vec<QueueEntry>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub autoplay_enabled: bool,
    pub can_undo: bool,
}

#[frb(mirror(HttpHeader))]
pub struct _HttpHeader {
    pub name: String,
    pub value: String,
}

#[frb(mirror(StreamInfo))]
pub struct _StreamInfo {
    pub song_id: i64,
    pub url: String,
    pub url_with_api_key: String,
    pub headers: Vec<HttpHeader>,
    pub may_transcode: bool,
}

#[frb(mirror(ArtworkRequest))]
pub struct _ArtworkRequest {
    pub url: String,
    pub headers: Vec<HttpHeader>,
}

#[frb(mirror(CoreEvent))]
pub struct _CoreEvent {
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
}

#[frb(mirror(SyncState))]
pub enum _SyncState {
    NotConnected,
    Idle,
    Syncing,
    Offline,
    SignedOut,
}

#[frb(mirror(SyncStatus))]
pub struct _SyncStatus {
    pub state: SyncState,
    pub pending: u32,
    pub last_synced_utc_ms: Option<i64>,
    pub server_url: Option<String>,
    pub reason: Option<String>,
}

#[frb(mirror(RemoteChange))]
pub struct _RemoteChange {
    pub liked_song_ids: Vec<i64>,
    pub unliked_song_ids: Vec<i64>,
    pub playlist_ids: Vec<i64>,
    pub history: bool,
    pub library_sync_suggested: bool,
}

#[frb(mirror(CompanionDevice))]
pub struct _CompanionDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub added_utc_ms: i64,
    pub last_seen_utc_ms: i64,
    pub signed_out: bool,
    pub this_device: bool,
}

#[frb(mirror(AlbumSort))]
pub enum _AlbumSort {
    Title,
    Artist,
    Year,
    RecentlyAdded,
}

#[frb(mirror(SongSort))]
pub enum _SongSort {
    Title,
    Artist,
    Album,
    RecentlyAdded,
}

#[frb(mirror(ResultKind))]
pub enum _ResultKind {
    Song,
    Album,
    Artist,
    Playlist,
}

#[frb(mirror(StartedFrom))]
pub enum _StartedFrom {
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

#[frb(mirror(ChosenBy))]
pub enum _ChosenBy {
    User,
    App,
}

#[frb(mirror(RepeatMode))]
pub enum _RepeatMode {
    Off,
    All,
    One,
}

#[frb(mirror(ListeningMode))]
pub enum _ListeningMode {
    Normal,
    Private,
    Guest,
    LearningOff,
}

#[frb(mirror(EndReason))]
pub enum _EndReason {
    Finished,
    PausedAbandoned,
    Interrupted,
    Error,
    EndedBySystem,
}

#[frb(mirror(PreviousResult))]
pub enum _PreviousResult {
    Restarted,
    Moved,
}

#[frb(mirror(Network))]
pub enum _Network {
    Wifi,
    Cellular,
}

#[frb(mirror(StreamQuality))]
pub enum _StreamQuality {
    Original,
    High,
    Normal,
    DataSaver,
}

#[frb(mirror(TranscodeCodec))]
pub enum _TranscodeCodec {
    Aac,
    Opus,
}

#[frb(mirror(ArtworkOf))]
pub enum _ArtworkOf {
    Album,
    Song,
    Artist,
    Playlist,
}

#[frb(mirror(EventKind))]
pub enum _EventKind {
    QueueChanged,
    NowPlaying,
    Preload,
    SyncProgress,
    SyncStatusChanged,
    RemoteChange,
    CredentialChanged,
}

#[frb(mirror(ErrorKind))]
pub enum _ErrorKind {
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

// Phase 3: skips, lyrics, palettes, playlist writes.

#[frb(mirror(SkipOrigin))]
pub enum _SkipOrigin {
    Button,
    Swipe,
    Headset,
    Car,
    LockScreen,
    Keyboard,
}

#[frb(mirror(InterruptionKind))]
pub enum _InterruptionKind {
    HeadphonesUnplugged,
    OutputChanged,
    Call,
    Assistant,
}

#[frb(mirror(SkipTarget))]
pub struct _SkipTarget {
    pub song: Option<QueueEntry>,
    pub end_of_queue: bool,
}

#[frb(mirror(LyricsSync))]
pub enum _LyricsSync {
    Word,
    Line,
    Unsynced,
}

#[frb(mirror(LyricsOrigin))]
pub enum _LyricsOrigin {
    MediaServer,
    EmbeddedTag,
    Sidecar,
    Provider,
    Generated,
}

#[frb(mirror(LyricsFormat))]
pub enum _LyricsFormat {
    Jellyfin,
    Lrc,
    EnhancedLrc,
    Ttml,
    Plain,
}

#[frb(mirror(LyricSide))]
pub enum _LyricSide {
    Main,
    Opposite,
}

#[frb(mirror(SingerKind))]
pub enum _SingerKind {
    Person,
    Group,
    Other,
}

#[frb(mirror(Singer))]
pub struct _Singer {
    pub id: String,
    pub name: Option<String>,
    pub kind: SingerKind,
}

#[frb(mirror(LyricWord))]
pub struct _LyricWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub space_after: bool,
}

#[frb(mirror(LyricBackground))]
pub struct _LyricBackground {
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub words: Vec<LyricWord>,
}

#[frb(mirror(LyricLine))]
pub struct _LyricLine {
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub words: Vec<LyricWord>,
    pub singer: Option<u32>,
    pub side: LyricSide,
    pub background: Vec<LyricBackground>,
    pub confidence: f64,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
}

#[frb(mirror(LyricGap))]
pub struct _LyricGap {
    pub after_line: Option<u32>,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[frb(mirror(SongLyrics))]
pub struct _SongLyrics {
    pub song_id: i64,
    pub sync_level: LyricsSync,
    pub origin: LyricsOrigin,
    pub format: LyricsFormat,
    pub lines: Vec<LyricLine>,
    pub singers: Vec<Singer>,
    pub gaps: Vec<LyricGap>,
    pub instrumental: bool,
    pub language: Option<String>,
    pub fetched_utc_ms: i64,
}

#[frb(mirror(PaletteScheme))]
pub enum _PaletteScheme {
    Dark,
    Light,
}

#[frb(mirror(PaletteSwatch))]
pub struct _PaletteSwatch {
    pub colour: String,
    pub share: f64,
}

#[frb(mirror(ArtworkPalette))]
pub struct _ArtworkPalette {
    pub scheme: PaletteScheme,
    pub base: String,
    pub stops: Vec<String>,
    pub text: String,
    pub lowest_contrast: f64,
    pub sampled: Vec<PaletteSwatch>,
}

#[frb(mirror(DropTargetKind))]
pub enum _DropTargetKind {
    Playlist,
    NewPlaylist,
    LikedSongs,
    UpNext,
    PlayNext,
    Album,
}

#[frb(mirror(DropTarget))]
pub struct _DropTarget {
    pub kind: DropTargetKind,
    pub id: Option<i64>,
}

#[frb(mirror(DropRefusal))]
pub enum _DropRefusal {
    GuestCannotSave,
    AlbumsTakeNoSongs,
    PlaylistReadOnly,
    NothingToAdd,
}

#[frb(mirror(DropPreview))]
pub struct _DropPreview {
    pub count: u32,
    pub already_there: u32,
    pub needs_confirm: bool,
    pub suggested_name: Option<String>,
    pub refusal: Option<DropRefusal>,
    pub refusal_reason: Option<String>,
}

#[frb(mirror(EditResult))]
pub struct _EditResult {
    pub edit_id: Option<String>,
    pub added: u32,
    pub already_there: u32,
    pub playlist_id: Option<i64>,
    pub undo_until_utc_ms: Option<i64>,
}

#[frb(mirror(SentEdits))]
pub struct _SentEdits {
    pub sent: u32,
    pub refused: u32,
    pub waiting: u32,
}

// ---------------------------------------------------------------- errors

/// Every failure, flattened for Dart. `message` is the user-facing sentence;
/// `sha256` is set for CertificateUntrusted, `status` and `detail` for Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub kind: ErrorKind,
    pub message: String,
    pub sha256: Option<String>,
    pub status: Option<u16>,
    pub detail: Option<String>,
}

impl From<EncoreError> for ApiError {
    fn from(e: EncoreError) -> Self {
        let kind = e.kind();
        let message = e.message();
        let (sha256, status, detail) = match e {
            EncoreError::CertificateUntrusted { sha256, .. } => (Some(sha256), None, None),
            EncoreError::Server { status, detail, .. } => (None, Some(status), Some(detail)),
            _ => (None, None, None),
        };
        ApiError {
            kind,
            message,
            sha256,
            status,
            detail,
        }
    }
}

struct SinkListener(StreamSink<CoreEvent>);

impl CoreListener for SinkListener {
    fn on_event(&self, event: CoreEvent) {
        // A closed stream (the app stopped listening) is not an error.
        let _ = self.0.add(event);
    }
}

// ---------------------------------------------------------------- the handle

/// One per app. Calls marked sync return at once (they read the on-device mirror);
/// the others are Futures in Dart.
#[frb(opaque)]
pub struct Encore {
    core: Arc<EncoreCore>,
}

impl Encore {
    /// Start the core. `config.data_dir` must be a folder the app can write.
    #[frb(sync)]
    pub fn create(config: CoreConfig) -> Result<Encore, ApiError> {
        Ok(Encore {
            core: EncoreCore::new(config)?,
        })
    }

    /// Events: queue changed, now playing, preload, sync progress. One stream at a
    /// time: calling this again replaces the previous stream.
    pub fn events(&self, sink: StreamSink<CoreEvent>) {
        self.core.set_listener(Arc::new(SinkListener(sink)));
    }

    /// Close the events stream (it reports done). Cancelling a subscription alone
    /// does not end it, because the core still holds the stream's sink.
    #[frb(sync)]
    pub fn stop_events(&self) {
        self.core.clear_listener()
    }

    // connect
    pub async fn check_address(&self, address: String) -> Result<ServerCheck, ApiError> {
        Ok(self.core.check_address(address).await?)
    }
    #[frb(sync)]
    pub fn trust_certificate(&self, address: String, sha256: String) {
        self.core.trust_certificate(address, sha256)
    }
    pub async fn sign_in(
        &self,
        base_url: String,
        username: String,
        password: String,
    ) -> Result<SignedIn, ApiError> {
        Ok(self.core.sign_in(base_url, username, password).await?)
    }
    pub async fn quick_connect_start(
        &self,
        base_url: String,
    ) -> Result<QuickConnectStart, ApiError> {
        Ok(self.core.quick_connect_start(base_url).await?)
    }
    pub async fn quick_connect_wait(&self) -> Result<SignedIn, ApiError> {
        Ok(self.core.quick_connect_wait().await?)
    }
    #[frb(sync)]
    pub fn quick_connect_cancel(&self) {
        self.core.quick_connect_cancel()
    }

    // profiles
    #[frb(sync)]
    pub fn profiles(&self) -> Result<Vec<ProfileSummary>, ApiError> {
        Ok(self.core.profiles()?)
    }

    // Phase 5: smart picks
    #[frb(sync)]
    pub fn refresh_autoplay(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.refresh_autoplay()?)
    }
    #[frb(sync)]
    pub fn start_radio(&self, seed: RadioSeed, id: i64) -> Result<QueueState, ApiError> {
        Ok(self.core.start_radio(seed, id)?)
    }
    #[frb(sync)]
    pub fn smart_shuffle(
        &self,
        song_ids: Vec<i64>,
        started_from: StartedFrom,
        context_id: Option<String>,
    ) -> Result<QueueState, ApiError> {
        Ok(self
            .core
            .smart_shuffle(song_ids, started_from, context_id)?)
    }
    #[frb(sync)]
    pub fn why_this_song(&self, song_id: i64) -> Result<WhyThisSong, ApiError> {
        Ok(self.core.why_this_song(song_id)?)
    }
    #[frb(sync)]
    pub fn more_like_this(&self, song_id: i64) -> Result<(), ApiError> {
        Ok(self.core.more_like_this(song_id)?)
    }
    #[frb(sync)]
    pub fn fewer_like_this(&self, song_id: i64) -> Result<(), ApiError> {
        Ok(self.core.fewer_like_this(song_id)?)
    }
    #[frb(sync)]
    pub fn not_interested_song(
        &self,
        song_id: i64,
        reason: NotInterestedReason,
    ) -> Result<NotInterestedItem, ApiError> {
        Ok(self.core.not_interested_song(song_id, reason)?)
    }
    #[frb(sync)]
    pub fn not_interested_artist(
        &self,
        artist: String,
        reason: NotInterestedReason,
    ) -> Result<NotInterestedItem, ApiError> {
        Ok(self.core.not_interested_artist(artist, reason)?)
    }
    #[frb(sync)]
    pub fn remove_not_interested(&self, item: NotInterestedItem) -> Result<(), ApiError> {
        Ok(self.core.remove_not_interested(item)?)
    }
    #[frb(sync)]
    pub fn not_interested_list(&self) -> Result<Vec<NotInterestedItem>, ApiError> {
        Ok(self.core.not_interested_list()?)
    }
    #[frb(sync)]
    pub fn clear_not_interested(&self) -> Result<(), ApiError> {
        Ok(self.core.clear_not_interested()?)
    }
    #[frb(sync)]
    pub fn set_adventurousness(&self, level: Adventurousness) -> Result<(), ApiError> {
        Ok(self.core.set_adventurousness(level)?)
    }
    #[frb(sync)]
    pub fn adventurousness(&self) -> Result<Adventurousness, ApiError> {
        Ok(self.core.adventurousness()?)
    }
    #[frb(sync)]
    pub fn reset_suggestions(&self) -> Result<(), ApiError> {
        Ok(self.core.reset_suggestions()?)
    }
    #[frb(sync)]
    pub fn clear_listening_history(&self) -> Result<u32, ApiError> {
        Ok(self.core.clear_listening_history()?)
    }
    pub async fn home(&self) -> Result<Home, ApiError> {
        Ok(self.core.home().await?)
    }
    pub async fn regenerate_mixes(&self) -> Result<Home, ApiError> {
        Ok(self.core.regenerate_mixes().await?)
    }
    #[frb(sync)]
    pub fn mix(&self, mix_id: String) -> Result<MixPage, ApiError> {
        Ok(self.core.mix(mix_id)?)
    }
    #[frb(sync)]
    pub fn pin_mix(&self, mix_id: String, pinned: bool) -> Result<(), ApiError> {
        Ok(self.core.pin_mix(mix_id, pinned)?)
    }
    #[frb(sync)]
    pub fn hide_mix(&self, mix_id: String) -> Result<(), ApiError> {
        Ok(self.core.hide_mix(mix_id)?)
    }
    #[frb(sync)]
    pub fn not_in_this_mix(&self, mix_id: String, song_id: i64) -> Result<(), ApiError> {
        Ok(self.core.not_in_this_mix(mix_id, song_id)?)
    }
    #[frb(sync)]
    pub fn save_mix_as_playlist(&self, mix_id: String) -> Result<EditResult, ApiError> {
        Ok(self.core.save_mix_as_playlist(mix_id)?)
    }
    #[frb(sync)]
    pub fn suggestion_stats(&self) -> Result<SuggestionStats, ApiError> {
        Ok(self.core.suggestion_stats()?)
    }

    // Phase 4: profiles on this device (board 14)
    pub async fn switch_profile(
        &self,
        profile_id: String,
        tokens: Vec<SourceToken>,
        keep_playing: bool,
    ) -> Result<QueueState, ApiError> {
        Ok(self
            .core
            .switch_profile(profile_id, tokens, keep_playing)
            .await?)
    }
    #[frb(sync)]
    pub fn unlock_profile(&self, profile_id: String, pin: Option<String>) -> Result<(), ApiError> {
        Ok(self.core.unlock_profile(profile_id, pin)?)
    }
    #[frb(sync)]
    pub fn set_profile_pin(&self, profile_id: String, pin: String) -> Result<(), ApiError> {
        Ok(self.core.set_profile_pin(profile_id, pin)?)
    }
    #[frb(sync)]
    pub fn remove_profile_pin(&self, profile_id: String) -> Result<(), ApiError> {
        Ok(self.core.remove_profile_pin(profile_id)?)
    }
    #[frb(sync)]
    pub fn biometrics_changed(&self) -> Result<(), ApiError> {
        Ok(self.core.biometrics_changed()?)
    }
    #[frb(sync)]
    pub fn rename_profile(&self, profile_id: String, name: Option<String>) -> Result<(), ApiError> {
        Ok(self.core.rename_profile(profile_id, name)?)
    }
    #[frb(sync)]
    pub fn reorder_profiles(&self, profile_ids: Vec<String>) -> Result<(), ApiError> {
        Ok(self.core.reorder_profiles(profile_ids)?)
    }
    pub fn remove_profile_from_device(&self, profile_id: String) -> Result<(), ApiError> {
        Ok(self.core.remove_profile_from_device(profile_id)?)
    }
    #[frb(sync)]
    pub fn last_active_profile(&self) -> Result<Option<String>, ApiError> {
        Ok(self.core.last_active_profile()?)
    }
    #[frb(sync)]
    pub fn start_guest(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.start_guest()?)
    }
    #[frb(sync)]
    pub fn end_guest(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.end_guest()?)
    }
    #[frb(sync)]
    pub fn guest_session(&self) -> Result<Option<GuestSession>, ApiError> {
        Ok(self.core.guest_session()?)
    }
    #[frb(sync)]
    pub fn open_profile(
        &self,
        profile_id: String,
        tokens: Vec<SourceToken>,
    ) -> Result<(), ApiError> {
        Ok(self.core.open_profile(profile_id, tokens)?)
    }
    #[frb(sync)]
    pub fn close_profile(&self) -> Result<(), ApiError> {
        Ok(self.core.close_profile()?)
    }
    #[frb(sync)]
    pub fn current_profile(&self) -> Option<String> {
        self.core.current_profile()
    }

    // servers and sync
    #[frb(sync)]
    pub fn servers(&self) -> Result<Vec<ServerRow>, ApiError> {
        Ok(self.core.servers()?)
    }
    pub async fn sync_library(&self) -> Result<Vec<SyncResult>, ApiError> {
        Ok(self.core.sync_library().await?)
    }

    // Phase 4: the companion server
    pub async fn connect_companion(&self, url: String) -> Result<SyncStatus, ApiError> {
        Ok(self.core.connect_companion(url).await?)
    }
    pub async fn disconnect_companion(&self) -> Result<(), ApiError> {
        Ok(self.core.disconnect_companion().await?)
    }
    #[frb(sync)]
    pub fn sync_status(&self) -> Result<SyncStatus, ApiError> {
        Ok(self.core.sync_status()?)
    }
    pub async fn sync_now(&self) -> Result<SyncStatus, ApiError> {
        Ok(self.core.sync_now().await?)
    }
    pub async fn companion_devices(&self) -> Result<Vec<CompanionDevice>, ApiError> {
        Ok(self.core.companion_devices().await?)
    }
    pub async fn delete_profile_everywhere(
        &self,
        profile_id: String,
        typed_name: String,
    ) -> Result<(), ApiError> {
        Ok(self
            .core
            .delete_profile_everywhere(profile_id, typed_name)
            .await?)
    }
    pub async fn revoke_device(&self, device_id: String) -> Result<(), ApiError> {
        Ok(self.core.revoke_device(device_id).await?)
    }
    pub async fn sign_out(&self, source_id: i64) -> Result<(), ApiError> {
        Ok(self.core.sign_out(source_id).await?)
    }
    #[frb(sync)]
    pub fn remove_server(&self, source_id: i64) -> Result<(), ApiError> {
        Ok(self.core.remove_server(source_id)?)
    }

    // browse
    #[frb(sync)]
    pub fn albums(
        &self,
        sort: AlbumSort,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<AlbumSummary>, ApiError> {
        Ok(self.core.albums(sort, offset, limit)?)
    }
    #[frb(sync)]
    pub fn album(&self, album_id: i64) -> Result<AlbumDetail, ApiError> {
        Ok(self.core.album(album_id)?)
    }
    #[frb(sync)]
    pub fn songs(&self, sort: SongSort, offset: u32, limit: u32) -> Result<Vec<Song>, ApiError> {
        Ok(self.core.songs(sort, offset, limit)?)
    }
    #[frb(sync)]
    pub fn artists(&self, offset: u32, limit: u32) -> Result<Vec<ArtistSummary>, ApiError> {
        Ok(self.core.artists(offset, limit)?)
    }
    #[frb(sync)]
    pub fn artist(&self, artist_id: i64) -> Result<ArtistDetail, ApiError> {
        Ok(self.core.artist(artist_id)?)
    }
    #[frb(sync)]
    pub fn artist_songs(
        &self,
        artist_id: i64,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Song>, ApiError> {
        Ok(self.core.artist_songs(artist_id, offset, limit)?)
    }
    #[frb(sync)]
    pub fn playlists(&self, offset: u32, limit: u32) -> Result<Vec<PlaylistSummary>, ApiError> {
        Ok(self.core.playlists(offset, limit)?)
    }
    #[frb(sync)]
    pub fn playlist(&self, playlist_id: i64) -> Result<PlaylistDetail, ApiError> {
        Ok(self.core.playlist(playlist_id)?)
    }
    #[frb(sync)]
    pub fn genres(&self) -> Result<Vec<GenreSummary>, ApiError> {
        Ok(self.core.genres()?)
    }
    #[frb(sync)]
    pub fn genre_songs(
        &self,
        genre: String,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Song>, ApiError> {
        Ok(self.core.genre_songs(genre, offset, limit)?)
    }
    #[frb(sync)]
    pub fn song(&self, song_id: i64) -> Result<Song, ApiError> {
        Ok(self.core.song(song_id)?)
    }

    // search
    #[frb(sync)]
    pub fn search(&self, query: String) -> Result<SearchResults, ApiError> {
        Ok(self.core.search(query)?)
    }
    #[frb(sync)]
    pub fn add_recent_search(&self, query: String) -> Result<(), ApiError> {
        Ok(self.core.add_recent_search(query)?)
    }
    #[frb(sync)]
    pub fn recent_searches(&self) -> Result<Vec<String>, ApiError> {
        Ok(self.core.recent_searches()?)
    }
    #[frb(sync)]
    pub fn remove_recent_search(&self, query: String) -> Result<(), ApiError> {
        Ok(self.core.remove_recent_search(query)?)
    }
    #[frb(sync)]
    pub fn clear_recent_searches(&self) -> Result<(), ApiError> {
        Ok(self.core.clear_recent_searches()?)
    }

    // queue
    #[frb(sync)]
    pub fn queue(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.queue()?)
    }
    #[frb(sync)]
    pub fn play(
        &self,
        song_ids: Vec<i64>,
        start_index: u32,
        started_from: StartedFrom,
        context_id: Option<String>,
    ) -> Result<QueueState, ApiError> {
        Ok(self
            .core
            .play(song_ids, start_index, started_from, context_id)?)
    }
    #[frb(sync)]
    pub fn play_next(&self, song_ids: Vec<i64>) -> Result<QueueState, ApiError> {
        Ok(self.core.play_next(song_ids)?)
    }
    #[frb(sync)]
    pub fn add_to_up_next(&self, song_ids: Vec<i64>) -> Result<QueueState, ApiError> {
        Ok(self.core.add_to_up_next(song_ids)?)
    }
    #[frb(sync)]
    pub fn next(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.next()?)
    }
    #[frb(sync)]
    pub fn previous(&self, position_ms: i64, from_swipe: bool) -> Result<PreviousResult, ApiError> {
        Ok(self.core.previous(position_ms, from_swipe)?)
    }
    #[frb(sync)]
    pub fn jump_to(&self, uid: i64) -> Result<QueueState, ApiError> {
        Ok(self.core.jump_to(uid)?)
    }
    #[frb(sync)]
    pub fn replay_from_history(&self, uid: i64) -> Result<QueueState, ApiError> {
        Ok(self.core.replay_from_history(uid)?)
    }
    #[frb(sync)]
    pub fn remove_from_queue(&self, uid: i64) -> Result<QueueState, ApiError> {
        Ok(self.core.remove_from_queue(uid)?)
    }
    #[frb(sync)]
    pub fn move_in_queue(&self, uid: i64, to_index: u32) -> Result<QueueState, ApiError> {
        Ok(self.core.move_in_queue(uid, to_index)?)
    }
    #[frb(sync)]
    pub fn clear_up_next(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.clear_up_next()?)
    }
    #[frb(sync)]
    pub fn undo_queue(&self) -> Result<QueueState, ApiError> {
        Ok(self.core.undo_queue()?)
    }
    #[frb(sync)]
    pub fn set_shuffle(&self, on: bool) -> Result<QueueState, ApiError> {
        Ok(self.core.set_shuffle(on)?)
    }
    #[frb(sync)]
    pub fn set_repeat(&self, mode: RepeatMode) -> Result<QueueState, ApiError> {
        Ok(self.core.set_repeat(mode)?)
    }
    #[frb(sync)]
    pub fn set_autoplay(&self, enabled: bool) -> Result<QueueState, ApiError> {
        Ok(self.core.set_autoplay(enabled)?)
    }

    // playback reports
    #[frb(sync)]
    pub fn playback_started(&self, uid: i64, utc_offset_minutes: i32) -> Result<(), ApiError> {
        Ok(self.core.playback_started(uid, utc_offset_minutes)?)
    }
    #[frb(sync)]
    pub fn playback_progress(&self, position_ms: i64, listened_ms: i64) -> Result<(), ApiError> {
        Ok(self.core.playback_progress(position_ms, listened_ms)?)
    }
    #[frb(sync)]
    pub fn playback_ended(
        &self,
        reason: EndReason,
        listened_ms: i64,
    ) -> Result<Option<QueueEntry>, ApiError> {
        Ok(self.core.playback_ended(reason, listened_ms)?)
    }
    #[frb(sync)]
    pub fn lyric_line_tapped(&self, line_index: u32, line_start_ms: i64) -> Result<(), ApiError> {
        Ok(self.core.lyric_line_tapped(line_index, line_start_ms)?)
    }
    #[frb(sync)]
    pub fn set_listening_mode(&self, mode: ListeningMode) -> Result<(), ApiError> {
        Ok(self.core.set_listening_mode(mode)?)
    }

    // streams, artwork, lyrics
    #[frb(sync)]
    pub fn stream_for(&self, song_id: i64, network: Network) -> Result<StreamInfo, ApiError> {
        Ok(self.core.stream_for(song_id, network)?)
    }
    #[frb(sync)]
    pub fn stream_quality(&self, network: Network) -> Result<StreamQuality, ApiError> {
        Ok(self.core.stream_quality(network)?)
    }
    #[frb(sync)]
    pub fn set_stream_quality(
        &self,
        network: Network,
        quality: StreamQuality,
    ) -> Result<(), ApiError> {
        Ok(self.core.set_stream_quality(network, quality)?)
    }
    #[frb(sync)]
    pub fn set_playback_capabilities(
        &self,
        direct_play_containers: Vec<String>,
        transcode_codec: TranscodeCodec,
    ) -> Result<(), ApiError> {
        Ok(self
            .core
            .set_playback_capabilities(direct_play_containers, transcode_codec)?)
    }
    #[frb(sync)]
    pub fn artwork(
        &self,
        of: ArtworkOf,
        id: i64,
        size_px: u32,
    ) -> Result<ArtworkRequest, ApiError> {
        Ok(self.core.artwork(of, id, size_px)?)
    }
    pub async fn lyrics_raw(&self, song_id: i64) -> Result<Option<String>, ApiError> {
        Ok(self.core.lyrics_raw(song_id).await?)
    }

    // Phase 3: backups
    pub fn backup_profile(&self, profile_id: String, dest_path: String) -> Result<(), ApiError> {
        Ok(self.core.backup_profile(profile_id, dest_path)?)
    }
    pub fn export_profile(
        &self,
        profile_id: String,
        dest_path: String,
    ) -> Result<ExportSummary, ApiError> {
        Ok(self.core.export_profile(profile_id, dest_path)?)
    }
    pub fn restore_profile(&self, profile_id: String, from_path: String) -> Result<(), ApiError> {
        Ok(self.core.restore_profile(profile_id, from_path)?)
    }

    // Phase 3: lyrics and palettes
    pub async fn lyrics(&self, song_id: i64) -> Result<Option<SongLyrics>, ApiError> {
        Ok(self.core.lyrics(song_id).await?)
    }
    pub async fn fetch_lyrics_ahead(&self, max_songs: u32) -> Result<u32, ApiError> {
        Ok(self.core.fetch_lyrics_ahead(max_songs).await?)
    }
    pub async fn album_palette(&self, album_id: i64) -> Result<Option<ArtworkPalette>, ApiError> {
        Ok(self.core.album_palette(album_id).await?)
    }
    pub async fn playlist_palette(
        &self,
        playlist_id: i64,
    ) -> Result<Option<ArtworkPalette>, ApiError> {
        Ok(self.core.playlist_palette(playlist_id).await?)
    }

    // Phase 3: skips
    #[frb(sync)]
    pub fn skip_next(&self, origin: SkipOrigin) -> Result<QueueState, ApiError> {
        Ok(self.core.skip_next(origin)?)
    }
    #[frb(sync)]
    pub fn skip_previous(
        &self,
        origin: SkipOrigin,
        position_ms: i64,
    ) -> Result<PreviousResult, ApiError> {
        Ok(self.core.skip_previous(origin, position_ms)?)
    }
    #[frb(sync)]
    pub fn next_target(&self) -> Result<SkipTarget, ApiError> {
        Ok(self.core.next_target()?)
    }
    #[frb(sync)]
    pub fn previous_target(&self) -> Result<SkipTarget, ApiError> {
        Ok(self.core.previous_target()?)
    }
    #[frb(sync)]
    pub fn playback_interrupted(&self, kind: InterruptionKind) -> Result<(), ApiError> {
        Ok(self.core.playback_interrupted(kind)?)
    }
    #[frb(sync)]
    pub fn playback_seeked(&self, from_ms: i64, to_ms: i64) -> Result<(), ApiError> {
        Ok(self.core.playback_seeked(from_ms, to_ms)?)
    }

    // Phase 3: playlists and likes
    #[frb(sync)]
    pub fn drop_preview(
        &self,
        target: DropTarget,
        song_ids: Vec<i64>,
    ) -> Result<DropPreview, ApiError> {
        Ok(self.core.drop_preview(target, song_ids)?)
    }
    #[frb(sync)]
    pub fn add_to(
        &self,
        target: DropTarget,
        song_ids: Vec<i64>,
        position: Option<u32>,
        allow_duplicates: bool,
    ) -> Result<EditResult, ApiError> {
        Ok(self
            .core
            .add_to(target, song_ids, position, allow_duplicates)?)
    }
    #[frb(sync)]
    pub fn like(&self, song_ids: Vec<i64>) -> Result<EditResult, ApiError> {
        Ok(self.core.like(song_ids)?)
    }
    #[frb(sync)]
    pub fn unlike(&self, song_ids: Vec<i64>) -> Result<EditResult, ApiError> {
        Ok(self.core.unlike(song_ids)?)
    }
    #[frb(sync)]
    pub fn are_liked(&self, song_ids: Vec<i64>) -> Result<Vec<bool>, ApiError> {
        Ok(self.core.are_liked(song_ids)?)
    }
    #[frb(sync)]
    pub fn liked_songs(&self, offset: u32, limit: u32) -> Result<Vec<Song>, ApiError> {
        Ok(self.core.liked_songs(offset, limit)?)
    }
    #[frb(sync)]
    pub fn remove_from_playlist(
        &self,
        playlist_id: i64,
        positions: Vec<u32>,
    ) -> Result<EditResult, ApiError> {
        Ok(self.core.remove_from_playlist(playlist_id, positions)?)
    }
    #[frb(sync)]
    pub fn move_in_playlist(
        &self,
        playlist_id: i64,
        from_position: u32,
        to_position: u32,
    ) -> Result<EditResult, ApiError> {
        Ok(self
            .core
            .move_in_playlist(playlist_id, from_position, to_position)?)
    }
    #[frb(sync)]
    pub fn rename_playlist(&self, playlist_id: i64, name: String) -> Result<EditResult, ApiError> {
        Ok(self.core.rename_playlist(playlist_id, name)?)
    }
    #[frb(sync)]
    pub fn undo(&self, edit_id: String) -> Result<(), ApiError> {
        Ok(self.core.undo(edit_id)?)
    }
    #[frb(sync)]
    pub fn pending_edits(&self) -> Result<u32, ApiError> {
        Ok(self.core.pending_edits()?)
    }
    pub async fn send_edits(&self) -> Result<SentEdits, ApiError> {
        Ok(self.core.send_edits().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_flatten_with_their_details() {
        let e: ApiError = EncoreError::CertificateUntrusted {
            sha256: "ab".into(),
            reason: "m".into(),
        }
        .into();
        assert_eq!(
            (e.kind, e.sha256.as_deref()),
            (ErrorKind::CertificateUntrusted, Some("ab"))
        );
        let e: ApiError = EncoreError::Server {
            status: 503,
            reason: "m".into(),
            detail: "d".into(),
        }
        .into();
        assert_eq!((e.status, e.detail.as_deref()), (Some(503), Some("d")));
    }

    #[test]
    fn the_handle_works_without_a_profile() {
        let dir = std::env::temp_dir().join(format!("encore-dart-{}", std::process::id()));
        let e = Encore::create(CoreConfig {
            data_dir: dir.to_string_lossy().into_owned(),
            device_id: "d".into(),
            device_name: "n".into(),
            platform: "test".into(),
        })
        .unwrap();
        assert!(e.profiles().unwrap().is_empty());
        assert_eq!(e.servers().unwrap_err().kind, ErrorKind::NoProfile);
        let _ = std::fs::remove_dir_all(dir);
    }
}
