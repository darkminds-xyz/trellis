use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration as StdDuration, Instant},
};

use actix_web::{
    HttpRequest,
    cookie::{
        Cookie, SameSite,
        time::{Duration, OffsetDateTime},
    },
};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use sqlx::SqlitePool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const ADMIN_SESSION_COOKIE: &str = "trellis_admin_session";
const ADMIN_SESSION_TTL: Duration = Duration::hours(12);
const ADMIN_LOGIN_ATTEMPT_LIMIT: u32 = 5;
const ADMIN_LOGIN_ATTEMPT_WINDOW: StdDuration = StdDuration::from_secs(15 * 60);
const ADMIN_LOGIN_MAX_CONCURRENT_VERIFICATIONS: usize = 2;

#[derive(Debug, Clone)]
pub struct AdminSessions {
    pool: SqlitePool,
    secure_cookies: bool,
}

#[derive(Debug, Clone)]
struct LoginAttempt {
    failed_attempts: u32,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct AdminLoginLimiter {
    attempts: Arc<Mutex<HashMap<String, LoginAttempt>>>,
    verification_slots: Arc<Semaphore>,
}

impl Default for AdminLoginLimiter {
    fn default() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(HashMap::new())),
            verification_slots: Arc::new(Semaphore::new(ADMIN_LOGIN_MAX_CONCURRENT_VERIFICATIONS)),
        }
    }
}

impl AdminLoginLimiter {
    pub fn is_allowed(&self, key: &str) -> bool {
        let now = Instant::now();
        let Ok(mut attempts) = self.attempts.lock() else {
            return false;
        };

        prune_expired_login_attempts(&mut attempts, now);
        attempts
            .get(key)
            .map(|attempt| attempt.failed_attempts < ADMIN_LOGIN_ATTEMPT_LIMIT)
            .unwrap_or(true)
    }

    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let Ok(mut attempts) = self.attempts.lock() else {
            return;
        };

        prune_expired_login_attempts(&mut attempts, now);
        attempts
            .entry(key.to_string())
            .and_modify(|attempt| {
                attempt.failed_attempts = attempt.failed_attempts.saturating_add(1)
            })
            .or_insert(LoginAttempt {
                failed_attempts: 1,
                expires_at: now + ADMIN_LOGIN_ATTEMPT_WINDOW,
            });
    }

    pub fn record_success(&self, key: &str) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts.remove(key);
        }
    }

    pub fn try_acquire_verification(&self) -> Option<OwnedSemaphorePermit> {
        self.verification_slots.clone().try_acquire_owned().ok()
    }
}

impl AdminSessions {
    pub fn new(pool: SqlitePool, secure_cookies: bool) -> Self {
        Self {
            pool,
            secure_cookies,
        }
    }

    pub async fn create_session_cookie(&self) -> sqlx::Result<Cookie<'static>> {
        let token = SaltString::generate(&mut OsRng).to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + ADMIN_SESSION_TTL;

        prune_expired_sessions(&self.pool, now).await?;
        sqlx::query(
            r#"
            INSERT INTO admin_sessions (token, expires_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(&token)
        .bind(expires_at.unix_timestamp())
        .execute(&self.pool)
        .await?;

        Ok(Cookie::build(ADMIN_SESSION_COOKIE, token)
            .path("/")
            .http_only(true)
            .secure(self.secure_cookies)
            .same_site(SameSite::Strict)
            .max_age(ADMIN_SESSION_TTL)
            .finish())
    }

    pub fn clear_session_cookie_variants(&self) -> Vec<Cookie<'static>> {
        ["/", "/admin", "/admin/list", "/api", "/api/admin"]
            .into_iter()
            .map(|path| {
                Cookie::build(ADMIN_SESSION_COOKIE, "")
                    .path(path)
                    .http_only(true)
                    .secure(self.secure_cookies)
                    .same_site(SameSite::Strict)
                    .max_age(Duration::seconds(0))
                    .finish()
            })
            .collect()
    }

    pub async fn is_authenticated(&self, req: &HttpRequest) -> bool {
        let tokens = admin_session_cookie_values(req);
        if tokens.is_empty() {
            return false;
        }

        let now = OffsetDateTime::now_utc();
        if prune_expired_sessions(&self.pool, now).await.is_err() {
            return false;
        }

        for token in tokens {
            let is_valid = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM admin_sessions
                WHERE token = ?1 AND expires_at > ?2
                "#,
            )
            .bind(token)
            .bind(now.unix_timestamp())
            .fetch_one(&self.pool)
            .await
            .map(|count| count > 0)
            .unwrap_or(false);

            if is_valid {
                return true;
            }
        }

        false
    }

    pub async fn clear_session_cookie(&self, req: &HttpRequest) -> Cookie<'static> {
        let now = OffsetDateTime::now_utc();

        let _ = prune_expired_sessions(&self.pool, now).await;
        let tokens = admin_session_cookie_values(req);
        for token in tokens {
            let _ = sqlx::query("DELETE FROM admin_sessions WHERE token = ?1")
                .bind(token)
                .execute(&self.pool)
                .await;
        }

        self.clear_session_cookie_variants()
            .into_iter()
            .next()
            .expect("session cookie variants include root path")
    }
}

fn admin_session_cookie_values(req: &HttpRequest) -> Vec<String> {
    req.cookies()
        .map(|cookies| {
            cookies
                .iter()
                .filter(|cookie| cookie.name() == ADMIN_SESSION_COOKIE)
                .map(|cookie| cookie.value().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

async fn prune_expired_sessions(pool: &SqlitePool, now: OffsetDateTime) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM admin_sessions WHERE expires_at <= ?1")
        .bind(now.unix_timestamp())
        .execute(pool)
        .await?;

    Ok(())
}

fn prune_expired_login_attempts(attempts: &mut HashMap<String, LoginAttempt>, now: Instant) {
    attempts.retain(|_, attempt| attempt.expires_at > now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_sessions() -> AdminSessions {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(include_str!("schemas/migrations/003_admin_sessions.sql"))
            .execute(&pool)
            .await
            .unwrap();

        AdminSessions::new(pool, true)
    }

    #[actix_web::test]
    async fn created_session_authenticates_until_expiry() {
        let sessions = test_sessions().await;
        let cookie = sessions.create_session_cookie().await.unwrap();
        let req = actix_web::test::TestRequest::default()
            .cookie(cookie)
            .to_http_request();

        assert!(sessions.is_authenticated(&req).await);
    }

    #[actix_web::test]
    async fn created_session_survives_new_session_store_instance() {
        let sessions = test_sessions().await;
        let cookie = sessions.create_session_cookie().await.unwrap();
        let req = actix_web::test::TestRequest::default()
            .cookie(cookie)
            .to_http_request();
        let reloaded_sessions = AdminSessions::new(sessions.pool.clone(), true);

        assert!(reloaded_sessions.is_authenticated(&req).await);
    }

    #[actix_web::test]
    async fn duplicate_session_cookies_authenticate_when_any_token_is_valid() {
        let sessions = test_sessions().await;
        let valid_cookie = sessions.create_session_cookie().await.unwrap();
        let req = actix_web::test::TestRequest::default()
            .cookie(Cookie::new(ADMIN_SESSION_COOKIE, "stale-session"))
            .cookie(valid_cookie)
            .to_http_request();

        assert!(sessions.is_authenticated(&req).await);
    }

    #[actix_web::test]
    async fn session_cookie_secure_flag_is_configurable() {
        let sessions = test_sessions().await;
        let secure_cookie = sessions.create_session_cookie().await.unwrap();
        assert!(secure_cookie.secure().unwrap_or(false));

        let local_sessions = AdminSessions::new(sessions.pool.clone(), false);
        let local_cookie = local_sessions.create_session_cookie().await.unwrap();
        assert!(!local_cookie.secure().unwrap_or(false));
    }

    #[actix_web::test]
    async fn expired_session_does_not_authenticate_and_is_pruned() {
        let sessions = test_sessions().await;
        let token = "expired-session".to_string();

        sqlx::query("INSERT INTO admin_sessions (token, expires_at) VALUES (?1, ?2)")
            .bind(&token)
            .bind((OffsetDateTime::now_utc() - Duration::seconds(1)).unix_timestamp())
            .execute(&sessions.pool)
            .await
            .unwrap();

        let req = actix_web::test::TestRequest::default()
            .cookie(Cookie::new(ADMIN_SESSION_COOKIE, token.clone()))
            .to_http_request();

        assert!(!sessions.is_authenticated(&req).await);

        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM admin_sessions WHERE token = ?1")
                .bind(&token)
                .fetch_one(&sessions.pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[actix_web::test]
    async fn clear_session_removes_backing_token() {
        let sessions = test_sessions().await;
        let cookie = sessions.create_session_cookie().await.unwrap();
        let token = cookie.value().to_owned();
        let req = actix_web::test::TestRequest::default()
            .cookie(cookie)
            .to_http_request();

        sessions.clear_session_cookie(&req).await;

        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM admin_sessions WHERE token = ?1")
                .bind(&token)
                .fetch_one(&sessions.pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn failed_logins_are_limited_until_window_expires() {
        let limiter = AdminLoginLimiter::default();
        let key = "127.0.0.1:admin";

        for _ in 0..ADMIN_LOGIN_ATTEMPT_LIMIT {
            assert!(limiter.is_allowed(key));
            limiter.record_failure(key);
        }

        assert!(!limiter.is_allowed(key));

        limiter
            .attempts
            .lock()
            .unwrap()
            .get_mut(key)
            .unwrap()
            .expires_at = Instant::now() - StdDuration::from_secs(1);

        assert!(limiter.is_allowed(key));
        assert!(!limiter.attempts.lock().unwrap().contains_key(key));
    }

    #[test]
    fn successful_login_clears_failed_attempts() {
        let limiter = AdminLoginLimiter::default();
        let key = "127.0.0.1:admin";

        limiter.record_failure(key);
        limiter.record_success(key);

        assert!(limiter.is_allowed(key));
        assert!(!limiter.attempts.lock().unwrap().contains_key(key));
    }

    #[test]
    fn argon2_verifications_are_capped() {
        let limiter = AdminLoginLimiter::default();
        let first = limiter.try_acquire_verification();
        let second = limiter.try_acquire_verification();

        assert!(first.is_some());
        assert!(second.is_some());
        assert!(limiter.try_acquire_verification().is_none());

        drop(first);
        assert!(limiter.try_acquire_verification().is_some());
    }
}
