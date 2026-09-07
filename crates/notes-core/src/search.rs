//! Safe FTS expressions and parameterized filters. Snippets are plain text, not HTML.
use crate::{
    Error, ErrorCode, Library, Result,
    index::{IndexStatus, db_error},
};
use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default)]
pub enum SearchMode {
    #[default]
    Literal,
    Phrase,
    Prefix,
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
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub id: String,
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
        // Use the same tokenizer as the index. Punctuation separates literal
        // tokens rather than accidentally turning a-b into a phrase query.
        self.connection.execute_batch("CREATE VIRTUAL TABLE IF NOT EXISTS temp.query_tokens USING fts5(text, tokenize='unicode61'); CREATE VIRTUAL TABLE IF NOT EXISTS temp.query_vocab USING fts5vocab(temp, query_tokens, instance); DELETE FROM temp.query_tokens;").map_err(db_error)?;
        self.connection
            .execute(
                "INSERT INTO temp.query_tokens(text) VALUES(?1)",
                [&query.text],
            )
            .map_err(db_error)?;
        let tokens: Vec<String> = self
            .connection
            .prepare("SELECT term FROM temp.query_vocab ORDER BY offset")
            .map_err(db_error)?
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)?;
        if tokens.is_empty() {
            return Err(Error::new(
                ErrorCode::Usage,
                "query has no searchable tokens",
            ));
        }
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let expression = match query.mode {
            SearchMode::Phrase => quote(&tokens.join(" ")),
            SearchMode::Literal | SearchMode::Prefix => tokens
                .iter()
                .map(|t| {
                    format!(
                        "{}{}",
                        quote(t),
                        if matches!(query.mode, SearchMode::Prefix) {
                            "*"
                        } else {
                            ""
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND "),
        };
        let mut stmt = self.connection.prepare(
            "SELECT n.id,n.path,n.title,snippet(notes_fts,-1,'','','…',24),bm25(notes_fts,0,10,1,2,2)
             FROM notes_fts JOIN notes n ON n.id=notes_fts.note_id
             WHERE notes_fts MATCH ?1
             AND (?2 IS NULL OR EXISTS(SELECT 1 FROM tags t WHERE t.note_id=n.id AND t.tag=?2 COLLATE BINARY))
             AND (?3 IS NULL OR substr(n.path,1,length(?3)+1)=?3||'/')
             ORDER BY bm25(notes_fts,0,10,1,2,2),n.path COLLATE BINARY LIMIT ?4"
        ).map_err(db_error)?;
        stmt.query_map(
            params![expression, query.tag, query.folder, query.limit as i64],
            |r| {
                Ok(SearchHit {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    title: r.get(2)?,
                    snippet: r.get(3)?,
                    rank: r.get(4)?,
                })
            },
        )
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
