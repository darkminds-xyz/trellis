use std::fs;

use actix_web::{HttpResponse, HttpResponseBuilder, http::StatusCode, web};
use handlebars::Handlebars;
use serde_json::json;
use sqlx::SqlitePool;

use actix_web::{Responder, get};

use crate::{
    WebTemplates,
    config::AppConfig,
    markdown::{MarkdownHtmlRenderer, RushdownMarkdownRenderer},
    schemas::documents,
    typography::Typography,
};

#[get("/health")]
pub async fn healthcheck_handler() -> impl Responder {
    HttpResponse::Ok().json(json!({ "ping": "pong" }))
}

#[get("/")]
pub async fn index(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
) -> impl Responder {
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if documents.is_empty() {
        return render_with_styles(
            hb,
            "onboarding",
            page_data(
                &config,
                "Welcome to Trellis",
                "index",
                None,
                None,
                None,
                Vec::new(),
            ),
            StatusCode::OK,
        );
    }

    let Some(document) = documents.first() else {
        return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
    };

    render_document(
        hb,
        config,
        "index",
        document,
        documents.first().map(|document| document.id),
        nav_from_documents(&documents, Some(document.id)),
    )
}

#[get("/notes/{post_id}")]
pub async fn note(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    post_id: web::Path<i64>,
) -> impl Responder {
    let post_id = post_id.into_inner();
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let Some(document) = documents.iter().find(|document| document.id == post_id) else {
        return HttpResponse::new(StatusCode::NOT_FOUND);
    };

    render_document(
        hb,
        config,
        "page",
        document,
        documents.first().map(|document| document.id),
        nav_from_documents(&documents, Some(document.id)),
    )
}

fn render_document(
    hb: WebTemplates,
    config: web::Data<AppConfig>,
    template: &str,
    document: &documents::StoredDocument,
    index_id: Option<i64>,
    nav: Vec<serde_json::Value>,
) -> HttpResponse {
    let renderer = RushdownMarkdownRenderer::new();
    let html = match renderer.render_html(&document.doc) {
        Ok(html) => html,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let title = document_title(document);

    render_with_styles(
        hb,
        template,
        page_data(
            &config,
            &title,
            &document_slug(document.id, index_id),
            Some(html),
            document.ctime.as_deref(),
            document.mtime.as_deref(),
            nav,
        ),
        StatusCode::OK,
    )
}

fn render_with_styles(
    hb: WebTemplates,
    template: &str,
    mut data: serde_json::Value,
    status: StatusCode,
) -> HttpResponse {
    match fs::read_to_string("js/public/assets/styles.css") {
        Ok(styles) => {
            data["styles"] = json!(styles);
            render(hb, template, data, HttpResponseBuilder::new(status))
        }
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn page_data(
    config: &AppConfig,
    title: &str,
    slug: &str,
    article_html: Option<String>,
    created: Option<&str>,
    updated: Option<&str>,
    nav: Vec<serde_json::Value>,
) -> serde_json::Value {
    let html = article_html.unwrap_or_default();
    let typography = Typography::from_config(&config.typography);

    json!({
        "styles": "",
        "font_css": typography.font_css,
        "fonts_href": typography.fonts_href,
        "site": {
            "name": config.site.name.clone(),
            "tagline": config.site.tagline.clone(),
        },
        "article": {
            "slug": slug,
            "title": title,
            "created": created,
            "updated": updated,
            "read_time": read_time(&html),
            "tags": [],
            "html": html,
        },
        "footer": {
            "version": env!("CARGO_PKG_VERSION"),
            "year": chrono::Utc::now().format("%Y").to_string(),
            "links": [],
        },
        "nav": nav,
        "explorer": {
            "id": "explorer",
            "title": "Notes",
            "use_saved_state": "true",
            "folder_click_behavior": "link",
            "folder_default_state": "collapsed",
            "data_fns_json": "{}",
        },
        "graph": {
            "title": "Graph",
            "local_cfg_json": "{}",
            "global_cfg_json": "{}",
        },
        "backlinks": {
            "has_backlinks": false,
            "hide_when_empty": true,
            "title": "Backlinks",
            "empty_text": "No backlinks",
            "items": [],
        },
        "scripts": {},
    })
}

fn nav_from_documents(
    documents: &[documents::StoredDocument],
    active_id: Option<i64>,
) -> Vec<serde_json::Value> {
    let index_id = documents.first().map(|document| document.id);

    documents
        .iter()
        .map(|document| {
            json!({
                "title": document_title(document),
                "path": document_path(document.id, index_id),
                "active": Some(document.id) == active_id,
            })
        })
        .collect()
}

fn document_path(id: i64, index_id: Option<i64>) -> String {
    if Some(id) == index_id {
        String::new()
    } else {
        format!("notes/{id}")
    }
}

fn document_slug(id: i64, index_id: Option<i64>) -> String {
    if Some(id) == index_id {
        "index".to_string()
    } else {
        format!("notes/{id}")
    }
}

fn document_title(document: &documents::StoredDocument) -> String {
    markdown_title(&document.doc)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Untitled {}", document.id))
}

fn markdown_title(markdown: &str) -> Option<&str> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
    })
}

fn read_time(content: &str) -> String {
    let words = content.split_whitespace().count();
    let minutes = (words / 200).max(1);
    format!("{minutes} min read")
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
