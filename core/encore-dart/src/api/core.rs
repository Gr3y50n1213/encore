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
    AlbumDetail, AlbumSort, AlbumSummary, ArtistDetail, ArtistSummary, ArtworkOf, ArtworkRequest,
    ChosenBy, CoreConfig, CoreEvent, CredentialKey, EncoreError, EndReason, ErrorKind, EventKind,
    GenreSummary, HttpHeader, ListeningMode, Network, PlaylistDetail, PlaylistEntry,
    PlaylistSummary, PreviousResult, ProfileSummary, QueueEntry, QueueState, QuickConnectStart,
    RepeatMode, ResultKind, SearchResults, ServerCheck, ServerRow, SignedIn, Song, SongSort,
    SourceToken, StartedFrom, StreamInfo, StreamQuality, SyncResult, TopResult, TranscodeCodec,
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
