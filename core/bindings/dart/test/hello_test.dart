// Smoke test: Dart calls the Rust core through the generated bindings.
// Run with ENCORE_DART_LIB set to the built encore_dart library
// (core/target/debug/libencore_dart.so on Linux, encore_dart.dll on Windows).
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

  test('token constants are usable', () {
    expect(EncoreTokens.breakpoints.railWidth, 80);
    expect(EncoreTokens.motion.timing.reduceMotionCrossfadeMs, 120);
    expect(EncoreTokens.color.accents.iris.light.primary, '#6146CB');
    expect(EncoreTokens.typography.scale.compact.pageTitle.size, 28);
    expect(EncoreTokens.recap.type.tide, 'Inter, heavy');
  });
}
