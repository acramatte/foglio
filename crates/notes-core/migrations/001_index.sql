CREATE TABLE files (
    path TEXT PRIMARY KEY NOT NULL,
    id TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    stale INTEGER NOT NULL DEFAULT 0 CHECK(stale IN (0, 1))
);
CREATE TABLE notes (
    id TEXT PRIMARY KEY NOT NULL,
    path TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL
);
CREATE TABLE tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT COLLATE BINARY NOT NULL,
    PRIMARY KEY(note_id, tag)
);
CREATE VIRTUAL TABLE notes_fts USING fts5(note_id UNINDEXED, title, body, path, tags, tokenize='unicode61');
CREATE TRIGGER notes_delete AFTER DELETE ON notes BEGIN
    DELETE FROM notes_fts WHERE note_id = old.id;
END;
PRAGMA user_version = 1;
