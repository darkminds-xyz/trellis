mod auth;
mod config;
mod handlers;

use log::info;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::{env, io};

use actix_cors::Cors;
use actix_web::{App, HttpServer, http::header, web};
use handlebars::Handlebars;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tokio::fs::File;
use walkdir::WalkDir;

pub type WebTemplates = web::Data<Handlebars<'static>>;

pub async fn run() -> io::Result<()> {
    let pool = get_db_pool()
        .await
        .expect("Unable to create or load existing sqlite database!");

    HttpServer::new(move || {
        App::new()
            .app_data(web::PayloadConfig::new(12))
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(build_handlebars()))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
                    .allowed_headers(vec![header::CONTENT_TYPE, header::ACCEPT]),
            )
            .configure(handlers::config)
    })
    .bind(("0.0.0.0", 47055))?
    .run()
    .await
}

fn build_handlebars() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    // Register every .hbs file in `templates/`` so they are available
    let templates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    for entry in WalkDir::new(&templates_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file() && e.path().extension() == Some(OsStr::new("hbs")))
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(&templates_dir)
            .expect("template path prefix");
        let rel_no_ext = rel.with_extension("");
        let name = rel_no_ext.to_string_lossy().replace('\\', "/");

        if rel.parent().map(|p| p == Path::new("")).unwrap_or(true) {
            // top-level templates. e.g. index, page
            let stem = rel_no_ext
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("template");
            handlebars
                .register_template_file(stem, &path)
                .unwrap_or_else(|e| panic!("failed to register template {}: {}", stem, e));
        } else {
            // nested templates treated as partials (e.g., components/...)
            let partial_src = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read partial {}: {}", name, e));
            handlebars
                .register_partial(name.as_str(), partial_src)
                .unwrap_or_else(|e| panic!("failed to register partial {}: {}", name, e));
        }
    }
    handlebars
}

pub async fn get_db_pool() -> anyhow::Result<SqlitePool> {
    // Override database path via .env
    let url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        let mut path = env::current_dir().expect("cwd");
        path.push("trellis.db");
        path.display().to_string()
    });

    let uri = format!("sqlite://{}", &url);
    let db_path = std::path::PathBuf::from(&url);

    // Ensure the directories exist and create db if missing
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    if tokio::fs::metadata(&db_path).await.is_err() {
        File::create(&db_path).await?;
    }

    info!("Loading sqlite database: {}", &uri);
    let pool = SqlitePoolOptions::new().connect(&uri).await?;
    Ok(pool)
}
