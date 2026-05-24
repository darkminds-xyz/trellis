use std::{collections::HashMap, fs};

use actix_web::{HttpResponse, HttpResponseBuilder, http::StatusCode, web};
use handlebars::Handlebars;
use regex::Regex;
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use actix_web::{Responder, get};

use crate::{
    WebTemplates,
    config::AppConfig,
    markdown::{
        MarkdownHtmlRenderer, RushdownMarkdownRenderer, markdown_without_frontmatter,
        tags_from_markdown, title_from_markdown,
    },
    schemas::documents,
    typography::Typography,
};

#[get("/health")]
pub async fn healthcheck_handler() -> impl Responder {
    HttpResponse::Ok().json(json!({ "ping": "pong" }))
}

#[get("/static/content-index.json")]
pub async fn content_index(pool: web::Data<SqlitePool>) -> impl Responder {
    render_content_index(pool).await
}

#[get("/static/context-index.json")]
pub async fn context_index(pool: web::Data<SqlitePool>) -> impl Responder {
    render_content_index(pool).await
}

#[get("/")]
pub async fn index(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
) -> impl Responder {
    let context = match PublicDocuments::load(&pool).await {
        Ok(context) => context,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if context.documents.is_empty() {
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
                None,
                Vec::new(),
            ),
            StatusCode::OK,
        );
    }

    let Some(document) = context.documents.first() else {
        return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let slug = context
        .slug_for(document)
        .unwrap_or_else(|| "index".to_string());

    render_document(
        hb,
        config,
        "index",
        document,
        &slug,
        nav_from_documents(&context, Some(document.id)),
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
    let context = match PublicDocuments::load(&pool).await {
        Ok(context) => context,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let Some(document) = context
        .documents
        .iter()
        .find(|document| document.id == post_id)
    else {
        return HttpResponse::new(StatusCode::NOT_FOUND);
    };
    let slug = context
        .slug_for(document)
        .unwrap_or_else(|| format!("notes/{post_id}"));

    render_document(
        hb,
        config,
        "page",
        document,
        &slug,
        nav_from_documents(&context, Some(document.id)),
    )
}

#[get("/{path:.*}")]
pub async fn virtual_note(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    path: web::Path<String>,
) -> impl Responder {
    let path = path.into_inner();
    if path.is_empty() {
        return HttpResponse::new(StatusCode::NOT_FOUND);
    }

    let context = match PublicDocuments::load(&pool).await {
        Ok(context) => context,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let Some((document, slug)) = context.document_by_slug(&path) else {
        return HttpResponse::new(StatusCode::NOT_FOUND);
    };

    render_document(
        hb,
        config,
        "page",
        document,
        slug,
        nav_from_documents(&context, Some(document.id)),
    )
}

fn render_document(
    hb: WebTemplates,
    config: web::Data<AppConfig>,
    template: &str,
    document: &documents::StoredDocument,
    slug: &str,
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
            slug,
            Some(format!("/admin/edit/{}", document.id)),
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
    edit_url: Option<String>,
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
            "edit_url": edit_url,
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
            "title": "Xplorer",
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

async fn render_content_index(pool: web::Data<SqlitePool>) -> HttpResponse {
    let context = match PublicDocuments::load(&pool).await {
        Ok(context) => context,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut entries = Map::new();
    for document in &context.documents {
        let Some(slug) = context.slug_for(document) else {
            continue;
        };
        entries.insert(slug.clone(), content_index_entry(document, &slug, &context));
    }

    HttpResponse::Ok().json(Value::Object(entries))
}

fn content_index_entry(
    document: &documents::StoredDocument,
    slug: &str,
    context: &PublicDocuments,
) -> Value {
    let renderer = RushdownMarkdownRenderer::new();
    let html = renderer
        .render_html(markdown_without_frontmatter(&document.doc))
        .unwrap_or_default();

    json!({
        "slug": slug,
        "filePath": context.virtual_file_path(document).unwrap_or_else(|| document.name.clone()),
        "title": document_title(document),
        "links": markdown_links(&document.doc, context),
        "tags": tags_from_markdown(&document.doc),
        "content": html_to_text(&html),
    })
}

fn nav_from_documents(context: &PublicDocuments, active_id: Option<i64>) -> Vec<serde_json::Value> {
    nav_children(context, None, active_id)
}

fn nav_children(
    context: &PublicDocuments,
    parent_id: Option<i64>,
    active_id: Option<i64>,
) -> Vec<serde_json::Value> {
    let mut folders = context
        .folders_by_id
        .values()
        .filter(|folder| folder.parent_id == parent_id && !folder.hidden && !folder.draft)
        .collect::<Vec<_>>();
    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let mut items = Vec::new();
    for folder in folders {
        let children = nav_children(context, Some(folder.id), active_id);
        let open = children.iter().any(|child| {
            child
                .get("active")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || child.get("open").and_then(Value::as_bool).unwrap_or(false)
        });
        let path = context.folder_slug(folder.id);
        items.push(json!({
            "is_folder": true,
            "title": folder.title.as_deref().unwrap_or(&folder.name),
            "path": path,
            "folder_path": path,
            "open": open,
            "children": children,
        }));
    }

    let mut documents = context
        .documents
        .iter()
        .filter(|document| document.parent_id == parent_id)
        .collect::<Vec<_>>();
    documents.sort_by(|a, b| {
        document_title(a)
            .to_lowercase()
            .cmp(&document_title(b).to_lowercase())
    });

    for document in documents {
        let slug = context.slug_for(document).unwrap_or_default();
        items.push(json!({
            "is_folder": false,
            "title": document_title(document),
            "path": if slug == "index" { String::new() } else { slug },
            "active": Some(document.id) == active_id,
        }));
    }

    items
}

fn document_title(document: &documents::StoredDocument) -> String {
    title_from_markdown(&document.doc).unwrap_or_else(|| format!("Untitled {}", document.id))
}

#[derive(Debug)]
struct PublicDocuments {
    documents: Vec<documents::StoredDocument>,
    folders_by_id: HashMap<i64, documents::DocumentNode>,
    slugs_by_id: HashMap<i64, String>,
}

impl PublicDocuments {
    async fn load(pool: &SqlitePool) -> sqlx::Result<Self> {
        let public_documents = documents::list_public(pool).await?;
        let nodes = documents::list_nodes(pool).await?;
        let folders_by_id = nodes
            .into_iter()
            .filter(|node| node.kind == "folder")
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();
        let mut context = Self {
            documents: public_documents,
            folders_by_id,
            slugs_by_id: HashMap::new(),
        };
        context.slugs_by_id = context
            .documents
            .iter()
            .map(|document| (document.id, context.virtual_slug(document)))
            .collect();

        Ok(context)
    }

    fn slug_for(&self, document: &documents::StoredDocument) -> Option<String> {
        self.slugs_by_id.get(&document.id).cloned()
    }

    fn document_by_slug(&self, path: &str) -> Option<(&documents::StoredDocument, &str)> {
        let normalized = normalize_slug(path);
        self.documents.iter().find_map(|document| {
            let slug = self.slugs_by_id.get(&document.id)?;
            (slug == &normalized).then_some((document, slug.as_str()))
        })
    }

    fn virtual_file_path(&self, document: &documents::StoredDocument) -> Option<String> {
        let mut parts = self.folder_parts(document.parent_id);
        parts.push(document.name.clone());
        Some(parts.join("/"))
    }

    fn virtual_slug(&self, document: &documents::StoredDocument) -> String {
        let mut parts = self.folder_parts(document.parent_id);
        parts.push(note_slug_part(&document.name));
        normalize_slug(&parts.join("/"))
    }

    fn folder_parts(&self, parent_id: Option<i64>) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = parent_id;
        while let Some(id) = current {
            let Some(folder) = self.folders_by_id.get(&id) else {
                break;
            };
            parts.push(slug_part(&folder.name));
            current = folder.parent_id;
        }
        parts.reverse();
        parts
    }

    fn folder_slug(&self, folder_id: i64) -> String {
        self.folder_parts(Some(folder_id)).join("/")
    }
}

fn markdown_links(markdown: &str, context: &PublicDocuments) -> Vec<String> {
    let re = Regex::new(r"\[[^\]]+\]\(([^)]+)\)").expect("valid markdown link regex");
    re.captures_iter(markdown)
        .filter_map(|capture| capture.get(1))
        .filter_map(|link| normalize_internal_link(link.as_str(), context))
        .collect()
}

fn normalize_internal_link(link: &str, context: &PublicDocuments) -> Option<String> {
    let link = link.split('#').next().unwrap_or(link).trim();
    if link.is_empty()
        || link.starts_with("http://")
        || link.starts_with("https://")
        || link.starts_with("mailto:")
        || link.starts_with("/media/")
        || link.starts_with("/static/")
    {
        return None;
    }

    let slug = normalize_slug(link.trim_start_matches('/'));
    if context
        .slugs_by_id
        .values()
        .any(|candidate| candidate == &slug)
    {
        Some(slug)
    } else {
        None
    }
}

fn html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push('\n');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }

    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn note_slug_part(name: &str) -> String {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    slug_part(stem)
}

fn slug_part(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn normalize_slug(value: &str) -> String {
    let value = value.trim().trim_matches('/');
    if value.is_empty() {
        return "index".to_string();
    }
    let value = value.strip_suffix(".md").unwrap_or(value);
    let value = value.strip_prefix("./").unwrap_or(value);
    value
        .split('/')
        .map(slug_part)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
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
