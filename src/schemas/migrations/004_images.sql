CREATE TABLE IF NOT EXISTS images (
  id TEXT PRIMARY KEY,
  mime TEXT NOT NULL,
  alt TEXT,
  width INTEGER,
  height INTEGER,
  bytes BLOB NOT NULL,
  size_bytes INTEGER NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS document_images (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  document_id INTEGER NOT NULL,
  image_id TEXT NOT NULL,
  UNIQUE(document_id, image_id),
  FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
  FOREIGN KEY (image_id) REFERENCES images(id)
);

CREATE INDEX IF NOT EXISTS idx_document_images_document_id
ON document_images(document_id);
