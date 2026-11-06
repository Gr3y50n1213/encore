/// The Encore core for Dart (Windows and Linux apps).
///
/// Call `await RustLib.init(externalLibrary: ...)` once at start-up, then call the
/// functions. `docs/core/CORE-API.md` says what each one does.
library;

export 'src/rust/frb_generated.dart' show RustLib;
export 'src/rust/api/hello.dart';
