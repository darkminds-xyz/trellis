use std::fs;

use actix_web::{HttpResponse, HttpResponseBuilder, http::StatusCode, web};
use gray_matter::{Matter, ParsedEntity, engine::YAML};
use handlebars::Handlebars;
use serde_json::json;

use actix_web::{Responder, get};

use crate::WebTemplates;

#[get("/health")]
pub async fn healthcheck_handler() -> impl Responder {
    HttpResponse::Ok().json(json!({ "ping": "pong" }))
}

#[get("/")]
pub async fn index(hb: WebTemplates) -> impl Responder {
    let matter = Matter::<YAML>::new();
    let md = String::new();
    let result: ParsedEntity = match matter.parse(&md) {
        Ok(fm) => fm,
        Err(_) => return HttpResponse::new(StatusCode::NOT_FOUND),
    };
    let markdown_json = match serde_json::to_string(&result.content) {
        Ok(markdown) => markdown,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    match fs::read_to_string("js/public/assets/styles.css") {
        Ok(styles) => render(
            hb,
            "index",
            json!({
                "styles": styles,
                "markdown_json": markdown_json,
                "fonts_href": ""
            }),
            HttpResponseBuilder::new(StatusCode::OK),
        ),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn render(
    hb: web::Data<Handlebars<'static>>,
    template: &str,
    data: serde_json::Value,
    mut builder: HttpResponseBuilder,
) -> HttpResponse {
    match hb.render(template, &data) {
        Ok(body) => builder.content_type("text/html; charset=utf-8").body(body),
        Err(err) => HttpResponse::InternalServerError().body(format!("Template error: {}", err)),
    }
}
