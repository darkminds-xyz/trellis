use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub struct StoredDocument {
    pub id: i64,
    pub doc: String,
    pub ctime: Option<String>,
    pub mtime: Option<String>,
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<StoredDocument>> {
    sqlx::query_as::<_, StoredDocument>(
        r#"
        SELECT rowid AS id, doc, ctime, mtime
        FROM documents
        ORDER BY datetime(ctime) ASC, rowid ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<StoredDocument>> {
    sqlx::query_as::<_, StoredDocument>(
        r#"
        SELECT rowid AS id, doc, ctime, mtime
        FROM documents
        WHERE rowid = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(pool: &SqlitePool, doc: &str) -> sqlx::Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO documents (doc, ctime, mtime)
        VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(doc)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn update(pool: &SqlitePool, id: i64, doc: &str) -> sqlx::Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE documents
        SET doc = ?2, mtime = CURRENT_TIMESTAMP
        WHERE rowid = ?1
        "#,
    )
    .bind(id)
    .bind(doc)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn save(pool: &SqlitePool, id: Option<i64>, doc: &str) -> sqlx::Result<i64> {
    if let Some(id) = id {
        if update(pool, id, doc).await? {
            return Ok(id);
        }
    }

    create(pool, doc).await
}
