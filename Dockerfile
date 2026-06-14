# ── Build stage ──
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY agent-circle-core/ ./agent-circle-core/
COPY agent-circle-plugin/ ./agent-circle-plugin/
COPY plugins/ ./plugins/
COPY src/ ./src/

RUN cargo build --release --bin agent-circle && \
    strip target/release/agent-circle

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/agent-circle /usr/local/bin/agent-circle

ENV RUST_LOG=info
ENV AGENT_CIRCLE_HOME=/data

VOLUME ["/data"]
EXPOSE 9099

ENTRYPOINT ["agent-circle"]
CMD ["daemon", "start"]
