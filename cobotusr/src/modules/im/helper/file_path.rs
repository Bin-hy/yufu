use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

pub const DEFAULT_STORAGE_ROOT: &str = "storage";

fn sanitize_segment(segment: &str, fallback: &str) -> String {
    let safe: String = segment
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .collect();

    if safe.is_empty() {
        fallback.to_string()
    } else {
        safe.to_ascii_lowercase()
    }
}

pub fn normalize_folder(folder: &str) -> String {
    sanitize_segment(folder, "misc")
}

pub fn normalize_extension(ext: &str) -> String {
    let trimmed = ext.trim().trim_start_matches('.');
    sanitize_segment(trimmed, "bin")
}

pub fn normalize_storage_root(root_dir: &str) -> String {
    let path = Path::new(root_dir.trim());
    let mut sanitized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            _ => return DEFAULT_STORAGE_ROOT.to_string(),
        }
    }

    let normalized = sanitized
        .to_str()
        .map(|v| v.replace('\\', "/"))
        .unwrap_or_default();

    if normalized.is_empty() {
        DEFAULT_STORAGE_ROOT.to_string()
    } else {
        normalized
    }
}

pub fn build_file_name(ext: &str) -> String {
    let ext = normalize_extension(ext);
    format!("{}.{}", Uuid::new_v4(), ext)
}

pub fn build_storage_dir(root_dir: &str, folder: &str) -> String {
    format!(
        "{}/{}",
        normalize_storage_root(root_dir),
        normalize_folder(folder)
    )
}

pub fn build_storage_path(root_dir: &str, folder: &str, file: &str) -> String {
    let dir = build_storage_dir(root_dir, folder);
    let file = Path::new(file)
        .file_name()
        .and_then(|v| v.to_str())
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or("file.bin");

    format!("{}/{}", dir, file)
}

pub fn build_generated_storage_path(root_dir: &str, folder: &str, ext: &str) -> String {
    let file_name = build_file_name(ext);
    build_storage_path(root_dir, folder, &file_name)
}

pub fn resolve_absolute_storage_path(relative_path: &str) -> PathBuf {
    PathBuf::from(relative_path)
}

pub fn sanitize_relative_storage_path(root_dir: &str, relative_path: &str) -> Option<String> {
    let normalized_root = normalize_storage_root(root_dir);
    let root = Path::new(&normalized_root);
    let path = Path::new(relative_path.trim());
    let root_components = root.components().collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();

    if path_components.len() < root_components.len()
        || !path_components
            .iter()
            .zip(root_components.iter())
            .all(|(path_component, root_component)| path_component == root_component)
    {
        return None;
    }

    let mut sanitized = PathBuf::new();
    for component in path_components {
        match component {
            Component::Normal(part) => sanitized.push(part),
            _ => return None,
        }
    }

    sanitized.to_str().map(|v| v.replace('\\', "/"))
}

pub fn guess_content_type(path: &str) -> mime_guess::Mime {
    mime_guess::from_path(path).first_or_octet_stream()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_folder_name() {
        assert_eq!(normalize_folder("Image Files"), "imagefiles");
        assert_eq!(normalize_folder("video_uploads"), "video_uploads");
        assert_eq!(normalize_folder(""), "misc");
    }

    #[test]
    fn normalizes_extension() {
        assert_eq!(normalize_extension(".JpG"), "jpg");
        assert_eq!(normalize_extension("mp4"), "mp4");
        assert_eq!(normalize_extension("???"), "bin");
    }

    #[test]
    fn normalizes_storage_root() {
        assert_eq!(normalize_storage_root("storage"), "storage");
        assert_eq!(
            normalize_storage_root("./target/test-artifacts/storage"),
            "target/test-artifacts/storage"
        );
        assert_eq!(normalize_storage_root("../storage"), DEFAULT_STORAGE_ROOT);
    }

    #[test]
    fn builds_storage_path_with_safe_file_name() {
        let path = build_storage_path(DEFAULT_STORAGE_ROOT, "image", "../avatar.png");
        assert_eq!(path, "storage/image/avatar.png");
    }

    #[test]
    fn sanitizes_relative_storage_path() {
        assert_eq!(
            sanitize_relative_storage_path(DEFAULT_STORAGE_ROOT, "storage/image/a.png").as_deref(),
            Some("storage/image/a.png")
        );
        assert!(
            sanitize_relative_storage_path(DEFAULT_STORAGE_ROOT, "../storage/image/a.png")
                .is_none()
        );
        assert!(sanitize_relative_storage_path(DEFAULT_STORAGE_ROOT, "storage/../a.png").is_none());
        assert!(sanitize_relative_storage_path(DEFAULT_STORAGE_ROOT, "image/a.png").is_none());
    }

    #[test]
    fn sanitizes_relative_storage_path_for_nested_root() {
        let root = "target/test-artifacts/storage";
        assert_eq!(
            sanitize_relative_storage_path(root, "target/test-artifacts/storage/image/a.png")
                .as_deref(),
            Some("target/test-artifacts/storage/image/a.png")
        );
        assert!(sanitize_relative_storage_path(root, "storage/image/a.png").is_none());
    }
}
