mod auth;
mod config;
mod handlers;
mod markdown;
mod schemas;
mod typography;

use log::info;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::{env, io};

use actix_cors::Cors;
use actix_web::{App, HttpServer, http::header, middleware, web};
use handlebars::Handlebars;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tokio::fs::File;
use walkdir::WalkDir;

use crate::config::AppConfig;

pub type WebTemplates = web::Data<Handlebars<'static>>;

pub async fn run() -> io::Result<()> {
    let config = AppConfig::load().expect("Unable to load config.yml");
    let pool = get_db_pool(&config)
        .await
        .expect("Unable to create or load existing sqlite database!");
    let config = web::Data::new(config);
    let admin_sessions = web::Data::new(auth::AdminSessions::new(
        pool.clone(),
        config.admin.secure_cookies,
    ));
    let admin_login_limiter = web::Data::new(auth::AdminLoginLimiter::default());

    let server_addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Trellis is listening on: http://{}", &server_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(5 * 1024 * 1024))
            .app_data(web::Data::new(pool.clone()))
            .app_data(config.clone())
            .app_data(admin_sessions.clone())
            .app_data(admin_login_limiter.clone())
            .app_data(web::Data::new(build_handlebars()))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
                    .allowed_headers(vec![header::CONTENT_TYPE, header::ACCEPT]),
            )
            .wrap(middleware::NormalizePath::trim())
            .configure(handlers::config)
    })
    .bind(server_addr)?
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
            let partial_src = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read partial {}: {}", name, e));
            handlebars
                .register_partial(stem, partial_src)
                .unwrap_or_else(|e| panic!("failed to register partial {}: {}", stem, e));
        } else {
            // nested templates treated as partials (e.g., components/...)
            let partial_src = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read partial {}: {}", name, e));
            handlebars
                .register_template_file(name.as_str(), &path)
                .unwrap_or_else(|e| panic!("failed to register template {}: {}", name, e));
            handlebars
                .register_partial(name.as_str(), partial_src)
                .unwrap_or_else(|e| panic!("failed to register partial {}: {}", name, e));
        }
    }
    handlebars
}

pub async fn get_db_pool(config: &AppConfig) -> anyhow::Result<SqlitePool> {
    let (uri, db_path) = sqlite_database_config(config.database_url.clone());

    // Ensure the directories exist and create db if missing
    if let Some(db_path) = db_path {
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        if tokio::fs::metadata(&db_path).await.is_err() {
            File::create(&db_path).await?;
        }
    }

    info!("Loading sqlite database: {}", &uri);
    let options = uri
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    schemas::migrations::run(&pool).await?;
    schemas::accounts::seed_admin_from_config(&pool, &config.admin).await?;
    Ok(pool)
}

fn sqlite_database_config(database_url: Option<String>) -> (String, Option<PathBuf>) {
    let value = database_url.unwrap_or_else(default_database_path);

    if value == "sqlite::memory:" || value.starts_with("sqlite::memory:?") {
        return (value, None);
    }

    if let Some(path) = value.strip_prefix("sqlite://") {
        return (value.clone(), sqlite_url_path(path).map(PathBuf::from));
    }

    if value.starts_with("sqlite:") {
        return (value, None);
    }

    let path = PathBuf::from(&value);
    (format!("sqlite://{value}"), Some(path))
}

fn default_database_path() -> String {
    let mut path = env::current_dir().expect("cwd");
    path.push("trellis.db");
    path.display().to_string()
}

fn sqlite_url_path(path_and_query: &str) -> Option<&str> {
    let path = path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query);

    (!path.is_empty() && path != ":memory:").then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_database_path_becomes_sqlite_url_and_filesystem_path() {
        let (uri, path) = sqlite_database_config(Some("trellis.db".to_string()));

        assert_eq!(uri, "sqlite://trellis.db");
        assert_eq!(path, Some(PathBuf::from("trellis.db")));
    }

    #[test]
    fn absolute_database_path_becomes_sqlite_url_and_filesystem_path() {
        let (uri, path) = sqlite_database_config(Some("/tmp/trellis.db".to_string()));

        assert_eq!(uri, "sqlite:///tmp/trellis.db");
        assert_eq!(path, Some(PathBuf::from("/tmp/trellis.db")));
    }

    #[test]
    fn sqlite_url_is_not_prefixed_again() {
        let (uri, path) = sqlite_database_config(Some("sqlite://trellis.db".to_string()));

        assert_eq!(uri, "sqlite://trellis.db");
        assert_eq!(path, Some(PathBuf::from("trellis.db")));
    }

    #[test]
    fn sqlite_url_filesystem_path_ignores_query_string() {
        let (uri, path) =
            sqlite_database_config(Some("sqlite://data/trellis.db?mode=rwc".to_string()));

        assert_eq!(uri, "sqlite://data/trellis.db?mode=rwc");
        assert_eq!(path, Some(PathBuf::from("data/trellis.db")));
    }

    #[test]
    fn sqlite_memory_url_has_no_filesystem_path() {
        let (uri, path) = sqlite_database_config(Some("sqlite::memory:".to_string()));

        assert_eq!(uri, "sqlite::memory:");
        assert_eq!(path, None);
    }
}
