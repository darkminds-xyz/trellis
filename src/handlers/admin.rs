use std::fs;

use actix_multipart::Multipart;
use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder, delete, get,
    http::{StatusCode, header},
    patch, post, web,
};
use futures_util::StreamExt;
use handlebars::Handlebars;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;

use crate::{
    WebTemplates,
    auth::{AdminLoginLimiter, AdminSessions},
    config::AppConfig,
    markdown::title_from_markdown,
    schemas::{accounts, documents, images},
    typography::Typography,
};

#[derive(Debug, Deserialize)]
pub struct AdminQuery {
    error: Option<String>,
    saved: Option<String>,
    uploaded: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DraftPayload {
    doc: String,
    draft: Option<bool>,
    name: Option<String>,
    #[serde(default)]
    parent_id: Option<Option<i64>>,
    title: Option<String>,
    change_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderPayload {
    parent_id: Option<i64>,
    name: String,
    hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFolderPayload {
    name: Option<String>,
    #[serde(default)]
    parent_id: Option<Option<i64>>,
    hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotePayload {
    parent_id: Option<i64>,
    name: String,
    markdown: String,
    title: Option<String>,
    draft: Option<bool>,
    change_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotePayload {
    name: Option<String>,
    #[serde(default)]
    parent_id: Option<Option<i64>>,
    markdown: Option<String>,
    title: Option<String>,
    draft: Option<bool>,
    change_summary: Option<String>,
}

#[get("/admin")]
pub async fn admin() -> impl Responder {
    redirect("/admin/list")
}

#[get("/admin/list")]
pub async fn admin_list(
    req: HttpRequest,
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
) -> impl Responder {
    if sessions.is_authenticated(&req).await {
        return render_admin_list(hb, pool, config, query.into_inner()).await;
    }

    let has_admin = match accounts::has_admin(&pool).await {
        Ok(has_admin) => has_admin,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_admin_login(hb, config, query.into_inner(), has_admin, "/admin/list")
}

#[get("/admin/edit/{post_id}")]
pub async fn edit_document(
    req: HttpRequest,
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
    post_id: web::Path<i64>,
) -> impl Responder {
    let post_id = post_id.into_inner();
    let next = format!("/admin/edit/{post_id}");

    if !sessions.is_authenticated(&req).await {
        let has_admin = match accounts::has_admin(&pool).await {
            Ok(has_admin) => has_admin,
            Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        };

        return render_admin_login(hb, config, query.into_inner(), has_admin, &next);
    }

    render_admin_edit(hb, pool, config, query.into_inner(), post_id).await
}

#[get("/admin/import")]
pub async fn import_vault(
    req: HttpRequest,
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
) -> impl Responder {
    if sessions.is_authenticated(&req).await {
        return render_admin_import(hb, pool, config, query.into_inner()).await;
    }

    let has_admin = match accounts::has_admin(&pool).await {
        Ok(has_admin) => has_admin,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_admin_login(hb, config, query.into_inner(), has_admin, "/admin/import")
}

#[post("/admin/login")]
pub async fn login(
    req: HttpRequest,
    form: web::Form<LoginForm>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
    login_limiter: web::Data<AdminLoginLimiter>,
) -> impl Responder {
    let login_key = admin_login_limit_key(&req, &form.username);

    if !login_limiter.is_allowed(&login_key) {
        return HttpResponse::new(StatusCode::TOO_MANY_REQUESTS);
    }

    let Some(_verification_slot) = login_limiter.try_acquire_verification() else {
        return HttpResponse::new(StatusCode::TOO_MANY_REQUESTS);
    };

    let is_authenticated = match accounts::authenticate(&pool, &form.username, &form.password).await
    {
        Ok(is_authenticated) => is_authenticated,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if !is_authenticated {
        login_limiter.record_failure(&login_key);
        let next = sanitize_admin_next(form.next.as_deref());
        return redirect(&admin_url(&next, Some("invalid")));
    }

    login_limiter.record_success(&login_key);
    let next = sanitize_admin_next(form.next.as_deref());
    let session_cookie = match sessions.create_session_cookie().await {
        Ok(cookie) => cookie,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, next))
        .cookie(session_cookie)
        .finish()
}

#[post("/admin/logout")]
pub async fn logout(req: HttpRequest, sessions: web::Data<AdminSessions>) -> impl Responder {
    let clear_cookie = sessions.clear_session_cookie(&req).await;

    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, "/admin"))
        .cookie(clear_cookie)
        .finish()
}

#[post("/admin/edit/{post_id}")]
pub async fn save_document(
    req: HttpRequest,
    post_id: web::Path<i64>,
    payload: web::Json<DraftPayload>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    let post_id = post_id.into_inner();
    let edit_path = format!("/admin/edit/{post_id}");

    if !sessions.is_authenticated(&req).await {
        return HttpResponse::Unauthorized().json(json!({
            "error": "unauthorized",
            "login_url": edit_path,
        }));
    }

    if payload.doc.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "error": "empty",
            "message": "Document cannot be empty.",
        }));
    }

    let doc_id = if post_id > 0 { Some(post_id) } else { None };
    match save_editor_document(&pool, doc_id, &payload).await {
        Ok(Some(doc_id)) => {
            if images::sync_document_images(&pool, doc_id, &payload.doc)
                .await
                .is_err()
            {
                return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
            }

            HttpResponse::Ok().json(json!({
                "id": doc_id,
                "edit_url": format!("/admin/edit/{doc_id}?saved=1"),
                "saved": true,
            }))
        }
        Ok(None) => HttpResponse::new(StatusCode::NOT_FOUND),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[post("/admin/folders")]
pub async fn create_folder(
    req: HttpRequest,
    payload: web::Json<CreateFolderPayload>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    match documents::create_folder(
        &pool,
        payload.parent_id,
        &payload.name,
        payload.hidden.unwrap_or(false),
    )
    .await
    {
        Ok(id) => match documents::get_node(&pool, id).await {
            Ok(Some(node)) => HttpResponse::Created().json(document_node_json(&node)),
            Ok(None) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Err(err) => document_error_response(err),
    }
}

#[get("/admin/documents")]
pub async fn list_documents(
    req: HttpRequest,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    match documents::list_nodes(&pool).await {
        Ok(nodes) => HttpResponse::Ok().json(json!({
            "documents": nodes.iter().map(document_node_json).collect::<Vec<_>>(),
        })),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[patch("/admin/folders/{folder_id}")]
pub async fn update_folder(
    req: HttpRequest,
    folder_id: web::Path<i64>,
    payload: web::Json<UpdateFolderPayload>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    let folder_id = folder_id.into_inner();
    match documents::update_folder(
        &pool,
        folder_id,
        payload.name.as_deref(),
        payload.parent_id,
        payload.hidden,
    )
    .await
    {
        Ok(true) => match documents::get_node(&pool, folder_id).await {
            Ok(Some(node)) => HttpResponse::Ok().json(document_node_json(&node)),
            Ok(None) => HttpResponse::new(StatusCode::NOT_FOUND),
            Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Ok(false) => HttpResponse::new(StatusCode::NOT_FOUND),
        Err(err) => document_error_response(err),
    }
}

#[post("/admin/notes")]
pub async fn create_note(
    req: HttpRequest,
    payload: web::Json<CreateNotePayload>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    if payload.markdown.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "error": "empty",
            "message": "Document cannot be empty.",
        }));
    }

    match documents::create_note(
        &pool,
        payload.parent_id,
        &payload.name,
        &payload.markdown,
        payload.draft.unwrap_or(false),
        payload.title.as_deref(),
        payload.change_summary.as_deref(),
    )
    .await
    {
        Ok(id) => {
            if images::sync_document_images(&pool, id, &payload.markdown)
                .await
                .is_err()
            {
                return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
            }

            match documents::get(&pool, id).await {
                Ok(Some(note)) => HttpResponse::Created().json(document_json(&note)),
                Ok(None) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
                Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(err) => document_error_response(err),
    }
}

#[patch("/admin/notes/{note_id}")]
pub async fn update_note(
    req: HttpRequest,
    note_id: web::Path<i64>,
    payload: web::Json<UpdateNotePayload>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    let note_id = note_id.into_inner();
    match documents::update_note(
        &pool,
        note_id,
        payload.name.as_deref(),
        payload.parent_id,
        payload.markdown.as_deref(),
        payload.draft,
        payload.title.as_deref(),
        payload.change_summary.as_deref(),
    )
    .await
    {
        Ok(true) => {
            if let Some(markdown) = payload.markdown.as_deref() {
                if images::sync_document_images(&pool, note_id, markdown)
                    .await
                    .is_err()
                {
                    return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            match documents::get(&pool, note_id).await {
                Ok(Some(note)) => HttpResponse::Ok().json(document_json(&note)),
                Ok(None) => HttpResponse::new(StatusCode::NOT_FOUND),
                Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Ok(false) => HttpResponse::new(StatusCode::NOT_FOUND),
        Err(err) => document_error_response(err),
    }
}

#[get("/admin/notes/{note_id}/versions")]
pub async fn list_note_versions(
    req: HttpRequest,
    note_id: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    match documents::list_versions(&pool, note_id.into_inner()).await {
        Ok(versions) => HttpResponse::Ok().json(json!({
            "versions": versions
                .iter()
                .map(|version| {
                    json!({
                        "id": version.id,
                        "document_id": version.document_id,
                        "version_number": version.version_number,
                        "title": version.title,
                        "markdown": version.markdown,
                        "change_summary": version.change_summary,
                        "ctime": version.ctime,
                    })
                })
                .collect::<Vec<_>>(),
        })),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[delete("/admin/documents/{document_id}")]
pub async fn delete_document(
    req: HttpRequest,
    document_id: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return api_unauthorized();
    }

    match documents::delete(&pool, document_id.into_inner()).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::new(StatusCode::NOT_FOUND),
        Err(err) => document_error_response(err),
    }
}

#[post("/admin/import")]
pub async fn upload_vault(
    req: HttpRequest,
    mut payload: Multipart,
    sessions: web::Data<AdminSessions>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    if !sessions.is_authenticated(&req).await {
        return redirect("/admin/import");
    }

    let mut image_count = 0usize;
    while let Some(item) = payload.next().await {
        let Ok(mut field) = item else {
            return HttpResponse::new(StatusCode::BAD_REQUEST);
        };

        let filename = field
            .content_disposition()
            .and_then(|disposition| disposition.get_filename())
            .map(str::to_string);

        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            let Ok(chunk) = chunk else {
                return HttpResponse::new(StatusCode::BAD_REQUEST);
            };
            bytes.extend_from_slice(&chunk);
            if bytes.len() > 20 * 1024 * 1024 {
                return HttpResponse::new(StatusCode::PAYLOAD_TOO_LARGE);
            }
        }

        if filename.is_none() || bytes.is_empty() {
            continue;
        }

        let Ok(encoded) = images::encode_upload(&bytes) else {
            continue;
        };

        if images::insert(&pool, &encoded, filename.as_deref())
            .await
            .is_err()
        {
            return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
        }

        image_count += 1;
    }

    redirect(&format!("/admin/import?uploaded={image_count}"))
}

fn render_admin_login(
    hb: WebTemplates,
    config: web::Data<AppConfig>,
    query: AdminQuery,
    has_admin: bool,
    next: &str,
) -> HttpResponse {
    render_with_styles(
        hb,
        "admin/login",
        admin_data(
            &config,
            query,
            has_admin,
            AdminView::Login { next },
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        ),
        StatusCode::OK,
    )
}

async fn render_admin_list(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    query: AdminQuery,
) -> HttpResponse {
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let nodes = match documents::list_nodes(&pool).await {
        Ok(nodes) => nodes,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_with_styles(
        hb,
        "admin/index",
        admin_data(
            &config,
            query,
            true,
            AdminView::List,
            documents,
            nodes,
            None,
            Vec::new(),
        ),
        StatusCode::OK,
    )
}

async fn render_admin_edit(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    query: AdminQuery,
    post_id: i64,
) -> HttpResponse {
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let nodes = match documents::list_nodes(&pool).await {
        Ok(nodes) => nodes,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let selected = if post_id > 0 {
        match documents::get(&pool, post_id).await {
            Ok(Some(document)) => Some(document),
            Ok(None) => return HttpResponse::new(StatusCode::NOT_FOUND),
            Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        None
    };

    render_with_styles(
        hb,
        "admin/index",
        admin_data(
            &config,
            query,
            true,
            AdminView::Edit,
            documents,
            nodes,
            selected,
            Vec::new(),
        ),
        StatusCode::OK,
    )
}

async fn render_admin_import(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    config: web::Data<AppConfig>,
    query: AdminQuery,
) -> HttpResponse {
    let images = match images::list(&pool).await {
        Ok(images) => images,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_with_styles(
        hb,
        "admin/index",
        admin_data(
            &config,
            query,
            true,
            AdminView::Import,
            Vec::new(),
            Vec::new(),
            None,
            images,
        ),
        StatusCode::OK,
    )
}

#[derive(Debug, Clone, Copy)]
enum AdminView<'a> {
    Login { next: &'a str },
    List,
    Edit,
    Import,
}

fn admin_data(
    config: &AppConfig,
    query: AdminQuery,
    has_admin: bool,
    view: AdminView<'_>,
    documents: Vec<documents::StoredDocument>,
    nodes: Vec<documents::DocumentNode>,
    selected: Option<documents::StoredDocument>,
    images: Vec<images::ImageSummary>,
) -> serde_json::Value {
    let selected_doc_id = selected.as_ref().map(|document| document.id);
    let selected_doc_name = selected.as_ref().map(|document| document.name.as_str());
    let selected_doc_parent_id = selected.as_ref().and_then(|document| document.parent_id);
    let selected_doc_is_draft = selected
        .as_ref()
        .map(|document| document.draft)
        .unwrap_or(false);
    let index_doc_id = documents.first().map(|document| document.id);
    let selected_doc_label = selected_doc_id.map(|id| document_label(id, index_doc_id));
    let is_first_post_draft = selected_doc_id.is_none() && documents.is_empty();
    let draft = selected
        .as_ref()
        .map(|document| document.doc.as_str())
        .unwrap_or("# First post\n\nStart writing your first Trellis document here.\n");
    let draft_json = serde_json::to_string(draft).unwrap_or_else(|_| "\"\"".to_string());
    let typography = Typography::from_config(&config.typography);
    let posts = documents
        .iter()
        .map(|document| {
            let label = document_label(document.id, index_doc_id);
            json!({
                "id": document.id,
                "label": label,
                "title": title_from_markdown(&document.doc).unwrap_or_else(|| label.clone()),
                "ctime": document.ctime,
                "mtime": document.mtime,
                "selected": Some(document.id) == selected_doc_id,
            })
        })
        .collect::<Vec<_>>();
    let folder_options = folder_options(
        &nodes,
        selected.as_ref().and_then(|document| document.parent_id),
    );
    let document_tree = document_tree(&nodes, selected_doc_id);
    let images = images
        .iter()
        .map(|image| {
            let alt = image.alt.as_deref().unwrap_or("uploaded image");
            json!({
                "id": image.id,
                "short_id": image.id.chars().take(12).collect::<String>(),
                "mime": image.mime,
                "alt": image.alt,
                "width": image.width,
                "height": image.height,
                "size_bytes": image.size_bytes,
                "created_at": image.created_at,
                "markdown": format!("![{alt}](/media/images/{})", image.id),
            })
        })
        .collect::<Vec<_>>();
    let has_images = !images.is_empty();

    json!({
        "styles": "",
        "editor_styles": "",
        "font_css": typography.font_css,
        "fonts_href": typography.fonts_href,
        "site": {
            "name": config.site.name.clone(),
            "tagline": config.site.tagline.clone(),
        },
        "footer": {
            "version": env!("CARGO_PKG_VERSION"),
            "year": chrono::Utc::now().format("%Y").to_string(),
            "links": [],
        },
        "nav": [],
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
        "has_admin": has_admin,
        "is_login": matches!(view, AdminView::Login { .. }),
        "is_list": matches!(view, AdminView::List),
        "is_edit": matches!(view, AdminView::Edit),
        "is_import": matches!(view, AdminView::Import),
        "error": query.error,
        "saved": query.saved,
        "uploaded": query.uploaded,
        "next": match view {
            AdminView::Login { next } => next,
            _ => "/admin/list",
        },
        "posts": posts,
        "has_posts": !posts.is_empty(),
        "document_tree": document_tree,
        "has_document_tree": !nodes.is_empty(),
        "folder_options": folder_options,
        "selected_doc_name": selected_doc_name,
        "selected_doc_parent_id": selected_doc_parent_id,
        "selected_doc_is_draft": selected_doc_is_draft,
        "images": images,
        "has_images": has_images,
        "selected_doc_id": selected_doc_id,
        "selected_doc_label": selected_doc_label,
        "is_first_post_draft": is_first_post_draft,
        "draft": draft,
        "draft_json": draft_json,
    })
}

fn render_with_styles(
    hb: web::Data<Handlebars<'static>>,
    template: &str,
    mut data: serde_json::Value,
    status: StatusCode,
) -> HttpResponse {
    let styles = fs::read_to_string("js/public/assets/styles.css").unwrap_or_default();
    let editor_styles = fs::read_to_string("js/public/assets/editor.css").unwrap_or_default();
    data["styles"] = json!(styles);
    data["editor_styles"] = json!(editor_styles);

    render(hb, template, data, HttpResponseBuilder::new(status))
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

fn folder_options(
    nodes: &[documents::DocumentNode],
    selected_parent_id: Option<i64>,
) -> Vec<serde_json::Value> {
    let mut options = vec![json!({
        "id": serde_json::Value::Null,
        "label": "Root",
        "selected": selected_parent_id.is_none(),
    })];
    append_folder_options(nodes, None, 0, selected_parent_id, &mut options);
    options
}

fn append_folder_options(
    nodes: &[documents::DocumentNode],
    parent_id: Option<i64>,
    depth: usize,
    selected_parent_id: Option<i64>,
    options: &mut Vec<serde_json::Value>,
) {
    let mut children = nodes
        .iter()
        .filter(|node| node.parent_id == parent_id && node.kind == "folder")
        .collect::<Vec<_>>();
    children.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for node in children {
        let prefix = "  ".repeat(depth);
        options.push(json!({
            "id": node.id,
            "label": format!("{prefix}{}", node.name),
            "selected": Some(node.id) == selected_parent_id,
        }));
        append_folder_options(nodes, Some(node.id), depth + 1, selected_parent_id, options);
    }
}

fn document_tree(
    nodes: &[documents::DocumentNode],
    selected_doc_id: Option<i64>,
) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    append_document_tree(nodes, None, 0, selected_doc_id, &mut rows);
    rows
}

fn append_document_tree(
    nodes: &[documents::DocumentNode],
    parent_id: Option<i64>,
    depth: usize,
    selected_doc_id: Option<i64>,
    rows: &mut Vec<serde_json::Value>,
) {
    let mut children = nodes
        .iter()
        .filter(|node| node.parent_id == parent_id)
        .collect::<Vec<_>>();
    children.sort_by(|a, b| {
        let kind_order = a.kind.cmp(&b.kind);
        if kind_order == std::cmp::Ordering::Equal {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            kind_order
        }
    });

    for node in children {
        let is_folder = node.kind == "folder";
        rows.push(json!({
            "id": node.id,
            "parent_id": node.parent_id,
            "name": node.name,
            "title": node.title.as_deref().unwrap_or(&node.name),
            "kind": node.kind,
            "is_folder": is_folder,
            "is_note": node.kind == "note",
            "hidden": node.hidden,
            "draft": node.draft,
            "depth": depth,
            "indent": depth * 18,
            "selected": Some(node.id) == selected_doc_id,
            "ctime": node.ctime,
            "mtime": node.mtime,
        }));

        if is_folder {
            append_document_tree(nodes, Some(node.id), depth + 1, selected_doc_id, rows);
        }
    }
}

async fn save_editor_document(
    pool: &SqlitePool,
    doc_id: Option<i64>,
    payload: &DraftPayload,
) -> sqlx::Result<Option<i64>> {
    if let Some(id) = doc_id {
        let updated = documents::update_note(
            pool,
            id,
            payload.name.as_deref(),
            payload.parent_id,
            Some(&payload.doc),
            payload.draft,
            payload.title.as_deref(),
            payload.change_summary.as_deref(),
        )
        .await
        .map_err(document_error_into_sqlx)?;

        return Ok(updated.then_some(id));
    }

    documents::create_root_note(
        pool,
        &payload.doc,
        payload.draft.unwrap_or(false),
        payload.title.as_deref(),
        payload.change_summary.as_deref(),
    )
    .await
    .map(Some)
    .map_err(document_error_into_sqlx)
}

fn document_json(document: &documents::StoredDocument) -> serde_json::Value {
    json!({
        "id": document.id,
        "parent_id": document.parent_id,
        "name": document.name,
        "kind": document.kind,
        "current_version_id": document.current_version_id,
        "version_number": document.version_number,
        "title": document.title,
        "markdown": document.doc,
        "hidden": document.hidden,
        "draft": document.draft,
        "ctime": document.ctime,
        "mtime": document.mtime,
    })
}

fn document_node_json(document: &documents::DocumentNode) -> serde_json::Value {
    json!({
        "id": document.id,
        "parent_id": document.parent_id,
        "name": document.name,
        "kind": document.kind,
        "current_version_id": document.current_version_id,
        "title": document.title,
        "hidden": document.hidden,
        "draft": document.draft,
        "ctime": document.ctime,
        "mtime": document.mtime,
    })
}

fn document_error_response(err: documents::DocumentError) -> HttpResponse {
    match err {
        documents::DocumentError::Domain(documents::DocumentErrorKind::FolderNotEmpty) => {
            HttpResponse::Conflict().json(json!({
                "error": "folder_not_empty",
                "message": "Folder contains notes and cannot be deleted.",
            }))
        }
        documents::DocumentError::Domain(documents::DocumentErrorKind::InvalidDocumentKind) => {
            HttpResponse::BadRequest().json(json!({
                "error": "invalid_document_kind",
                "message": "Document type does not support this operation.",
            }))
        }
        documents::DocumentError::Domain(documents::DocumentErrorKind::InvalidParent) => {
            HttpResponse::BadRequest().json(json!({
                "error": "invalid_parent",
                "message": "Choose a valid destination folder.",
            }))
        }
        documents::DocumentError::Sqlx(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn document_error_into_sqlx(err: documents::DocumentError) -> sqlx::Error {
    match err {
        documents::DocumentError::Sqlx(err) => err,
        documents::DocumentError::Domain(kind) => sqlx::Error::Protocol(format!("{kind:?}")),
    }
}

fn api_unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().json(json!({
        "error": "unauthorized",
        "login_url": "/admin",
    }))
}

fn redirect(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, location))
        .finish()
}

fn admin_url(path: &str, error: Option<&str>) -> String {
    let Some(error) = error else {
        return path.to_string();
    };

    if path.contains('?') {
        format!("{path}&error={error}")
    } else {
        format!("{path}?error={error}")
    }
}

fn sanitize_admin_next(next: Option<&str>) -> String {
    next.filter(|next| next.starts_with("/admin") && !next.starts_with("//"))
        .unwrap_or("/admin/list")
        .to_string()
}

fn admin_login_limit_key(req: &HttpRequest, username: &str) -> String {
    let remote_addr = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let username = username.trim().to_ascii_lowercase();

    format!("{remote_addr}:{username}")
}

fn document_label(id: i64, index_doc_id: Option<i64>) -> String {
    if Some(id) == index_doc_id {
        "index.md".to_string()
    } else {
        format!("document-{id}.md")
    }
}
