use env_logger::Env;
use log::info;
use std::io;

#[actix_web::main]
async fn main() -> io::Result<()> {
    info!("Growing your garden...");
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    trellis::run().await
}
