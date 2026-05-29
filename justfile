set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

cobotmap-build:
    cargo build --bin cobotmap --bin cobotmap-verify

cobotmap-run config="cobotmap.toml":
    cargo run --bin cobotmap -- --config "{{config}}"

cobotmap-verify listen="127.0.0.1:8080":
    cargo run --quiet --bin cobotmap-verify -- "http://{{listen}}/maps/api/js"

