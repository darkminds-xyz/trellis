use sqlx::SqlitePool;

const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_documents.sql"),
    include_str!("migrations/002_accounts.sql"),
    include_str!("migrations/003_admin_sessions.sql"),
    include_str!("migrations/004_images.sql"),
];

pub async fn run(pool: &SqlitePool) -> sqlx::Result<()> {
    for migration in MIGRATIONS {
        sqlx::query(*migration).execute(pool).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn migrations_can_run_more_than_once() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        run(&pool).await.unwrap();
        run(&pool).await.unwrap();
    }
}
