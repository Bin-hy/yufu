use serde::de::DeserializeOwned;
use tracing::debug;

pub async fn shutdown_signal() {
    let str = signal::wait_for_stop_signal().await;
    debug!("Received signal: {}", str);
}

/// Load a TOML config file, falling back to defaults if the file doesn't exist.
#[allow(dead_code)]
pub fn load<T: DeserializeOwned + Default>(name: String, path: Option<String>) -> T {
    if let Some(path) = path {
        let p = std::path::Path::new(&path);
        if p.try_exists().unwrap_or(false) {
            return toml::from_str(
                std::fs::read_to_string(p)
                    .unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
                    .as_str(),
            )
            .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
        }
    }
    eprintln!("=== No {} config file, use default config ===", name);
    T::default()
}
