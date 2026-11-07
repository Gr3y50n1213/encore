// Smoke test: Dart calls the Rust core through the generated bindings.
// Run with ENCORE_DART_LIB set to the built encore_dart library
// (core/target/debug/libencore_dart.so on Linux, encore_dart.dll on Windows).
import 'dart:async';
import 'dart:io';

import 'package:encore_core/encore_core.dart';
import 'package:encore_core/encore_tokens.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:test/test.dart';

void main() {
  setUpAll(() async {
    final path = Platform.environment['ENCORE_DART_LIB'];
    if (path == null || path.isEmpty) {
      throw StateError('Set ENCORE_DART_LIB to the built encore_dart library.');
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  test('hello comes from the Rust core', () {
    expect(hello(name: 'CI'), 'Hello, CI, from the Encore core.');
    expect(hello(name: ''), 'Hello from the Encore core.');
  });

  test('version and name come from the Rust core', () {
    expect(coreVersion(), isNotEmpty);
    expect(appName(), 'Encore');
  });

  // The Encore handle: sync and async calls, errors, events and the library flow,
  // one step per test so a failure names its step.
  group('the Encore handle', () {
    late Directory dir;
    late Encore core;
    setUp(() {
      dir = Directory.systemTemp.createTempSync('encore-dart-smoke');
      core = Encore.create(
        config: CoreConfig(
            dataDir: dir.path, deviceId: 'smoke', deviceName: 'CI', platform: 'ci'),
      );
    });
    tearDown(() async {
      core.closeProfile();
      core.dispose();
      // A background send can hold the database for a moment after an edit, and
      // Windows won't delete an open file: try for up to 5 s.
      for (var i = 0; i < 50; i++) {
        try {
          dir.deleteSync(recursive: true);
          return;
        } on FileSystemException {
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
      }
      dir.deleteSync(recursive: true);
    });

    test('synchronous calls and a typed error', () {
      expect(core.profiles(), isEmpty);
      expect(core.currentProfile(), isNull);
      expect(
        () => core.servers(),
        throwsA(isA<ApiError>().having((e) => e.kind, 'kind', ErrorKind.noProfile)),
      );
    }, timeout: const Timeout(Duration(seconds: 20)));

    test('an async call fails with a typed error', () async {
      await expectLater(
        core.checkAddress(address: 'http://music.example.org'),
        throwsA(isA<ApiError>()
            .having((e) => e.kind, 'kind', ErrorKind.insecureAddress)
            .having((e) => e.message, 'message', contains('https'))),
      );
    }, timeout: const Timeout(Duration(seconds: 20)));

    // Over real HTTP against the real Jellyfin recording, which
    // tools/ci/replay_jellyfin.py serves on 127.0.0.1 (see tools/ci/replay.sh).
    // CI runs with --run-skipped, so a missing ENCORE_REPLAY_URL fails there.
    final replay = Platform.environment['ENCORE_REPLAY_URL'];
    test('sign in, sync, browse, play and search', () async {
      if (replay == null) fail('set ENCORE_REPLAY_URL (tools/ci/replay.sh)');
      final server = await core.checkAddress(address: replay);
      expect(server.version, '12.0.0');
      expect(server.plainHttpOnLocalNetwork, isTrue);
      final start = await core.quickConnectStart(baseUrl: server.baseUrl);
      expect(start.code.length, 6);
      final signedIn = await core.quickConnectWait();
      expect(signedIn.profileId, startsWith('jf-'));
      expect(core.currentProfile(), signedIn.profileId);
      final synced = await core.syncLibrary();
      expect(synced.single.songs, 150);
      final album = core
          .albums(sort: AlbumSort.title, offset: 0, limit: 100)
          .firstWhere((a) => a.songCount >= 2);
      final songs = core.album(albumId: album.id).songs;
      final state = core.play(
          songIds: Int64List.fromList([for (final s in songs) s.id]),
          startIndex: 0,
          startedFrom: StartedFrom.album,
          contextId: 'smoke');
      final current = state.current!;
      expect(current.song.id, songs[0].id);
      expect(state.upNext.length, songs.length - 1);
      core.playbackStarted(uid: current.uid, utcOffsetMinutes: 0);
      final next = core.playbackEnded(
          reason: EndReason.finished, listenedMs: current.song.durationMs);
      expect(next?.song.id, songs[1].id);
      final found = core.search(query: songs[0].title);
      expect(found.songs.map((s) => s.title), contains(songs[0].title));

      // Phase 3: lyrics, an album's colours (the same bytes on every platform:
      // the replay serves board 04's cover), a playlist edit and its undo.
      final all = core.songs(sort: SongSort.title, offset: 0, limit: 500);
      SongLyrics? lyrics;
      for (final s in all.where((s) => s.hasLyrics)) {
        lyrics = await core.lyrics(songId: s.id);
        if (lyrics != null) break;
      }
      expect(lyrics, isNotNull);
      expect(lyrics!.syncLevel, LyricsSync.line);
      expect(lyrics.lines, isNotEmpty);
      // Search finds the song by a line of its lyrics (or by its title).
      final words = lyrics.lines
          .firstWhere((l) => l.text.split(' ').length >= 2)
          .text
          .split(' ');
      final byLyrics = core.search(query: '${words[0]} ${words[1]}');
      expect(
          byLyrics.lyrics.any((h) => h.song.id == lyrics!.songId) ||
              byLyrics.songs.any((s) => s.id == lyrics!.songId),
          isTrue);
      // Export: one open JSON file.
      final exportFile = File('${dir.path}/export.json');
      final exported = await core.exportProfile(
          profileId: signedIn.profileId, destPath: exportFile.path);
      expect(exported.bytes, greaterThan(BigInt.zero));
      expect(exportFile.lengthSync(), exported.bytes.toInt());
      final artAlbum = core
          .albums(sort: AlbumSort.title, offset: 0, limit: 100)
          .firstWhere((a) => a.hasArtwork);
      final palette = await core.albumPalette(albumId: artAlbum.id);
      expect(palette!.base, '3D0000');
      expect(palette.stops.length, 4);
      final list = core.playlists(offset: 0, limit: 1).first;
      final entriesBefore = core.playlist(playlistId: list.id).entries;
      final inList = {
        for (final e in entriesBefore)
          if (e.song != null) e.song!.id
      };
      final fresh = all.firstWhere((s) => !inList.contains(s.id));
      final edit = core.addTo(
          target: DropTarget(kind: DropTargetKind.playlist, id: list.id),
          songIds: Int64List.fromList([fresh.id]),
          allowDuplicates: false);
      expect(edit.added, 1);
      core.undo(editId: edit.editId!);
      expect(core.playlist(playlistId: list.id).entries.length,
          entriesBefore.length);
    },
        skip: replay == null ? 'set ENCORE_REPLAY_URL (tools/ci/replay.sh)' : false,
        timeout: const Timeout(Duration(seconds: 60)));

    test('the events stream opens and stopEvents closes it', () async {
      final closed = Completer<void>();
      core.events().listen((_) {}, onDone: closed.complete);
      // Let the stream reach the core before stopping it.
      await Future<void>.delayed(const Duration(milliseconds: 200));
      core.stopEvents();
      await closed.future;
    }, timeout: const Timeout(Duration(seconds: 20)));
  });

  test('token constants are usable', () {
    expect(EncoreTokens.breakpoints.railWidth, 80);
    expect(EncoreTokens.motion.timing.reduceMotionCrossfadeMs, 120);
    expect(EncoreTokens.color.accents.iris.light.primary, '#6146CB');
    expect(EncoreTokens.typography.scale.compact.pageTitle.size, 28);
    expect(EncoreTokens.recap.type.tide, 'Inter, heavy');
  });
}
