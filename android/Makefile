# The Android track's checks (root Makefile: `make check-android`).
#   make check        lint, unit tests, debug and release builds of the app (the Rust
#                     core built for every ABI in gradle.properties), then a 16 KB page
#                     alignment check of both APKs
#   make device-test  the instrumented tests on an attached device or emulator
# Locally, one ABI is enough: make check ABIS=arm64-v8a
# CI (.github/workflows/android.yml) runs `check` for all four ABIs, then
# `device-test` on an emulator.
.PHONY: check device-test

GRADLE ?= ./gradlew
ABIS ?=
GRADLE_ARGS = --stacktrace $(if $(ABIS),-Pencore.abis=$(ABIS),)
BUILD_TOOLS ?= $(ANDROID_HOME)/build-tools/36.0.0
APKS = app/build/outputs/apk/debug/app-debug.apk app/build/outputs/apk/release/app-release.apk

check:
	$(GRADLE) $(GRADLE_ARGS) -Pencore.signReleaseWithDebugKey=true \
	  lintDebug testDebugUnitTest assembleDebug assembleRelease assembleDebugAndroidTest
	@# zipalign -P 16: every uncompressed .so in the APK starts on a 16 KB page
	@# (developer.android.com/guide/practices/page-sizes, "Check alignment").
	@for apk in $(APKS); do \
	  "$(BUILD_TOOLS)/zipalign" -c -P 16 -v 4 "$$apk" | grep -E '\.so' ; \
	  "$(BUILD_TOOLS)/zipalign" -c -P 16 4 "$$apk" || { echo "$$apk is not 16 KB aligned" >&2; exit 1; }; \
	done
	@echo "check-android: passed"

device-test:
	$(GRADLE) $(GRADLE_ARGS) connectedDebugAndroidTest
