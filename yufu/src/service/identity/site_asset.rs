//! Validate actual image content before creating a durable private asset.
use crate::domain::identity::{error::IdentityError, site_branding::*};
use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

fn raster(bytes: &[u8], format: ImageFormat) -> Result<(u32, u32), IdentityError> {
    let invalid = |_| IdentityError::InvalidImageContent;
    if format == ImageFormat::Png
        && image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(invalid)?
            .is_apng()
            .map_err(invalid)?
    {
        return Err(IdentityError::AnimatedImageNotAllowed);
    }
    if format == ImageFormat::WebP
        && image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .map_err(invalid)?
            .has_animation()
    {
        return Err(IdentityError::AnimatedImageNotAllowed);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|e| {
        if matches!(e, image::ImageError::Limits(_)) {
            IdentityError::ImageLimitExceeded
        } else {
            IdentityError::InvalidImageContent
        }
    })?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(IdentityError::ImageLimitExceeded);
    }
    image::DynamicImage::from_decoder(decoder).map_err(invalid)?;
    Ok((w, h))
}
fn ico(bytes: &[u8]) -> Result<(u32, u32), IdentityError> {
    if bytes.len() < 6 || bytes[..4] != [0, 0, 1, 0] {
        return Err(IdentityError::InvalidImageContent);
    }
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    if count == 0 || 6 + count * 16 > bytes.len() {
        return Err(IdentityError::InvalidImageContent);
    }
    let mut max = (0, 0);
    let mut seen = std::collections::HashSet::new();
    for entry in bytes[6..6 + count * 16].chunks_exact(16) {
        let size = u32::from_le_bytes(
            entry[8..12]
                .try_into()
                .map_err(|_| IdentityError::InvalidImageContent)?,
        ) as usize;
        let offset = u32::from_le_bytes(
            entry[12..16]
                .try_into()
                .map_err(|_| IdentityError::InvalidImageContent)?,
        ) as usize;
        if offset < 6 + count * 16 || size == 0 {
            return Err(IdentityError::InvalidImageContent);
        }
        let payload = bytes
            .get(
                offset
                    ..offset
                        .checked_add(size)
                        .ok_or(IdentityError::InvalidImageContent)?,
            )
            .ok_or(IdentityError::InvalidImageContent)?;
        if !seen.insert((offset, size, entry[0], entry[1])) {
            continue;
        }
        // Decode every representation, including APNG detection in embedded PNGs.
        if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
            raster(payload, ImageFormat::Png)?;
        }
        let mut one = vec![0, 0, 1, 0, 1, 0];
        one.extend_from_slice(entry);
        one[18..22].copy_from_slice(&22_u32.to_le_bytes());
        one.extend_from_slice(payload);
        let dimensions = raster(&one, ImageFormat::Ico)?;
        max.0 = max.0.max(dimensions.0);
        max.1 = max.1.max(dimensions.1);
    }
    Ok(max)
}
fn validate(
    bytes: Vec<u8>,
    kind: SiteAssetKind,
) -> Result<(SiteAssetMetadata, Vec<u8>), IdentityError> {
    if bytes.is_empty() || bytes.len() > kind.max_bytes() {
        return Err(IdentityError::InvalidImageContent);
    }
    let format = image::guess_format(&bytes).map_err(|_| IdentityError::InvalidImageFormat)?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::WebP => "image/webp",
        ImageFormat::Ico if kind == SiteAssetKind::Favicon => "image/x-icon",
        _ => return Err(IdentityError::InvalidImageFormat),
    };
    let (width, height) = if format == ImageFormat::Ico {
        ico(&bytes)?
    } else {
        raster(&bytes, format)?
    };
    let id = Uuid::new_v4();
    let metadata = SiteAssetMetadata {
        id,
        kind,
        mime: mime.into(),
        width,
        height,
        byte_size: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
        preview_url: format!("/api/v1/admin/site-assets/{id}"),
    };
    Ok((metadata, bytes))
}
pub async fn validate_upload(
    bytes: Vec<u8>,
    kind: SiteAssetKind,
) -> Result<(SiteAssetMetadata, Vec<u8>), IdentityError> {
    static PERMITS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let semaphore = PERMITS.get_or_init(|| Arc::new(Semaphore::new(1))).clone();
    let permit = tokio::time::timeout(Duration::from_secs(15), semaphore.acquire_owned())
        .await
        .map_err(|_| IdentityError::CapacityExceeded)?
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    // Keep the permit in the blocking task even if its request is cancelled.
    tokio::time::timeout(
        Duration::from_secs(30),
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            validate(bytes, kind)
        }),
    )
    .await
    .map_err(|_| IdentityError::DeadlineExceeded)?
    .map_err(|_| IdentityError::DependencyUnavailable)?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn png() -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 3)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    #[test]
    fn site_branding_image_roundtrip_and_damage() {
        let bytes = png();
        let (m, original) = validate(bytes.clone(), SiteAssetKind::Logo).unwrap();
        assert_eq!((m.width, m.height), (2, 3));
        assert_eq!(original, bytes);
        assert_eq!(m.mime, "image/png");
        assert!(validate(bytes[..bytes.len() / 2].to_vec(), SiteAssetKind::Logo).is_err());
        for bytes in [
            b"<svg></svg>".to_vec(),
            b"GIF89a".to_vec(),
            vec![0; 2 * 1024 * 1024 + 1],
        ] {
            assert!(validate(bytes, SiteAssetKind::Logo).is_err());
        }
    }
    #[test]
    fn site_branding_ico_all_entries_and_kind() {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(16, 16)
            .write_to(&mut out, ImageFormat::Ico)
            .unwrap();
        let bytes = out.into_inner();
        assert!(validate(bytes.clone(), SiteAssetKind::Favicon).is_ok());
        assert!(validate(bytes.clone(), SiteAssetKind::Logo).is_err());
        assert!(validate(bytes[..20].to_vec(), SiteAssetKind::Favicon).is_err());
    }
    #[test]
    fn site_branding_dimensions_and_byte_boundaries() {
        for (w, h, valid) in [
            (8192, 1, true),
            (1, 8192, true),
            (8193, 1, false),
            (1, 8193, false),
        ] {
            let mut out = Cursor::new(Vec::new());
            image::DynamicImage::new_rgba8(w, h)
                .write_to(&mut out, ImageFormat::Png)
                .unwrap();
            assert_eq!(
                validate(out.into_inner(), SiteAssetKind::Cover).is_ok(),
                valid
            );
        }
        for kind in [
            SiteAssetKind::Logo,
            SiteAssetKind::Favicon,
            SiteAssetKind::Cover,
        ] {
            let mut bytes = png();
            bytes.resize(kind.max_bytes(), 0);
            assert!(validate(bytes.clone(), kind).is_ok());
            bytes.push(0);
            assert!(validate(bytes, kind).is_err());
        }
    }
    #[test]
    fn site_branding_formats_and_animation_rejection() {
        for format in [ImageFormat::Jpeg, ImageFormat::WebP] {
            let mut out = Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(2, 3)
                .write_to(&mut out, format)
                .unwrap();
            assert!(validate(out.into_inner(), SiteAssetKind::Logo).is_ok());
        }
        let apng = include_bytes!("../../../../tests/fixtures/site-branding/animated.png");
        assert!(
            image::codecs::png::PngDecoder::new(Cursor::new(apng))
                .unwrap()
                .is_apng()
                .unwrap()
        );
        assert!(validate(apng.to_vec(), SiteAssetKind::Cover).is_err());
        let mut static_webp = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 3)
            .write_to(&mut static_webp, ImageFormat::WebP)
            .unwrap();
        let payload = static_webp.into_inner();
        let mut chunks = Vec::new();
        let mut chunk = |name: &[u8], data: &[u8]| {
            chunks.extend_from_slice(name);
            chunks.extend_from_slice(&(data.len() as u32).to_le_bytes());
            chunks.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                chunks.push(0);
            }
        };
        chunk(b"VP8X", &[2, 0, 0, 0, 1, 0, 0, 2, 0, 0]);
        chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]);
        let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0, 100, 0, 0, 0];
        frame.extend_from_slice(&payload[12..]);
        chunk(b"ANMF", &frame);
        let mut animated = b"RIFF".to_vec();
        animated.extend_from_slice(&((chunks.len() + 4) as u32).to_le_bytes());
        animated.extend_from_slice(b"WEBP");
        animated.extend_from_slice(&chunks);
        assert!(
            image::codecs::webp::WebPDecoder::new(Cursor::new(&animated))
                .unwrap()
                .has_animation()
        );
        assert!(validate(animated, SiteAssetKind::Cover).is_err());
    }
    #[tokio::test]
    async fn site_branding_decoder_failure_releases_permit() {
        assert!(
            validate_upload(vec![1, 2, 3], SiteAssetKind::Logo)
                .await
                .is_err()
        );
        assert!(validate_upload(png(), SiteAssetKind::Logo).await.is_ok());
    }
}
