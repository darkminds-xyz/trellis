use std::collections::BTreeSet;
use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageDecoder, ImageReader, imageops::FilterType};
use ravif::{Encoder, Img, RGBA8};
use regex::Regex;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};

const MAX_IMAGE_EDGE: u32 = 1600;
const AVIF_QUALITY: f32 = 72.0;
const AVIF_SPEED: u8 = 7;

#[derive(Debug, Clone)]
pub struct EncodedImage {
    pub id: String,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredImage {
    pub id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct ImageSummary {
    pub id: String,
    pub mime: String,
    pub alt: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size_bytes: i64,
    pub created_at: String,
}

pub fn encode_upload(bytes: &[u8]) -> anyhow::Result<EncodedImage> {
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);

    let image = resize_for_storage(image);
    let (width, height) = image.dimensions();
    let encoded = encode_avif(&image, width, height)?;
    let mime = "image/avif";

    let id = hex::encode(Sha256::digest(&encoded));

    Ok(EncodedImage {
        id,
        mime,
        width,
        height,
        bytes: encoded,
    })
}

pub async fn insert(
    pool: &SqlitePool,
    image: &EncodedImage,
    alt: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO images (id, mime, alt, width, height, bytes, size_bytes)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&image.id)
    .bind(image.mime)
    .bind(alt)
    .bind(i64::from(image.width))
    .bind(i64::from(image.height))
    .bind(&image.bytes)
    .bind(image.bytes.len() as i64)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<StoredImage>> {
    sqlx::query_as::<_, StoredImage>(
        r#"
        SELECT id, mime, bytes, size_bytes
        FROM images
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<ImageSummary>> {
    sqlx::query_as::<_, ImageSummary>(
        r#"
        SELECT id, mime, alt, width, height, size_bytes, created_at
        FROM images
        ORDER BY datetime(created_at) DESC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn sync_document_images(
    pool: &SqlitePool,
    document_id: i64,
    markdown: &str,
) -> sqlx::Result<()> {
    let image_ids = image_ids_from_markdown(markdown);

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM document_images WHERE document_id = ?1")
        .bind(document_id)
        .execute(&mut *tx)
        .await?;

    for image_id in image_ids {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO document_images (document_id, image_id)
            SELECT ?1, id FROM images WHERE id = ?2
            "#,
        )
        .bind(document_id)
        .bind(image_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

pub fn image_ids_from_markdown(markdown: &str) -> BTreeSet<String> {
    let re = Regex::new(r"/media/images/([a-fA-F0-9]{64})").expect("valid image URL regex");
    re.captures_iter(markdown)
        .filter_map(|captures| captures.get(1))
        .map(|id| id.as_str().to_ascii_lowercase())
        .collect()
}

fn resize_for_storage(image: DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let longest_edge = width.max(height);

    if longest_edge <= MAX_IMAGE_EDGE {
        return image;
    }

    let scale = MAX_IMAGE_EDGE as f32 / longest_edge as f32;
    let resized_width = ((width as f32 * scale).round() as u32).max(1);
    let resized_height = ((height as f32 * scale).round() as u32).max(1);

    image.resize(resized_width, resized_height, FilterType::Lanczos3)
}

fn encode_avif(image: &DynamicImage, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let rgba = image.to_rgba8();
    let pixels = rgba
        .as_raw()
        .chunks_exact(4)
        .map(|pixel| RGBA8::new(pixel[0], pixel[1], pixel[2], pixel[3]))
        .collect::<Vec<_>>();
    let encoded = Encoder::new()
        .with_quality(AVIF_QUALITY)
        .with_speed(AVIF_SPEED)
        .encode_rgba(Img::new(&pixels, width as usize, height as usize))?;

    Ok(encoded.avif_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_media_image_ids_from_markdown() {
        let first = "a".repeat(64);
        let second = "B".repeat(64);
        let markdown = format!(
            "![one](/media/images/{first})\n![dupe](/media/images/{first})\n![two](/media/images/{second})"
        );

        let ids = image_ids_from_markdown(&markdown);

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&first));
        assert!(ids.contains(&second.to_ascii_lowercase()));
    }
}
