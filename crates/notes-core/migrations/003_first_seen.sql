-- First-seen creation evidence. The earliest filesystem birth time ever
-- observed for a path. This is NOT derived from file content: an ordinary
-- save replaces the file's inode and resets its birth time, so `reindex`
-- must preserve this table. Deleting the cache file itself loses it; the
-- fallback is the current birth time, never a fabricated date.
CREATE TABLE first_seen (
    path TEXT PRIMARY KEY NOT NULL,
    created_ms INTEGER NOT NULL
) WITHOUT ROWID;
PRAGMA user_version = 3;
