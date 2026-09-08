use crate::ErrorCode;
use crate::{Error, Result, revision};
use serde::Serialize;
use serde_yaml::Value;
use std::ops::Range;

fn field_comments(source: &str, newline: &str) -> String {
    use yaml_rust2::scanner::{Scanner, TScalarStyle, TokenType};
    let quoted: std::collections::HashSet<usize> = Scanner::new(source.chars())
        .filter_map(|token| match token.1 {
            TokenType::Scalar(TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted, _) => {
                Some(token.0.index())
            }
            _ => None,
        })
        .collect();
    let chars: Vec<_> = source.chars().collect();
    let mut quote = None;
    let mut i = 0;
    let mut comments = String::new();
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if q == '"' && c == '\\' {
                i += 2;
                continue;
            }
            if c == q {
                if q == '\'' && chars.get(i + 1) == Some(&q) {
                    i += 2;
                    continue;
                }
                quote = None;
            }
        } else if quoted.contains(&i) && matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if c == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            while i < chars.len() && !matches!(chars[i], '\r' | '\n') {
                comments.push(chars[i]);
                i += 1;
            }
            comments.push_str(newline);
            continue;
        }
        i += 1;
    }
    comments
}
#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub tags: Vec<String>,
    pub body: String,
    pub source: String,
    pub revision: crate::Revision,
    pub title: String,
    #[serde(skip)]
    header: Option<Range<usize>>,
    #[serde(skip)]
    fields: Vec<(String, Range<usize>)>,
    #[serde(skip)]
    body_start: usize,
    #[serde(skip)]
    newline: String,
}
impl Document {
    pub fn parse(bytes: &[u8], fallback: &str) -> Result<Self> {
        if bytes.len() > crate::filesystem::MAX_NOTE_BYTES {
            return Err(Error::new(ErrorCode::Unsupported, "note exceeds 16 MiB"));
        }
        let source = std::str::from_utf8(bytes)
            .map_err(|_| Error::new(ErrorCode::Encoding, "note is not UTF-8"))?
            .to_string();
        let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
        let newline = if source[bom..].contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
        .to_string();
        let mut header = None;
        let mut body_start = bom;
        let mut fields = Vec::new();
        let mut tags = Vec::new();
        let first = source[bom..].split_inclusive('\n').next().unwrap_or("");
        if first.trim_end_matches(['\r', '\n']) == "---" {
            let start = bom + first.len();
            let mut pos = start;
            let mut end = None;
            for line in source[start..].split_inclusive('\n') {
                let text = line.trim_end_matches(['\r', '\n']);
                if text == "---" || text == "..." {
                    end = Some(pos);
                    body_start = pos + line.len();
                    break;
                }
                pos += line.len();
            }
            let end =
                end.ok_or_else(|| Error::new(ErrorCode::Metadata, "unterminated frontmatter"))?;
            let raw = &source[start..end];
            if raw.len() > 262144
                || raw.lines().any(|l| l.len() - l.trim_start().len() > 64)
                || raw.lines().any(|l| l.trim_start().starts_with("<<:"))
            {
                return Err(Error::new(
                    ErrorCode::Metadata,
                    "unsupported YAML aliases, tags, merge or resource limit",
                ));
            }
            if raw.trim_start().starts_with(['{', '[']) {
                return Err(Error::new(ErrorCode::Metadata, "flow root is unsupported"));
            }
            use yaml_rust2::scanner::{Scanner, TokenType};
            let mut scanner = Scanner::new(raw.chars());
            let mut depth = 0usize;
            for token in scanner.by_ref() {
                match token.1 {
                    TokenType::Alias(_)
                    | TokenType::Anchor(_)
                    | TokenType::Tag(_, _)
                    | TokenType::TagDirective(_, _)
                    | TokenType::FlowMappingStart => {
                        return Err(Error::new(
                            ErrorCode::Metadata,
                            "aliases, tags and flow mappings unsupported",
                        ));
                    }
                    TokenType::BlockMappingStart
                    | TokenType::BlockSequenceStart
                    | TokenType::FlowSequenceStart => {
                        depth += 1;
                        if depth > 32 {
                            return Err(Error::new(ErrorCode::Metadata, "YAML nesting limit"));
                        }
                    }
                    TokenType::BlockEnd | TokenType::FlowSequenceEnd => {
                        depth = depth.saturating_sub(1)
                    }
                    _ => {}
                }
            }
            if scanner.get_error().is_some() {
                return Err(Error::new(ErrorCode::Metadata, "invalid YAML tokens"));
            }
            let value: Value = serde_yaml::from_str(raw)
                .map_err(|_| Error::new(ErrorCode::Metadata, "invalid YAML or duplicate key"))?;
            let empty = serde_yaml::Mapping::new();
            let map = if raw.trim().is_empty()
                || raw
                    .lines()
                    .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
            {
                &empty
            } else {
                value.as_mapping().ok_or_else(|| {
                    Error::new(ErrorCode::Metadata, "frontmatter must be a mapping")
                })?
            };
            if let Some(v) = map.get(Value::String("tags".into())) {
                for t in v.as_sequence().ok_or_else(|| {
                    Error::new(ErrorCode::Metadata, "tags must be string sequence")
                })? {
                    let s = t
                        .as_str()
                        .ok_or_else(|| Error::new(ErrorCode::Metadata, "tag must be string"))?
                        .to_string();
                    if !tags.contains(&s) {
                        tags.push(s);
                    }
                }
            }
            let mut starts = Vec::new();
            pos = start;
            for line in raw.split_inclusive('\n') {
                if !line.starts_with(char::is_whitespace)
                    && !line.starts_with('#')
                    && !line.starts_with("- ")
                    && line.trim() != "-"
                    && !line.trim().is_empty()
                {
                    let key = line
                        .split_once(':')
                        .ok_or_else(|| {
                            Error::new(ErrorCode::Metadata, "unsupported top-level YAML syntax")
                        })?
                        .0
                        .trim();
                    if key.starts_with(['\'', '"', '?']) {
                        return Err(Error::new(
                            ErrorCode::Metadata,
                            "quoted/complex top-level keys unsupported",
                        ));
                    }
                    starts.push((key.to_string(), pos));
                }
                pos += line.len();
            }
            // Line spans are supported only when they correspond one-to-one
            // with the parsed root mapping. A quoted multiline value can contain
            // apparent column-zero keys; never patch such text as metadata.
            let keys: std::collections::HashSet<_> = starts.iter().map(|(key, _)| key).collect();
            if keys.len() != starts.len()
                || starts.len() != map.len()
                || keys
                    .iter()
                    .any(|key| !map.contains_key(Value::String((*key).clone())))
            {
                return Err(Error::new(
                    ErrorCode::Metadata,
                    "root mapping cannot be patched losslessly",
                ));
            }
            for (i, (key, s)) in starts.iter().enumerate() {
                fields.push((key.clone(), *s..starts.get(i + 1).map_or(end, |x| x.1)));
            }
            header = Some(start..end);
        }
        let body = source[body_start..].to_string();
        let mut title = String::new();
        let mut in_h1 = false;
        for event in pulldown_cmark::Parser::new(&body) {
            use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};
            match event {
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    ..
                }) => in_h1 = true,
                Event::End(TagEnd::Heading(HeadingLevel::H1)) if in_h1 => break,
                Event::Text(t) | Event::Code(t) if in_h1 => title.push_str(&t),
                _ => {}
            }
        }
        if title.is_empty() {
            title = fallback.into();
        }
        Ok(Self {
            tags,
            body,
            revision: revision(bytes),
            source,
            title,
            header,
            fields,
            body_start,
            newline,
        })
    }
    pub fn with_tags(&self, tags: &[String]) -> String {
        self.patch(
            "tags",
            &serde_json::to_string(tags).expect("strings serialize"),
        )
    }
    fn patch(&self, key: &str, value: &str) -> String {
        let nl = &self.newline;
        let line = format!("{key}: {value}{nl}");
        let mut out = self.source.clone();
        if let Some((_, range)) = self.fields.iter().find(|(k, _)| k == key) {
            // Preserve comments, including inline sequence comments, without
            // interpreting a # inside a quoted tag as a YAML comment.
            let comments = field_comments(&self.source[range.clone()], nl);
            out.replace_range(range.clone(), &(line + &comments));
        } else if let Some(range) = &self.header {
            out.insert_str(range.start, &line);
        } else {
            let bom = if out.starts_with('\u{feff}') { 3 } else { 0 };
            out.insert_str(bom, &format!("---{nl}{line}---{nl}"));
        }
        out
    }
    pub fn with_body(&self, body: &str) -> String {
        let prefix = &self.source[..self.body_start];
        let separator = if self.header.is_some() && !prefix.ends_with('\n') && !body.is_empty() {
            self.newline.as_str()
        } else {
            ""
        };
        format!("{prefix}{separator}{body}")
    }
}
