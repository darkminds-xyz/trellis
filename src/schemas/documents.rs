use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentErrorKind {
    DuplicateName,
    FolderNotEmpty,
    InvalidDocumentKind,
    InvalidParent,
}

#[derive(Debug)]
pub enum DocumentError {
    Sqlx(sqlx::Error),
    Domain(DocumentErrorKind),
}

impl From<sqlx::Error> for DocumentError {
    fn from(err: sqlx::Error) -> Self {
        if is_duplicate_name_error(&err) {
            return Self::Domain(DocumentErrorKind::DuplicateName);
        }

        Self::Sqlx(err)
    }
}

pub type DocumentResult<T> = Result<T, DocumentError>;

fn is_duplicate_name_error(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|err| {
        let message = err.message();
        message.contains("idx_documents_unique_root_name")
            || message.contains("idx_documents_unique_child_name")
            || message.contains("UNIQUE constraint failed: documents.name")
            || message.contains("UNIQUE constraint failed: documents.parent_id, documents.name")
    })
}

#[derive(Debug, Clone, FromRow)]
pub struct StoredDocument {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub current_version_id: Option<i64>,
    pub version_number: Option<i64>,
    pub title: Option<String>,
    pub doc: String,
    pub hidden: bool,
    pub draft: bool,
    pub ctime: Option<String>,
    pub mtime: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DocumentNode {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub current_version_id: Option<i64>,
    pub title: Option<String>,
    pub hidden: bool,
    pub draft: bool,
    pub ctime: Option<String>,
    pub mtime: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DocumentVersion {
    pub id: i64,
    pub document_id: i64,
    pub version_number: i64,
    pub title: Option<String>,
    pub markdown: String,
    pub change_summary: Option<String>,
    pub ctime: String,
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<StoredDocument>> {
    list_notes(pool, false).await
}

pub async fn list_public(pool: &SqlitePool) -> sqlx::Result<Vec<StoredDocument>> {
    list_notes(pool, true).await
}

pub async fn list_nodes(pool: &SqlitePool) -> sqlx::Result<Vec<DocumentNode>> {
    sqlx::query_as::<_, DocumentNode>(
        r#"
        SELECT d.id, d.parent_id, d.name, d.kind, d.current_version_id,
               v.title, d.hidden, d.draft, d.ctime, d.mtime
        FROM documents d
        LEFT JOIN document_versions v ON v.id = d.current_version_id
        ORDER BY d.parent_id IS NOT NULL, d.parent_id, d.kind, d.name COLLATE NOCASE, d.id
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<StoredDocument>> {
    sqlx::query_as::<_, StoredDocument>(
        r#"
        SELECT d.id, d.parent_id, d.name, d.kind, d.current_version_id,
               v.version_number, v.title, COALESCE(v.markdown, '') AS doc,
               d.hidden, d.draft, d.ctime, d.mtime
        FROM documents d
        LEFT JOIN document_versions v ON v.id = d.current_version_id
        WHERE d.id = ?1 AND d.kind = 'note'
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_node(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<DocumentNode>> {
    sqlx::query_as::<_, DocumentNode>(
        r#"
        SELECT d.id, d.parent_id, d.name, d.kind, d.current_version_id,
               v.title, d.hidden, d.draft, d.ctime, d.mtime
        FROM documents d
        LEFT JOIN document_versions v ON v.id = d.current_version_id
        WHERE d.id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_versions(
    pool: &SqlitePool,
    document_id: i64,
) -> sqlx::Result<Vec<DocumentVersion>> {
    sqlx::query_as::<_, DocumentVersion>(
        r#"
        SELECT id, document_id, version_number, title, markdown, change_summary, ctime
        FROM document_versions
        WHERE document_id = ?1
        ORDER BY version_number DESC
        "#,
    )
    .bind(document_id)
    .fetch_all(pool)
    .await
}

pub async fn create_folder(
    pool: &SqlitePool,
    parent_id: Option<i64>,
    name: &str,
    hidden: bool,
) -> DocumentResult<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO documents (parent_id, name, kind, hidden, draft, ctime, mtime)
        VALUES (?1, ?2, 'folder', ?3, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(parent_id)
    .bind(name.trim())
    .bind(hidden)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn update_folder(
    pool: &SqlitePool,
    id: i64,
    name: Option<&str>,
    parent_id: Option<Option<i64>>,
    hidden: Option<bool>,
) -> DocumentResult<bool> {
    let Some(node) = get_node(pool, id).await? else {
        return Ok(false);
    };
    if node.kind != "folder" {
        return Err(DocumentError::Domain(
            DocumentErrorKind::InvalidDocumentKind,
        ));
    }
    if let Some(parent_id) = parent_id {
        validate_folder_parent(pool, id, parent_id).await?;
    }

    let result = sqlx::query(
        r#"
        UPDATE documents
        SET name = ?2,
            parent_id = ?3,
            hidden = ?4,
            mtime = CURRENT_TIMESTAMP
        WHERE id = ?1 AND kind = 'folder'
        "#,
    )
    .bind(id)
    .bind(name.map(str::trim).unwrap_or(&node.name))
    .bind(parent_id.unwrap_or(node.parent_id))
    .bind(hidden.unwrap_or(node.hidden))
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
pub async fn create(pool: &SqlitePool, doc: &str) -> sqlx::Result<i64> {
    create_root_note(pool, doc, false, None, None)
        .await
        .map_err(document_error_into_sqlx)
}

pub async fn create_root_note(
    pool: &SqlitePool,
    markdown: &str,
    draft: bool,
    title: Option<&str>,
    change_summary: Option<&str>,
) -> DocumentResult<i64> {
    let name = next_root_note_name(pool).await?;
    create_note(pool, None, &name, markdown, draft, title, change_summary).await
}

pub async fn create_note(
    pool: &SqlitePool,
    parent_id: Option<i64>,
    name: &str,
    markdown: &str,
    draft: bool,
    title: Option<&str>,
    change_summary: Option<&str>,
) -> DocumentResult<i64> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"
        INSERT INTO documents (parent_id, name, kind, draft, ctime, mtime)
        VALUES (?1, ?2, 'note', ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(parent_id)
    .bind(name.trim())
    .bind(draft)
    .execute(&mut *tx)
    .await?;
    let document_id = result.last_insert_rowid();

    let version_id =
        insert_version(&mut tx, document_id, 1, markdown, title, change_summary).await?;
    sqlx::query(
        r#"
        UPDATE documents
        SET current_version_id = ?2, mtime = CURRENT_TIMESTAMP
        WHERE id = ?1
        "#,
    )
    .bind(document_id)
    .bind(version_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(document_id)
}

#[cfg(test)]
pub async fn update(pool: &SqlitePool, id: i64, doc: &str) -> sqlx::Result<bool> {
    update_note(pool, id, None, None, Some(doc), None, None, None)
        .await
        .map_err(document_error_into_sqlx)
}

pub async fn update_note(
    pool: &SqlitePool,
    id: i64,
    name: Option<&str>,
    parent_id: Option<Option<i64>>,
    markdown: Option<&str>,
    draft: Option<bool>,
    title: Option<&str>,
    change_summary: Option<&str>,
) -> DocumentResult<bool> {
    let Some(note) = get(pool, id).await? else {
        return Ok(false);
    };

    let mut tx = pool.begin().await?;
    let mut current_version_id = note.current_version_id;

    if let Some(markdown) = markdown {
        let next_version = note.version_number.unwrap_or(0) + 1;
        let version_id =
            insert_version(&mut tx, id, next_version, markdown, title, change_summary).await?;
        current_version_id = Some(version_id);
    }

    let result = sqlx::query(
        r#"
        UPDATE documents
        SET name = ?2,
            parent_id = ?3,
            draft = ?4,
            current_version_id = ?5,
            mtime = CURRENT_TIMESTAMP
        WHERE id = ?1 AND kind = 'note'
        "#,
    )
    .bind(id)
    .bind(name.map(str::trim).unwrap_or(&note.name))
    .bind(parent_id.unwrap_or(note.parent_id))
    .bind(draft.unwrap_or(note.draft))
    .bind(current_version_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
pub async fn save(pool: &SqlitePool, id: Option<i64>, doc: &str) -> sqlx::Result<Option<i64>> {
    if let Some(id) = id {
        if update(pool, id, doc).await? {
            return Ok(Some(id));
        }

        return Ok(None);
    }

    create(pool, doc).await.map(Some)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> DocumentResult<bool> {
    let Some(node) = get_node(pool, id).await? else {
        return Ok(false);
    };

    if node.kind == "folder" && folder_contains_notes(pool, id).await? {
        return Err(DocumentError::Domain(DocumentErrorKind::FolderNotEmpty));
    }

    let result = sqlx::query("DELETE FROM documents WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn list_notes(pool: &SqlitePool, public_only: bool) -> sqlx::Result<Vec<StoredDocument>> {
    if public_only {
        return sqlx::query_as::<_, StoredDocument>(
            r#"
            SELECT d.id, d.parent_id, d.name, d.kind, d.current_version_id,
                   v.version_number, v.title, COALESCE(v.markdown, '') AS doc,
                   d.hidden, d.draft, d.ctime, d.mtime
            FROM documents d
            LEFT JOIN document_versions v ON v.id = d.current_version_id
            WHERE d.kind = 'note'
              AND d.draft = 0
              AND NOT EXISTS (
                WITH RECURSIVE ancestors(id, hidden, parent_id) AS (
                    SELECT parent.id, parent.hidden, parent.parent_id
                    FROM documents parent
                    WHERE parent.id = d.parent_id
                    UNION ALL
                    SELECT parent.id, parent.hidden, parent.parent_id
                    FROM documents parent
                    JOIN ancestors ON ancestors.parent_id = parent.id
                )
                SELECT 1 FROM ancestors WHERE hidden = 1
              )
            ORDER BY datetime(d.ctime) ASC, d.id ASC
            "#,
        )
        .fetch_all(pool)
        .await;
    }

    sqlx::query_as::<_, StoredDocument>(
        r#"
        SELECT d.id, d.parent_id, d.name, d.kind, d.current_version_id,
               v.version_number, v.title, COALESCE(v.markdown, '') AS doc,
               d.hidden, d.draft, d.ctime, d.mtime
        FROM documents d
        LEFT JOIN document_versions v ON v.id = d.current_version_id
        WHERE d.kind = 'note'
        ORDER BY datetime(d.ctime) ASC, d.id ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn folder_contains_notes(pool: &SqlitePool, folder_id: i64) -> sqlx::Result<bool> {
    let count: (i64,) = sqlx::query_as(
        r#"
        WITH RECURSIVE descendants(id, kind) AS (
            SELECT id, kind
            FROM documents
            WHERE parent_id = ?1
            UNION ALL
            SELECT child.id, child.kind
            FROM documents child
            JOIN descendants parent ON child.parent_id = parent.id
        )
        SELECT COUNT(*)
        FROM descendants
        WHERE kind = 'note'
        "#,
    )
    .bind(folder_id)
    .fetch_one(pool)
    .await?;

    Ok(count.0 > 0)
}

async fn validate_folder_parent(
    pool: &SqlitePool,
    folder_id: i64,
    parent_id: Option<i64>,
) -> DocumentResult<()> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };

    if parent_id == folder_id {
        return Err(DocumentError::Domain(DocumentErrorKind::InvalidParent));
    }

    let invalid: (i64,) = sqlx::query_as(
        r#"
        WITH RECURSIVE descendants(id) AS (
            SELECT id
            FROM documents
            WHERE parent_id = ?1
            UNION ALL
            SELECT child.id
            FROM documents child
            JOIN descendants parent ON child.parent_id = parent.id
        )
        SELECT COUNT(*)
        FROM descendants
        WHERE id = ?2
        "#,
    )
    .bind(folder_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;

    if invalid.0 > 0 {
        return Err(DocumentError::Domain(DocumentErrorKind::InvalidParent));
    }

    Ok(())
}

async fn insert_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    document_id: i64,
    version_number: i64,
    markdown: &str,
    title: Option<&str>,
    change_summary: Option<&str>,
) -> sqlx::Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO document_versions
            (document_id, version_number, title, markdown, change_summary, ctime)
        VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(document_id)
    .bind(version_number)
    .bind(title)
    .bind(markdown)
    .bind(change_summary)
    .execute(&mut **tx)
    .await?;

    Ok(result.last_insert_rowid())
}

async fn next_root_note_name(pool: &SqlitePool) -> sqlx::Result<String> {
    let names: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT name
        FROM documents
        WHERE parent_id IS NULL AND kind = 'note'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let existing = names
        .into_iter()
        .map(|(name,)| name)
        .collect::<std::collections::BTreeSet<_>>();
    if !existing.contains("index.md") {
        return Ok("index.md".to_string());
    }

    for number in 2.. {
        let candidate = format!("document-{number}.md");
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }

    unreachable!("unbounded note name search always returns")
}

#[cfg(test)]
fn document_error_into_sqlx(err: DocumentError) -> sqlx::Error {
    match err {
        DocumentError::Sqlx(err) => err,
        DocumentError::Domain(kind) => sqlx::Error::Protocol(format!("{kind:?}")),
    }
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
    async fn save_with_existing_id_updates_document_and_keeps_history() {
        let pool = test_pool().await;
        let id = create(&pool, "old document").await.unwrap();

        let saved_id = save(&pool, Some(id), "updated document").await.unwrap();
        let document = get(&pool, id).await.unwrap().unwrap();
        let versions = list_versions(&pool, id).await.unwrap();

        assert_eq!(saved_id, Some(id));
        assert_eq!(document.doc, "updated document");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_number, 2);
    }

    #[tokio::test]
    async fn save_with_missing_id_does_not_create_document() {
        let pool = test_pool().await;

        let saved_id = save(&pool, Some(404), "wrong document").await.unwrap();
        let documents = list(&pool).await.unwrap();

        assert_eq!(saved_id, None);
        assert!(documents.is_empty());
    }

    #[tokio::test]
    async fn folders_can_be_hidden_from_public_notes() {
        let pool = test_pool().await;
        let public_folder = create_folder(&pool, None, "public", false).await.unwrap();
        let hidden_folder = create_folder(&pool, None, "private", true).await.unwrap();
        create_note(
            &pool,
            Some(public_folder),
            "one.md",
            "one",
            false,
            None,
            None,
        )
        .await
        .unwrap();
        create_note(
            &pool,
            Some(hidden_folder),
            "two.md",
            "two",
            false,
            None,
            None,
        )
        .await
        .unwrap();
        create_note(&pool, None, "draft.md", "draft", true, None, None)
            .await
            .unwrap();

        let public = list_public(&pool).await.unwrap();

        assert_eq!(public.len(), 1);
        assert_eq!(public[0].doc, "one");
    }

    #[tokio::test]
    async fn deleting_folder_with_notes_is_rejected() {
        let pool = test_pool().await;
        let folder_id = create_folder(&pool, None, "folder", false).await.unwrap();
        create_note(&pool, Some(folder_id), "note.md", "note", false, None, None)
            .await
            .unwrap();

        let result = delete(&pool, folder_id).await;

        assert!(matches!(
            result,
            Err(DocumentError::Domain(DocumentErrorKind::FolderNotEmpty))
        ));
    }

    #[tokio::test]
    async fn deleting_empty_folder_is_allowed() {
        let pool = test_pool().await;
        let folder_id = create_folder(&pool, None, "folder", false).await.unwrap();

        let deleted = delete(&pool, folder_id).await.unwrap();
        let nodes = list_nodes(&pool).await.unwrap();

        assert!(deleted);
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn note_can_be_moved_between_folders() {
        let pool = test_pool().await;
        let folder_id = create_folder(&pool, None, "folder", false).await.unwrap();
        let note_id = create_note(&pool, None, "note.md", "note", true, None, None)
            .await
            .unwrap();

        let moved = update_note(
            &pool,
            note_id,
            None,
            Some(Some(folder_id)),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        let note = get(&pool, note_id).await.unwrap().unwrap();

        assert!(moved);
        assert_eq!(note.parent_id, Some(folder_id));
    }

    #[tokio::test]
    async fn folder_cannot_be_moved_inside_descendant() {
        let pool = test_pool().await;
        let parent_id = create_folder(&pool, None, "parent", false).await.unwrap();
        let child_id = create_folder(&pool, Some(parent_id), "child", false)
            .await
            .unwrap();

        let result = update_folder(&pool, parent_id, None, Some(Some(child_id)), None).await;

        assert!(matches!(
            result,
            Err(DocumentError::Domain(DocumentErrorKind::InvalidParent))
        ));
    }
}
