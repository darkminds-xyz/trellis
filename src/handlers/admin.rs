use std::fs;

use actix_multipart::Multipart;
use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder, get,
    http::{StatusCode, header},
    post, web,
};
use futures_util::StreamExt;
use handlebars::Handlebars;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;

use crate::{
    WebTemplates,
    auth::AdminSessions,
    schemas::{accounts, documents},
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
pub struct DraftForm {
    doc: String,
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
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
) -> impl Responder {
    if sessions.is_authenticated(&req) {
        return render_admin_list(hb, pool, query.into_inner()).await;
    }

    let has_admin = match accounts::has_admin(&pool).await {
        Ok(has_admin) => has_admin,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_admin_login(hb, query.into_inner(), has_admin, "/admin/list")
}

#[get("/admin/edit/{post_id}")]
pub async fn edit_document(
    req: HttpRequest,
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
    post_id: web::Path<i64>,
) -> impl Responder {
    let post_id = post_id.into_inner();
    let next = format!("/admin/edit/{post_id}");

    if !sessions.is_authenticated(&req) {
        let has_admin = match accounts::has_admin(&pool).await {
            Ok(has_admin) => has_admin,
            Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        };

        return render_admin_login(hb, query.into_inner(), has_admin, &next);
    }

    render_admin_edit(hb, pool, query.into_inner(), post_id).await
}

#[get("/admin/import")]
pub async fn import_vault(
    req: HttpRequest,
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
    query: web::Query<AdminQuery>,
) -> impl Responder {
    if sessions.is_authenticated(&req) {
        return render_admin_import(hb, query.into_inner());
    }

    let has_admin = match accounts::has_admin(&pool).await {
        Ok(has_admin) => has_admin,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_admin_login(hb, query.into_inner(), has_admin, "/admin/import")
}

#[post("/admin/login")]
pub async fn login(
    form: web::Form<LoginForm>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    let is_authenticated = match accounts::authenticate(&pool, &form.username, &form.password).await
    {
        Ok(is_authenticated) => is_authenticated,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if !is_authenticated {
        let next = sanitize_admin_next(form.next.as_deref());
        return redirect(&admin_url(&next, Some("invalid")));
    }

    let next = sanitize_admin_next(form.next.as_deref());
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, next))
        .cookie(sessions.create_session_cookie())
        .finish()
}

#[post("/admin/logout")]
pub async fn logout(req: HttpRequest, sessions: web::Data<AdminSessions>) -> impl Responder {
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, "/admin"))
        .cookie(sessions.clear_session_cookie(&req))
        .finish()
}

#[post("/admin/edit/{post_id}")]
pub async fn save_document(
    req: HttpRequest,
    post_id: web::Path<i64>,
    form: web::Form<DraftForm>,
    pool: web::Data<SqlitePool>,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    let post_id = post_id.into_inner();
    let edit_path = format!("/admin/edit/{post_id}");

    if !sessions.is_authenticated(&req) {
        return redirect(&edit_path);
    }

    if form.doc.trim().is_empty() {
        return redirect(&admin_url(&edit_path, Some("empty")));
    }

    let doc_id = if post_id > 0 { Some(post_id) } else { None };
    match documents::save(&pool, doc_id, &form.doc).await {
        Ok(doc_id) => redirect(&format!("/admin/edit/{doc_id}?saved=1")),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[post("/admin/import")]
pub async fn upload_vault(
    req: HttpRequest,
    mut payload: Multipart,
    sessions: web::Data<AdminSessions>,
) -> impl Responder {
    if !sessions.is_authenticated(&req) {
        return redirect("/admin/import");
    }

    let mut file_count = 0usize;
    while let Some(item) = payload.next().await {
        let Ok(mut field) = item else {
            return HttpResponse::new(StatusCode::BAD_REQUEST);
        };

        let has_filename = field
            .content_disposition()
            .and_then(|disposition| disposition.get_filename())
            .is_some();
        if has_filename {
            file_count += 1;
        }

        while let Some(chunk) = field.next().await {
            if chunk.is_err() {
                return HttpResponse::new(StatusCode::BAD_REQUEST);
            }
        }
    }

    redirect(&format!("/admin/import?uploaded={file_count}"))
}

fn render_admin_login(
    hb: WebTemplates,
    query: AdminQuery,
    has_admin: bool,
    next: &str,
) -> HttpResponse {
    render_with_styles(
        hb,
        "admin/login",
        admin_data(
            query,
            has_admin,
            AdminView::Login { next },
            Vec::new(),
            None,
        ),
        StatusCode::OK,
    )
}

async fn render_admin_list(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    query: AdminQuery,
) -> HttpResponse {
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
        Err(_) => return HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    };

    render_with_styles(
        hb,
        "admin/index",
        admin_data(query, true, AdminView::List, documents, None),
        StatusCode::OK,
    )
}

async fn render_admin_edit(
    hb: WebTemplates,
    pool: web::Data<SqlitePool>,
    query: AdminQuery,
    post_id: i64,
) -> HttpResponse {
    let documents = match documents::list(&pool).await {
        Ok(documents) => documents,
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
        admin_data(query, true, AdminView::Edit, documents, selected),
        StatusCode::OK,
    )
}

fn render_admin_import(hb: WebTemplates, query: AdminQuery) -> HttpResponse {
    render_with_styles(
        hb,
        "admin/index",
        admin_data(query, true, AdminView::Import, Vec::new(), None),
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
    query: AdminQuery,
    has_admin: bool,
    view: AdminView<'_>,
    documents: Vec<documents::StoredDocument>,
    selected: Option<documents::StoredDocument>,
) -> serde_json::Value {
    let selected_doc_id = selected.as_ref().map(|document| document.id);
    let index_doc_id = documents.first().map(|document| document.id);
    let selected_doc_label = selected_doc_id.map(|id| document_label(id, index_doc_id));
    let draft = selected
        .as_ref()
        .map(|document| document.doc.as_str())
        .unwrap_or("# First post\n\nStart writing your first Trellis document here.\n");
    let draft_json = serde_json::to_string(draft).unwrap_or_else(|_| "\"\"".to_string());
    let typography = Typography::from_env();
    let posts = documents
        .iter()
        .map(|document| {
            let label = document_label(document.id, index_doc_id);
            json!({
                "id": document.id,
                "label": label,
                "title": markdown_title(&document.doc).unwrap_or(&label),
                "ctime": document.ctime,
                "mtime": document.mtime,
                "selected": Some(document.id) == selected_doc_id,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "styles": "",
        "editor_styles": "",
        "font_css": typography.font_css,
        "fonts_href": typography.fonts_href,
        "site": {
            "name": "Trellis",
            "tagline": serde_json::Value::Null,
        },
        "footer": {
            "version": env!("CARGO_PKG_VERSION"),
            "year": chrono::Utc::now().format("%Y").to_string(),
            "links": [],
        },
        "nav": [],
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
        "selected_doc_id": selected_doc_id,
        "selected_doc_label": selected_doc_label,
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

fn document_label(id: i64, index_doc_id: Option<i64>) -> String {
    if Some(id) == index_doc_id {
        "index.md".to_string()
    } else {
        format!("document-{id}.md")
    }
}

fn markdown_title(markdown: &str) -> Option<&str> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
    })
}
