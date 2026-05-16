mod admin;
mod index;
mod page;

use actix_files::Files;
use actix_web::web;

pub fn config(conf: &mut web::ServiceConfig) {
    let api_scope = web::scope("/api").service(index::healthcheck_handler);
    let site_scope = web::scope("").service(index::index);
    conf.service(api_scope);
    conf.service(
        Files::new("/static", "js/public/assets/")
            .prefer_utf8(true)
            .use_last_modified(true),
    );
    conf.service(site_scope);
}
