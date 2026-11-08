default:
    @just --list

dev config="conf/cobotusr.local.toml":
    cargo run --bin cobotusr --features webui -- --config "{{config}}" --allow-insecure-loopback-cookies

frontend:
    pnpm dev:cobotusr

build:
    pnpm build:cobotusr
    cargo build --bin cobotusr --features webui --release

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-features -- --test-threads=1
    pnpm --filter @cobotweb/cobotusr test
