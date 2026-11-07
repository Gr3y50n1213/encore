# syntax=docker/dockerfile:1.7
#
# The companion server image (plan section 5): one small container next to
# Jellyfin, for amd64 and arm64 (a PC, a NAS, a Raspberry Pi 5). Build from the
# repository root, because the server shares crates with the apps:
#
#   docker build -f server/Dockerfile -t encore-server .
#   docker buildx build -f server/Dockerfile --platform linux/amd64,linux/arm64 .
#
# The build runs on the builder's own CPU and cross-compiles for the other one, so
# an arm64 image doesn't need an hour of emulation. Settings are environment
# variables (server/compose.yaml shows them); data lives in the /data volume, and a
# backup is a copy of it or `encore-server backup /data/backup.sqlite`.

ARG RUST_IMAGE=rust:1.99.1-slim-trixie
ARG RUNTIME_IMAGE=debian:trixie-slim

FROM --platform=$BUILDPLATFORM ${RUST_IMAGE} AS build
ARG TARGETARCH
ARG BUILDARCH
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) t=x86_64-unknown-linux-gnu; cc=x86-64-linux-gnu ;; \
      arm64) t=aarch64-unknown-linux-gnu; cc=aarch64-linux-gnu ;; \
      *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    echo "$t" > /rust-target; \
    apt-get update; \
    if [ "$TARGETARCH" = "$BUILDARCH" ]; then \
      apt-get install -y --no-install-recommends gcc libc6-dev; \
    else \
      apt-get install -y --no-install-recommends "gcc-$cc" "libc6-dev-$TARGETARCH-cross"; \
      rustup target add "$t"; \
    fi; \
    rm -rf /var/lib/apt/lists/*
# Used only when cross-compiling (C code in SQLite and ring needs the target's compiler).
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar

WORKDIR /src
COPY core core
COPY server server
COPY design/tokens.json design/tokens.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    set -eux; \
    t="$(cat /rust-target)"; \
    if [ "$TARGETARCH" = "$BUILDARCH" ]; then \
      unset CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER CC_x86_64_unknown_linux_gnu AR_x86_64_unknown_linux_gnu \
            CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER CC_aarch64_unknown_linux_gnu AR_aarch64_unknown_linux_gnu; \
    fi; \
    cargo build --release --locked --manifest-path server/Cargo.toml --target "$t"; \
    cp "server/target/$t/release/encore-server" /encore-server

FROM ${RUNTIME_IMAGE}
# ca-certificates: the operating system's trust store, which the certificate check
# uses for an https Jellyfin.
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin encore; \
    mkdir -p /data; \
    chown encore /data
COPY --from=build /encore-server /usr/local/bin/encore-server
USER encore
ENV ENCORE_SERVER_BIND=0.0.0.0:8097 \
    ENCORE_DATA_DIR=/data
VOLUME /data
EXPOSE 8097
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD ["encore-server", "health"]
ENTRYPOINT ["encore-server"]
