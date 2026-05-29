use tokio::fs;

use crate::modules::im::helper::file_path;

pub async fn save_file(
    data: Vec<u8>,
    storage_root: &str,
    folder: &str,
    ext: &str,
) -> Result<String, std::io::Error> {
    let dir = file_path::build_storage_dir(storage_root, folder);
    fs::create_dir_all(&dir).await?;

    let path = file_path::build_generated_storage_path(storage_root, folder, ext);
    fs::write(&path, data).await?;

    Ok(path)
}

pub async fn read_file(storage_root: &str, path: &str) -> Result<Vec<u8>, std::io::Error> {
    let safe_path = file_path::sanitize_relative_storage_path(storage_root, path)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?;

    fs::read(file_path::resolve_absolute_storage_path(&safe_path)).await
}
