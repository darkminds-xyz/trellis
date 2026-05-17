use sqlx::SqlitePool;

const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_documents.sql"),
    include_str!("migrations/002_accounts.sql"),
];

pub async fn run(pool: &SqlitePool) -> sqlx::Result<()> {
    for migration in MIGRATIONS {
        sqlx::query(migration).execute(pool).await?;
    }

    Ok(())
}
