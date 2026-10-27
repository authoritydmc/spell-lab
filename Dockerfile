# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Build stage: the lexicon is embedded into the binary with include_str!, so the
# data files are build-time inputs and are not needed in the runtime image.
# -----------------------------------------------------------------------------
FROM rust:1.90-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data

RUN cargo build --release --locked

# -----------------------------------------------------------------------------
# Runtime stage: small Debian base, non-root user, only the binary + CA certs.
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --home-dir /app --shell /usr/sbin/nologin spell

WORKDIR /app
COPY --from=builder /build/target/release/spell-lab /usr/local/bin/spell-lab
RUN mkdir -p /data && chown -R spell:spell /data

USER spell

ENV SPELL_BIND=0.0.0.0:8080 \
    SPELL_DB_PATH=/data/spell.db \
    RUST_LOG=info,tower_http=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=4s --start-period=15s --retries=3 \
    CMD ["curl", "-fsS", "http://127.0.0.1:8080/api/v1/health"]

ENTRYPOINT ["/usr/local/bin/spell-lab"]
