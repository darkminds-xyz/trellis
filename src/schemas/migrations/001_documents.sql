PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES documents(id) ON DELETE CASCADE,

    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('folder', 'note')),

    current_version_id INTEGER REFERENCES document_versions(id) ON DELETE SET NULL,
    hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
    draft INTEGER NOT NULL DEFAULT 0 CHECK (draft IN (0, 1)),

    ctime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    mtime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CHECK (length(trim(name)) > 0),
    CHECK (
        (kind = 'folder' AND current_version_id IS NULL AND draft = 0)
        OR
        (kind = 'note')
    )
);

CREATE TABLE IF NOT EXISTS document_versions (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,

    version_number INTEGER NOT NULL,
    title TEXT,
    markdown TEXT NOT NULL,

    change_summary TEXT,
    ctime TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(document_id, version_number)
);

CREATE INDEX IF NOT EXISTS idx_documents_parent_id
ON documents(parent_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_unique_root_name
ON documents(name)
WHERE parent_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_unique_child_name
ON documents(parent_id, name)
WHERE parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_documents_kind
ON documents(kind);

CREATE INDEX IF NOT EXISTS idx_document_versions_document_id
ON document_versions(document_id);

CREATE INDEX IF NOT EXISTS idx_document_versions_created_at
ON document_versions(ctime);

-- enforces referential integrity aka the selected version must belong to the same document being updated
CREATE TRIGGER IF NOT EXISTS validate_current_version
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

CREATE TRIGGER IF NOT EXISTS validate_parent_on_insert
BEFORE INSERT ON documents
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'parent_id must reference a folder')
    WHERE NOT EXISTS (
        SELECT 1
        FROM documents parent
        WHERE parent.id = NEW.parent_id
          AND parent.kind = 'folder'
    );
END;

CREATE TRIGGER IF NOT EXISTS validate_parent_on_update
BEFORE UPDATE OF parent_id ON documents
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'parent_id must reference a folder')
    WHERE NEW.parent_id = NEW.id
       OR NOT EXISTS (
        SELECT 1
        FROM documents parent
        WHERE parent.id = NEW.parent_id
          AND parent.kind = 'folder'
    );
END;
