# Checks and codegen for the core track. The root `make check-core` runs `check` here
# and in ../server. Needs cargo and a POSIX shell (Git's sh on Windows).
#
#   make check     fmt, clippy and tests for core/ and tools/, then proof that the
#                  token constants and the bindings in bindings/ are up to date
#   make tokens    regenerate the token constants from design/tokens.json
#   make bindings  regenerate the Swift, Kotlin and Dart bindings
#
# Dart needs flutter_rust_bridge_codegen 2.13.0 and the Dart SDK. When they are not
# installed, the Dart steps say so and are skipped; CI sets REQUIRE_DART=1 so they
# can never be skipped there.

CARGO ?= cargo
FRB ?= flutter_rust_bridge_codegen
FRB_VERSION := 2.13.0
REQUIRE_DART ?= 0

ifeq ($(OS),Windows_NT)
  CORE_LIB := target/debug/encore_core.dll
else ifeq ($(shell uname -s),Darwin)
  CORE_LIB := target/debug/libencore_core.dylib
else
  CORE_LIB := target/debug/libencore_core.so
endif

BINDGEN := $(CARGO) run --locked -q -p uniffi-bindgen -- generate $(CORE_LIB) --no-format
# Rust output goes to the default, encore-dart/src/frb_generated.rs. Dart formatting,
# `dart fix` and dependency upgrades are off, so the output does not depend on which
# Dart SDK is installed; frb still needs a Dart SDK to run.
FRB_ARGS := --rust-input crate::api --rust-root encore-dart --dart-root bindings/dart \
  --dart-output bindings/dart/lib/src/rust \
  --no-web --no-auto-upgrade-dependency --no-dart-format --no-dart-fix --no-add-mod-to-lib
FRB_OUT := encore-dart/src/frb_generated.rs bindings/dart/lib/src/rust
CHECK_DIR := target/bindings-check

.PHONY: check fmt clippy test tools-check tokens tokens-check core-lib bindings uniffi uniffi-check dart dart-check

check: fmt clippy test tools-check tokens-check uniffi-check dart-check
	@echo "core: all checks passed"

fmt:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

test:
	$(CARGO) test --workspace --locked

tools-check:
	cd ../tools && $(CARGO) fmt --all -- --check
	cd ../tools && $(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	cd ../tools && $(CARGO) test --workspace --locked

tokens:
	cd ../tools && $(CARGO) run --locked -q -p encore-tokens -- generate

tokens-check:
	cd ../tools && $(CARGO) run --locked -q -p encore-tokens -- check

core-lib:
	$(CARGO) build --locked -p encore-core

bindings: uniffi dart

uniffi: core-lib
	$(BINDGEN) --language swift --out-dir bindings/swift
	$(BINDGEN) --language kotlin --out-dir bindings/kotlin

# Generate into a scratch folder and compare, so a check never rewrites bindings/.
uniffi-check: core-lib
	rm -rf $(CHECK_DIR) && mkdir -p $(CHECK_DIR)
	$(BINDGEN) --language swift --out-dir $(CHECK_DIR)/swift
	$(BINDGEN) --language kotlin --out-dir $(CHECK_DIR)/kotlin
	diff -r bindings/swift $(CHECK_DIR)/swift
	diff -r bindings/kotlin $(CHECK_DIR)/kotlin
	@echo "Swift and Kotlin bindings are up to date"

dart:
	@command -v $(FRB) >/dev/null 2>&1 || { echo "$(FRB) $(FRB_VERSION) is not installed (cargo install flutter_rust_bridge_codegen --version $(FRB_VERSION) --locked)" >&2; exit 1; }
	@command -v dart >/dev/null 2>&1 || { echo "flutter_rust_bridge needs the Dart SDK on PATH" >&2; exit 1; }
	@$(FRB) --version | grep -qF "$(FRB_VERSION)" || { echo "$(FRB) must be version $(FRB_VERSION), found: $$($(FRB) --version)" >&2; exit 1; }
	$(FRB) generate $(FRB_ARGS)

# frb writes its output in place, so the check regenerates and compares against git.
dart-check:
	@if ! command -v $(FRB) >/dev/null 2>&1 || ! command -v dart >/dev/null 2>&1; then \
	  if [ "$(REQUIRE_DART)" = "1" ]; then echo "Dart check needs $(FRB) $(FRB_VERSION) and the Dart SDK" >&2; exit 1; fi; \
	  echo "SKIPPED: Dart bindings check ($(FRB) or dart not installed here; CI runs it)" >&2; \
	else \
	  { $(FRB) --version | grep -qF "$(FRB_VERSION)" || { echo "$(FRB) must be version $(FRB_VERSION)" >&2; exit 1; }; } && \
	  $(FRB) generate $(FRB_ARGS) && \
	  git diff --exit-code --stat -- $(FRB_OUT) && \
	  test -z "$$(git status --porcelain -- $(FRB_OUT))" && \
	  echo "Dart bindings are up to date"; \
	fi
