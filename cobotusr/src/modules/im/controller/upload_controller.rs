use std::path::Path;

use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use mime::Mime;
use serde::Deserialize;

use crate::AppState;
use crate::modules::im::{
    helper::file_path,
    model::protocol::{ApiResponse, UploadFileResponse},
    service,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    Image,
    Audio,
    Video,
}

impl MediaKind {
    fn folder(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }

    fn from_mime(mime: &Mime) -> Option<Self> {
        match mime.type_() {
            mime::IMAGE => Some(Self::Image),
            mime::AUDIO => Some(Self::Audio),
            mime::VIDEO => Some(Self::Video),
            _ => None,
        }
    }

    fn is_allowed(self) -> bool {
        matches!(self, Self::Image | Self::Audio | Self::Video)
    }
}

#[derive(Debug, Clone)]
struct DetectedMime {
    mime: Mime,
    ext: String,
    kind: Option<MediaKind>,
}

fn file_name_extension(file_name: Option<&str>) -> Option<String> {
    file_name
        .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
}

fn extension_mime(ext: &str) -> Mime {
    mime_guess::from_ext(ext).first_or_octet_stream().clone()
}

fn detect_magic_mime(data: &[u8]) -> Option<DetectedMime> {
    let kind = infer::get(data)?;
    let mime = kind.mime_type().parse::<Mime>().ok()?;
    let media_kind = MediaKind::from_mime(&mime);

    Some(DetectedMime {
        mime,
        ext: kind.extension().to_string(),
        kind: media_kind,
    })
}

fn mime_matches(extension_mime: &Mime, detected_mime: &Mime) -> bool {
    if extension_mime == detected_mime {
        return true;
    }

    let ext_type = extension_mime.type_();
    let ext_subtype = extension_mime.subtype().as_str();
    let detected_type = detected_mime.type_();
    let detected_subtype = detected_mime.subtype().as_str();

    matches!(
        (ext_type, ext_subtype, detected_type, detected_subtype),
        (mime::AUDIO, "wave", mime::AUDIO, "wav")
            | (mime::AUDIO, "x-wav", mime::AUDIO, "wav")
            | (mime::AUDIO, "ogg", mime::APPLICATION, "ogg")
            | (mime::VIDEO, "ogg", mime::APPLICATION, "ogg")
            | (mime::AUDIO, "mp4", mime::APPLICATION, "mp4")
            | (mime::VIDEO, "mp4", mime::APPLICATION, "mp4")
            | (mime::VIDEO, "quicktime", mime::APPLICATION, "mp4")
    )
}

fn resolve_upload_type(
    file_name: Option<&str>,
    data: &[u8],
) -> Result<(MediaKind, String), String> {
    let magic =
        detect_magic_mime(data).ok_or_else(|| "unsupported or unknown file type".to_string())?;
    let ext = file_name_extension(file_name);

    if let Some(ext) = ext {
        let ext_mime = extension_mime(&ext);
        let ext_kind = MediaKind::from_mime(&ext_mime)
            .ok_or_else(|| format!("unsupported file extension MIME type: {ext_mime}"))?;
        if !ext_kind.is_allowed() {
            return Err(format!("unsupported file extension MIME type: {ext_mime}"));
        }
        if !mime_matches(&ext_mime, &magic.mime) {
            return Err(format!(
                "file extension MIME type {ext_mime} does not match detected content type {}",
                magic.mime
            ));
        }

        let detected_kind = magic.kind.unwrap_or(ext_kind);
        if detected_kind != ext_kind {
            return Err(format!(
                "file extension MIME type {ext_mime} does not match detected content type {}",
                magic.mime
            ));
        }

        return Ok((ext_kind, ext));
    }

    let kind = magic
        .kind
        .ok_or_else(|| "file extension is required for container formats".to_string())?;
    Ok((kind, magic.ext))
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    match multipart.next_field().await {
        Ok(Some(field)) => {
            let file_name = field.file_name().map(str::to_string);
            match field.bytes().await {
                Ok(data) => {
                    let (kind, ext) = match resolve_upload_type(file_name.as_deref(), &data) {
                        Ok(v) => v,
                        Err(e) => {
                            return super::bad_request::<UploadFileResponse>(e);
                        }
                    };

                    match service::save_file(
                        data.to_vec(),
                        &state.config.storage.root_dir,
                        kind.folder(),
                        &ext,
                    )
                    .await
                    {
                        Ok(path) => ApiResponse::ok(UploadFileResponse { path }),
                        Err(e) => super::internal_error::<UploadFileResponse>(format!(
                            "failed to save local file: {e}"
                        )),
                    }
                }
                Err(e) => super::bad_request::<UploadFileResponse>(format!(
                    "failed to read file bytes: {e}"
                )),
            }
        }
        Ok(None) => super::bad_request::<UploadFileResponse>("no file in multipart body"),
        Err(e) => {
            super::bad_request::<UploadFileResponse>(format!("invalid multipart payload: {e}"))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GetFileQuery {
    pub path: String,
}

pub async fn get_file(
    State(state): State<AppState>,
    Query(query): Query<GetFileQuery>,
) -> impl IntoResponse {
    let storage_root = &state.config.storage.root_dir;
    let safe_path = match file_path::sanitize_relative_storage_path(storage_root, &query.path) {
        Some(v) => v,
        None => {
            return (StatusCode::BAD_REQUEST, "invalid file path").into_response();
        }
    };

    match service::read_file(storage_root, &safe_path).await {
        Ok(bytes) => {
            let content_type = file_path::guess_content_type(&safe_path);
            let mut response = Response::new(Body::from(bytes));
            *response.status_mut() = StatusCode::OK;
            if let Ok(header_value) = HeaderValue::from_str(content_type.as_ref()) {
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, header_value);
            }
            response
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (StatusCode::NOT_FOUND, "file not found").into_response()
        }
        Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
            (StatusCode::BAD_REQUEST, "invalid file path").into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "failed to read file").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_image_with_matching_extension() {
        let data = [0xff, 0xd8, 0xff, 0xdb];
        let (kind, ext) = resolve_upload_type(Some("avatar.jpg"), &data).unwrap();
        assert_eq!(kind, MediaKind::Image);
        assert_eq!(ext, "jpg");
    }

    #[test]
    fn rejects_extension_magic_mismatch() {
        let data = [0xff, 0xd8, 0xff, 0xdb];
        let err = resolve_upload_type(Some("voice.mp3"), &data).unwrap_err();
        assert!(err.contains("does not match"));
    }

    #[test]
    fn uses_magic_extension_when_file_name_has_no_extension() {
        let data = b"\x89PNG\r\n\x1a\nrest";
        let (kind, ext) = resolve_upload_type(Some("avatar"), data).unwrap();
        assert_eq!(kind, MediaKind::Image);
        assert_eq!(ext, "png");
    }

    #[test]
    fn uses_extension_kind_for_mp4_container() {
        let data = b"\0\0\0\x18ftypmp42";
        let (kind, ext) = resolve_upload_type(Some("clip.mp4"), data).unwrap();
        assert_eq!(kind, MediaKind::Video);
        assert_eq!(ext, "mp4");
    }
}
