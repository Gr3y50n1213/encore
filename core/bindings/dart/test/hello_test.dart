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

  // No network: signing in, syncing and playing are tested in Rust on a real
  // Jellyfin recording. These prove the Encore handle, async calls, errors and
  // events, one step per test so a failure names its step.
  group('the Encore handle, without a server', () {
    late Directory dir;
    late Encore core;
    setUp(() {
      dir = Directory.systemTemp.createTempSync('encore-dart-smoke');
      core = Encore.create(
        config: CoreConfig(
            dataDir: dir.path, deviceId: 'smoke', deviceName: 'CI', platform: 'ci'),
      );
    });
    tearDown(() {
      core.dispose();
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
