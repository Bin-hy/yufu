# Repository guidance

This repository is an authentication center. Maintain only the cobotusr CLI/library, libs/auth and cobotweb/apps/cobotusr. External applications integrate using registered OIDC clients, explicitly granted permissions and token audiences.

Read docs/specs/auth-center-only for the current scope. Legacy service roadmaps are retired. Preserve user local configuration and signing keys; never print secrets. Existing migration IDs must remain recognizable and startup must not silently purge historical business data. Unsupported execution credential profiles fail closed.

Build frontend before embedding webui. Run cargo fmt, workspace Clippy, auth unit and isolated integration tests, frontend test/check/lint/build. Integration tests use disposable PostgreSQL containers and should run serially. Do not operate user servers or write to business databases for tests.

Start: cargo run --bin cobotusr --features webui -- --config conf/cobotusr.local.toml --allow-insecure-loopback-cookies (loopback development only). Keep production Secure cookies and clock readiness enforcement.
