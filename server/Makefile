# Checks for the companion server. The root `make check-core` runs `check` here.
CARGO ?= cargo

.PHONY: check fmt clippy test run

check: fmt clippy test
	@echo "server: all checks passed"

# Without --all, so it formats only the server (core/ checks its own files).
fmt:
	$(CARGO) fmt -- --check

clippy:
	$(CARGO) clippy --all-targets --locked -- -D warnings

test:
	$(CARGO) test --locked

run:
	$(CARGO) run --locked
