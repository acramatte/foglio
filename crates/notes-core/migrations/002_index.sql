CREATE TABLE files (
    path TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    stale INTEGER NOT NULL DEFAULT 0 CHECK(stale IN (0, 1))
);
CREATE TABLE notes (
    path TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL
);
CREATE TABLE tags (
    note_path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
    tag TEXT COLLATE BINARY NOT NULL,
    PRIMARY KEY(note_path, tag)
);
CREATE VIRTUAL TABLE notes_fts USING fts5(note_path UNINDEXED, title, body, path, tags, tokenize='unicode61');
CREATE TRIGGER notes_delete AFTER DELETE ON notes BEGIN
    DELETE FROM notes_fts WHERE note_path = old.path;
END;
PRAGMA user_version = 2;
