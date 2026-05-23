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
        SELECT id, doc, ctime, mtime
        FROM documents
        ORDER BY datetime(ctime) ASC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<StoredDocument>> {
    sqlx::query_as::<_, StoredDocument>(
        r#"
        SELECT id, doc, ctime, mtime
        FROM documents
        WHERE id = ?1
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
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .bind(doc)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn save(pool: &SqlitePool, id: Option<i64>, doc: &str) -> sqlx::Result<Option<i64>> {
    if let Some(id) = id {
        if update(pool, id, doc).await? {
            return Ok(Some(id));
        }

        return Ok(None);
    }

    create(pool, doc).await.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(include_str!("migrations/001_documents.sql"))
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn save_without_id_creates_document() {
        let pool = test_pool().await;

        let id = save(&pool, None, "new document").await.unwrap();

        assert!(id.is_some());
    }

    #[tokio::test]
    async fn save_with_existing_id_updates_document() {
        let pool = test_pool().await;
        let id = create(&pool, "old document").await.unwrap();

        let saved_id = save(&pool, Some(id), "updated document").await.unwrap();
        let document = get(&pool, id).await.unwrap().unwrap();

        assert_eq!(saved_id, Some(id));
        assert_eq!(document.doc, "updated document");
    }

    #[tokio::test]
    async fn save_with_missing_id_does_not_create_document() {
        let pool = test_pool().await;

        let saved_id = save(&pool, Some(404), "wrong document").await.unwrap();
        let documents = list(&pool).await.unwrap();

        assert_eq!(saved_id, None);
        assert!(documents.is_empty());
    }
}
