# ── Stage 1: Build ──────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release && strip target/release/smriti

# ── Stage 2: Runtime ────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/smriti /usr/local/bin/smriti

VOLUME /data
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD smriti --db /data/smriti.db stats || exit 1

ENTRYPOINT ["smriti"]
CMD ["--db", "/data/smriti.db", "serve"]
