# Repository guidance

This repository is an authentication center. Maintain only the yufu CLI/library, libs/auth and web. External applications integrate using registered OIDC clients, explicitly granted permissions and token audiences.

Read docs/specs/auth-center-only for the current scope. Legacy service roadmaps are retired. Preserve user local configuration and signing keys; never print secrets. Existing migration IDs must remain recognizable and startup must not silently purge historical business data. Unsupported execution credential profiles fail closed.

Build frontend before embedding webui. Run cargo fmt, workspace Clippy, auth unit and isolated integration tests, frontend test/check/lint/build. Integration tests use disposable PostgreSQL containers and should run serially. Do not operate user servers or write to business databases for tests.

Start: cargo run --bin yufu --features webui -- --config conf/yufu.local.toml --allow-insecure-loopback-cookies (loopback development only). Keep production Secure cookies and clock readiness enforcement.
