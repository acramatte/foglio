//! Safe FTS expressions and parameterized filters. Snippets are plain text, not HTML.
use crate::{
    Error, ErrorCode, Library, Result,
    index::{IndexStatus, db_error},
};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::collections::HashSet;

/// Weighted BM25 over (note_path, title, body, path, tags): a title hit
/// outweighs the same word in the Markdown body ten to one, and `note_path` is
/// UNINDEXED, so it must weigh zero. Equal relevance falls back to binary path.
const FIND: &str = concat!(
    "SELECT n.path,n.title,snippet(notes_fts,-1,'','','…',24),bm25(notes_fts,0,10,1,2,2)",
    " FROM notes_fts JOIN notes n ON n.path=notes_fts.note_path",
    " WHERE notes_fts MATCH ?1",
    " AND (?2 IS NULL OR EXISTS(SELECT 1 FROM tags t WHERE t.note_path=n.path AND t.tag=?2 COLLATE BINARY))",
    " AND (?3 IS NULL OR substr(n.path,1,length(?3)+1)=?3||'/')",
    " ORDER BY bm25(notes_fts,0,10,1,2,2),n.path COLLATE BINARY LIMIT ?4"
);
/// Shortest token Smart mode may expand: expanding complete single characters
/// would match nearly every note and make the results meaningless.
const PREFIX_FROM: usize = 2;

#[derive(Debug, Clone, Copy, Default)]
pub enum SearchMode {
    #[default]
    Literal,
    Phrase,
    Prefix,
    /// Complete words first, then the notes where every word also matches as a
    /// prefix, so `memo` reaches a note titled `Memory`.
    Smart,
}
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,
    pub tag: Option<String>,
    pub folder: Option<String>,
    pub limit: usize,
}
impl SearchQuery {
    pub fn literal(text: &str) -> Self {
        Self::text(text)
    }
    pub fn smart(text: &str) -> Self {
        Self {
            mode: SearchMode::Smart,
            ..Self::text(text)
        }
    }
    fn text(text: &str) -> Self {
        Self {
            text: text.into(),
            mode: SearchMode::Literal,
            tag: None,
            folder: None,
            limit: 50,
        }
    }
    fn validate(&self) -> Result<()> {
        if self.text.len() > 4096 || self.limit == 0 || self.limit > 1000 {
            return Err(Error::new(
                ErrorCode::Usage,
                "query is limited to 4096 bytes and result limit to 1..1000",
            ));
        }
        if let Some(folder) = &self.folder {
            crate::filesystem::relative(&format!("{folder}/filter.md"))?;
        }
        if !self.text.chars().any(char::is_alphanumeric) || self.text.contains('\0') {
            return Err(Error::new(
                ErrorCode::Usage,
                "query needs at least one letter or number and no NUL",
            ));
        }
        Ok(())
    }
}
/// Quote one token so punctuation in a query can never become FTS syntax.
fn quoted(token: &str) -> String {
    format!("\"{}\"", token.replace('"', "\"\""))
}
/// Literal mode: every token must match a complete indexed word.
fn complete(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| quoted(token))
        .collect::<Vec<_>>()
        .join(" AND ")
}
/// Explicit prefix mode: every token is a prefix, however short.
fn prefix_all(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| format!("{}*", quoted(token)))
        .collect::<Vec<_>>()
        .join(" AND ")
}
/// Smart expansion: tokens long enough to be selective also match words that
/// start with them; shorter tokens stay exact.
fn prefix_expanded(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| {
            if token.chars().count() < PREFIX_FROM {
                quoted(token)
            } else {
                format!("{}*", quoted(token))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub rank: f64,
}
#[derive(Debug, Serialize)]
pub struct SearchReport {
    pub hits: Vec<SearchHit>,
    pub status: IndexStatus,
}
/// A reconciled search snapshot holding the cooperative lock. Drop promptly to
/// release writers. Reuse for repeated queries without rescanning between keys.
pub struct SearchSession {
    // Fields drop in declaration order: close SQLite before releasing its lock.
    connection: Connection,
    _lock: crate::filesystem::LibraryLock,
    pub status: IndexStatus,
}
impl SearchSession {
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>> {
        query.validate()?;
        let tokens = self.query_tokens(&query.text)?;
        if tokens.is_empty() {
            return Err(Error::new(
                ErrorCode::Usage,
                "query has no searchable tokens",
            ));
        }
        let mut statement = self.connection.prepare(FIND).map_err(db_error)?;
        let mut find = |expression: &str, limit: usize| -> Result<Vec<SearchHit>> {
            statement
                .query_map(
                    params![expression, query.tag, query.folder, limit as i64],
                    |r| {
                        Ok(SearchHit {
                            path: r.get(0)?,
                            title: r.get(1)?,
                            snippet: r.get(2)?,
                            rank: r.get(3)?,
                        })
                    },
                )
                .map_err(db_error)?
                .collect::<rusqlite::Result<_>>()
                .map_err(db_error)
        };
        match query.mode {
            SearchMode::Phrase => find(&quoted(&tokens.join(" ")), query.limit),
            SearchMode::Literal => find(&complete(&tokens), query.limit),
            SearchMode::Prefix => find(&prefix_all(&tokens), query.limit),
            SearchMode::Smart => {
                // Complete words answer the query first; only slots still free
                // consider words that merely start with a query token. Tiers
                // keep BM25 scores of different expressions from being
                // compared as if they shared one relevance scale.
                let literal = complete(&tokens);
                let expanded = prefix_expanded(&tokens);
                let mut hits = find(&literal, query.limit)?;
                if hits.len() < query.limit && expanded != literal {
                    let seen: HashSet<String> = hits.iter().map(|h| h.path.clone()).collect();
                    for hit in find(&expanded, query.limit)? {
                        if hits.len() == query.limit {
                            break;
                        }
                        if !seen.contains(&hit.path) {
                            hits.push(hit);
                        }
                    }
                }
                Ok(hits)
            }
        }
    }
    /// Tokenize with the index tokenizer. Punctuation separates literal tokens
    /// rather than accidentally turning a-b into a phrase query.
    fn query_tokens(&self, text: &str) -> Result<Vec<String>> {
        self.connection.execute_batch("CREATE VIRTUAL TABLE IF NOT EXISTS temp.query_tokens USING fts5(text, tokenize='unicode61'); CREATE VIRTUAL TABLE IF NOT EXISTS temp.query_vocab USING fts5vocab(temp, query_tokens, instance); DELETE FROM temp.query_tokens;").map_err(db_error)?;
        self.connection
            .execute("INSERT INTO temp.query_tokens(text) VALUES(?1)", [text])
            .map_err(db_error)?;
        self.connection
            .prepare("SELECT term FROM temp.query_vocab ORDER BY offset")
            .map_err(db_error)?
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)
    }
}
impl Library {
    pub fn search_session(&self) -> Result<SearchSession> {
        let lock = self.lock()?;
        let (connection, status) = self.reconcile_locked(false, false)?;
        Ok(SearchSession {
            _lock: lock,
            connection,
            status,
        })
    }
    pub fn search(&self, query: &SearchQuery) -> Result<SearchReport> {
        query.validate()?;
        let session = self.search_session()?;
        Ok(SearchReport {
            hits: session.search(query)?,
            status: session.status,
        })
    }
}
