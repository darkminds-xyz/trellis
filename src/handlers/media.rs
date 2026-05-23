use actix_web::{
    HttpRequest, HttpResponse, Responder, get,
    http::{StatusCode, header},
    web,
};
use sqlx::SqlitePool;

use crate::schemas::images;

#[get("/media/images/{image_id}")]
pub async fn image(
    req: HttpRequest,
    pool: web::Data<SqlitePool>,
    image_id: web::Path<String>,
) -> impl Responder {
    let image_id = image_id.into_inner();
    if !is_valid_image_id(&image_id) {
        return HttpResponse::new(StatusCode::NOT_FOUND);
    }

    let image = match images::get(&pool, &image_id).await {
        Ok(Some(image)) => image,
        Ok(None) => return HttpResponse::new(StatusCode::NOT_FOUND),
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let etag = format!("\"{}\"", image.id);
    if req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == etag)
    {
        return HttpResponse::NotModified()
            .insert_header((header::ETAG, etag))
            .finish();
    }

    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, image.mime))
        .insert_header((header::CONTENT_LENGTH, image.size_bytes.to_string()))
        .insert_header((header::CACHE_CONTROL, "public, max-age=31536000, immutable"))
        .insert_header((header::ETAG, etag))
        .body(image.bytes)
}

fn is_valid_image_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}
