use anyhow::{Context, anyhow};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use log::warn;
use sqlx::{FromRow, SqlitePool};

use crate::config::AdminConfig;

#[derive(Debug, FromRow)]
struct Account {
    password: String,
}

pub async fn seed_admin_from_config(pool: &SqlitePool, config: &AdminConfig) -> anyhow::Result<()> {
    let username = match config.username.as_deref() {
        Some(username) if !username.trim().is_empty() => username.trim().to_string(),
        _ => {
            warn!("admin.username is not configured in config.yml; /admin login is disabled");
            return Ok(());
        }
    };
    let password = match config.password.as_deref() {
        Some(password) if !password.is_empty() => password,
        _ => {
            warn!("admin.password is not configured in config.yml; /admin login is disabled");
            return Ok(());
        }
    };

    if authenticate(pool, &username, &password).await? {
        return Ok(());
    }

    let password_hash = hash_password(&password)?;
    sqlx::query(
        r#"
        INSERT INTO accounts (is_admin, username, password)
        VALUES (TRUE, ?1, ?2)
        ON CONFLICT(username) DO UPDATE SET
          is_admin = TRUE,
          password = excluded.password
        "#,
    )
    .bind(username)
    .bind(password_hash)
    .execute(pool)
    .await
    .context("failed to seed admin account")?;

    Ok(())
}

pub async fn authenticate(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> anyhow::Result<bool> {
    let account = sqlx::query_as::<_, Account>(
        r#"
        SELECT password
        FROM accounts
        WHERE is_admin = TRUE AND username = ?1
        LIMIT 1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("failed to load admin account")?;

    let Some(account) = account else {
        return Ok(false);
    };

    verify_password(password, &account.password)
}

pub async fn has_admin(pool: &SqlitePool) -> sqlx::Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE is_admin = TRUE")
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow!("failed to hash admin password: {err}"))?;

    Ok(hash.to_string())
}

fn verify_password(password: &str, stored_hash: &str) -> anyhow::Result<bool> {
    let Ok(parsed_hash) = PasswordHash::new(stored_hash) else {
        return Ok(false);
    };

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
