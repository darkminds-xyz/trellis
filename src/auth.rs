use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use actix_web::{
    HttpRequest,
    cookie::{Cookie, SameSite, time::Duration},
};
use argon2::password_hash::SaltString;
use rand_core::OsRng;

pub const ADMIN_SESSION_COOKIE: &str = "trellis_admin_session";

#[derive(Debug, Clone, Default)]
pub struct AdminSessions {
    tokens: Arc<Mutex<HashSet<String>>>,
}

impl AdminSessions {
    pub fn create_session_cookie(&self) -> Cookie<'static> {
        let token = SaltString::generate(&mut OsRng).to_string();

        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.insert(token.clone());
        }

        Cookie::build(ADMIN_SESSION_COOKIE, token)
            .path("/admin")
            .http_only(true)
            .same_site(SameSite::Strict)
            .max_age(Duration::hours(12))
            .finish()
    }

    pub fn is_authenticated(&self, req: &HttpRequest) -> bool {
        let Some(cookie) = req.cookie(ADMIN_SESSION_COOKIE) else {
            return false;
        };

        self.tokens
            .lock()
            .map(|tokens| tokens.contains(cookie.value()))
            .unwrap_or(false)
    }

    pub fn clear_session_cookie(&self, req: &HttpRequest) -> Cookie<'static> {
        if let Some(cookie) = req.cookie(ADMIN_SESSION_COOKIE) {
            if let Ok(mut tokens) = self.tokens.lock() {
                tokens.remove(cookie.value());
            }
        }

        Cookie::build(ADMIN_SESSION_COOKIE, "")
            .path("/admin")
            .http_only(true)
            .same_site(SameSite::Strict)
            .max_age(Duration::seconds(0))
            .finish()
    }
}
