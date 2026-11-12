# syntax=docker/dockerfile:1.7
#
# yufu container image: the Vue console is built first, then embedded into the
# Rust identity service, so the runtime layer only carries one binary.
#
#   docker build -f deploy/docker/Dockerfile -t yufu:local .
#
# The project does not publish a prebuilt image; build it from this checkout.
# Compose usage and the deployment contract live in deploy/docker/README.md.

ARG NODE_IMAGE=node:24-bookworm-slim
ARG RUST_IMAGE=rust:1-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

# --- 1. Console bundle ------------------------------------------------------
# Output goes to <repo>/assets/web, which is what rust-embed reads when the
# service is compiled with --features webui.
FROM ${NODE_IMAGE} AS webui
ARG PNPM_VERSION=12.10.1
WORKDIR /src
# The workspace manifests are required so pnpm can resolve the locked project
# graph; only the console dependency tree is installed.
COPY . .
RUN npm install --global "pnpm@${PNPM_VERSION}" \
    && pnpm install --frozen-lockfile --filter @yufu/web \
    && pnpm build:web

# --- 2. Identity service ---------------------------------------------------
FROM ${RUST_IMAGE} AS build
WORKDIR /src
COPY . .
COPY --from=webui /src/assets/web ./assets/web
# Cache mounts keep the registry and intermediates between builds; the binary
# itself is copied to /out so it survives into the runtime stage.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo build --locked --release --bin yufu --features webui \
    && mkdir -p /out \
    && cp /src/target/release/yufu /out/yufu

# --- 3. Runtime ------------------------------------------------------------
FROM ${RUNTIME_IMAGE} AS runtime

# ca-certificates is required for outbound HTTPS (back-channel logout and
# discovery); curl backs the readiness healthcheck.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --home-dir /var/lib/yufu \
        --shell /usr/sbin/nologin yufu \
    && install --directory --owner yufu --group yufu --mode 0750 \
        /etc/yufu /var/lib/yufu

COPY --from=build /out/yufu /usr/local/bin/yufu
COPY --link deploy/docker/yufu.toml.example /usr/share/yufu/yufu.toml.example

USER 10001:10001
WORKDIR /var/lib/yufu
EXPOSE 4000
# The RSA signing key and any future state live here; mount it on persistent
# storage so a replaced container keeps the same issuer keys.
VOLUME ["/var/lib/yufu"]

# 503 means the security clock is not trusted, so authentication is unavailable.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl --fail --silent --show-error http://127.0.0.1:4000/api/v1/health/ready || exit 1

# The service handles SIGTERM itself and drains in-flight requests.
ENTRYPOINT ["/usr/local/bin/yufu"]
CMD ["--config", "/etc/yufu/yufu.toml"]
