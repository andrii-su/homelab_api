# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /app

# Cache deps: copy manifests first, build a stub, then the real sources.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && rm -rf src

COPY src ./src
# Touch so cargo rebuilds with real sources.
RUN touch src/main.rs && cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Docker CLI + compose plugin (static Go binaries) for /api/stacks actions.
COPY --from=docker:27-cli /usr/local/bin/docker /usr/local/bin/docker
COPY --from=docker:27-cli /usr/local/libexec/docker/cli-plugins/docker-compose \
    /usr/local/libexec/docker/cli-plugins/docker-compose

COPY --from=build /app/target/release/homelab_api /usr/local/bin/homelab_api

# Run as non-root. The host docker.sock group GID is injected at runtime via
# compose `group_add` so the CLI can reach the socket without root.
RUN useradd -r -u 10001 appuser
USER appuser
EXPOSE 8087
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8087/health || exit 1

ENTRYPOINT ["homelab_api"]
