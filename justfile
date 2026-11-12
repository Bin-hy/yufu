default:
    @just --list

dev config="conf/yufu.local.toml":
    cargo run --bin yufu --features webui -- --config "{{config}}" --allow-insecure-loopback-cookies

frontend:
    pnpm dev:web

build:
    pnpm build:web
    cargo build --bin yufu --features webui --release

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-features -- --test-threads=1
    pnpm --filter @yufu/web test

docker-build:
    docker build -f deploy/docker/Dockerfile -t yufu:local .
