PRAGMA foreign_keys = ON;

CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES documents(id) ON DELETE CASCADE,

    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('folder', 'note')),

    current_version_id INTEGER,

    ctime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    mtime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(parent_id, name),
    CHECK (
        (kind = 'folder' AND current_version_id IS NULL)
        OR
        (kind = 'note')
    )
);

CREATE TABLE document_versions (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,

    version_number INTEGER NOT NULL,
    title TEXT,
    markdown TEXT NOT NULL,

    change_summary TEXT,
    ctime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(document_id, version_number)
);

CREATE INDEX idx_documents_parent_id
ON documents(parent_id);

CREATE INDEX idx_document_versions_document_id
ON document_versions(document_id);

CREATE INDEX idx_document_versions_created_at
ON document_versions(ctime);

-- enforces referential integrity aka the selected version must belong to the same document being updated
CREATE TRIGGER validate_current_version
BEFORE UPDATE OF current_version_id ON documents
WHEN NEW.current_version_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'current_version_id must belong to document')
    WHERE NOT EXISTS (
        SELECT 1
        FROM document_versions v
        WHERE v.id = NEW.current_version_id
          AND v.document_id = NEW.id
    );
END;