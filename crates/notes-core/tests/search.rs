use notes_core::{
    ErrorCode, Library,
    search::{SearchMode, SearchQuery},
};

#[test]
fn fields_modes_exact_filters_ranking_and_hostile_text() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    lib.create(
        "foo/alpha.md",
        "# Zebra\nquick brown fox café 日本語 <script>alert(1)</script>",
        &["Case".into(), "tagword".into()],
    )
    .unwrap();
    lib.create(
        "foobar/beta.md",
        "# Other\nZebra quick fox brown",
        &["case".into()],
    )
    .unwrap();
    lib.create("foo/tie-a.md", "# Equal\nidentical", &[])
        .unwrap();
    lib.create("foo/tie-b.md", "# Equal\nidentical", &[])
        .unwrap();
    lib.create("f%_/literal.md", "wildcards", &[]).unwrap();
    let session = lib.search_session().unwrap();
    for term in ["Zebra", "café", "日本語", "alpha", "tagword"] {
        assert_eq!(
            session.search(&SearchQuery::literal(term)).unwrap()[0].path,
            "foo/alpha.md",
            "{term}"
        );
    }
    let mut q = SearchQuery::literal("quick brown");
    assert_eq!(session.search(&q).unwrap().len(), 2);
    assert_eq!(
        session
            .search(&SearchQuery::literal("quick-brown"))
            .unwrap()
            .len(),
        2
    );
    q.mode = SearchMode::Phrase;
    assert_eq!(session.search(&q).unwrap().len(), 1);
    q = SearchQuery::literal("qui");
    assert!(session.search(&q).unwrap().is_empty());
    q.mode = SearchMode::Prefix;
    assert_eq!(session.search(&q).unwrap().len(), 2);
    q = SearchQuery::literal("quick");
    q.tag = Some("Case".into());
    assert_eq!(session.search(&q).unwrap()[0].path, "foo/alpha.md");
    q.tag = Some("case".into());
    assert_eq!(session.search(&q).unwrap()[0].path, "foobar/beta.md");
    q.tag = None;
    q.folder = Some("foo".into());
    assert_eq!(session.search(&q).unwrap().len(), 1);
    q = SearchQuery::literal("wildcards");
    q.folder = Some("f%_".into());
    assert_eq!(session.search(&q).unwrap().len(), 1);
    assert_eq!(
        session
            .search(&SearchQuery::literal("identical"))
            .unwrap()
            .iter()
            .map(|h| h.path.as_str())
            .collect::<Vec<_>>(),
        ["foo/tie-a.md", "foo/tie-b.md"]
    );
    for text in [
        "OR",
        "NOT",
        "title:Zebra",
        "\"quick",
        "(quick)",
        "quick OR nonexistent",
        "'; DROP TABLE notes; --",
    ] {
        session.search(&SearchQuery::literal(text)).unwrap();
    }
    assert!(
        session
            .search(&SearchQuery::literal("quick OR nonexistent"))
            .unwrap()
            .is_empty()
    );
    for text in ["", "   ", "***", "\0quick"] {
        assert_eq!(
            session
                .search(&SearchQuery::literal(text))
                .unwrap_err()
                .code,
            ErrorCode::Usage
        );
    }
    assert!(
        session.search(&SearchQuery::literal("script")).unwrap()[0]
            .snippet
            .contains("<script>")
    );
}

#[test]
fn smart_mode_matches_word_prefixes_and_ranks_titles_above_bodies() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    // Only `exact.md` contains the complete word; the expanded tier must find
    // the notes that merely start with it.
    lib.create("exact.md", "# Tasks\nmemo", &["work".into()])
        .unwrap();
    lib.create(
        "title_only.md",
        "# Memory\nA reminder about the workshop",
        &["work".into()],
    )
    .unwrap();
    lib.create(
        "journal/body_only.md",
        "# Reminder\nmy memory of the trip",
        &["personal".into()],
    )
    .unwrap();
    // Matches both tiers, so it must appear exactly once.
    lib.create("both.md", "# Memory\nmemo and memory", &["work".into()])
        .unwrap();
    lib.create("plan.md", "# Plan\nproject budget review", &[])
        .unwrap();
    lib.create("single.md", "# Note\nzebra crossing", &[])
        .unwrap();
    let session = lib.search_session().unwrap();
    let paths = |text: &str, mode: SearchMode| {
        let mut query = SearchQuery::literal(text);
        query.mode = mode;
        session
            .search(&query)
            .unwrap()
            .iter()
            .map(|hit| hit.path.clone())
            .collect::<Vec<_>>()
    };
    // Literal mode is unchanged: no partial word reaches `Memory`.
    assert_eq!(paths("memo", SearchMode::Literal), ["exact.md", "both.md"]);
    // A complete word still outranks the same word found only in a title, and
    // a title-only hit outranks a body-only hit for the same word.
    assert_eq!(
        paths("memory", SearchMode::Literal),
        ["both.md", "title_only.md", "journal/body_only.md"]
    );
    let hits = paths("memo", SearchMode::Smart);
    assert_eq!(
        hits.len(),
        4,
        "expected the exact tier plus both prefix matches: {hits:?}"
    );
    let position = |path: &str| {
        hits.iter()
            .position(|hit| hit == path)
            .unwrap_or_else(|| panic!("{path} missing from {hits:?}"))
    };
    // Complete words answer first; the expanded tier only fills what is left.
    assert!(position("exact.md") < position("title_only.md"));
    assert!(position("both.md") < position("title_only.md"));
    assert!(position("both.md") < position("journal/body_only.md"));
    // Equal prefix matches rank the title above the Markdown body.
    assert!(position("title_only.md") < position("journal/body_only.md"));
    assert_eq!(
        paths("MEMO", SearchMode::Smart),
        hits,
        "query case must not change matching"
    );
    // Every query token expands, and filters stay exact in both tiers.
    assert_eq!(paths("proj bud", SearchMode::Smart), ["plan.md"]);
    let mut filtered = SearchQuery::smart("memo");
    filtered.tag = Some("personal".into());
    let tagged = session.search(&filtered).unwrap();
    assert_eq!(
        tagged.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
        ["journal/body_only.md"]
    );
    filtered.tag = None;
    filtered.folder = Some("journal".into());
    assert_eq!(
        session
            .search(&filtered)
            .unwrap()
            .iter()
            .map(|h| h.path.as_str())
            .collect::<Vec<_>>(),
        ["journal/body_only.md"]
    );
    // The result limit fills from the complete tier before the expanded one.
    let mut limited = SearchQuery::smart("memo");
    limited.limit = 2;
    let capped = session.search(&limited).unwrap();
    assert_eq!(capped.len(), 2);
    assert!(
        capped
            .iter()
            .all(|hit| hit.path == "exact.md" || hit.path == "both.md"),
        "limit must not pull expanded matches ahead of complete words"
    );
    // Interior substrings are not matches: `get` must not find `budget`.
    assert!(paths("get", SearchMode::Smart).is_empty());
    assert!(paths("udget", SearchMode::Smart).is_empty());
    // One-character tokens stay exact instead of matching everything.
    assert!(paths("z", SearchMode::Smart).is_empty());
    assert_eq!(paths("ze", SearchMode::Smart), ["single.md"]);
    // Hostile and malformed text stays a usage error, never FTS syntax.
    for text in ["", "   ", "***", "\0quick"] {
        assert_eq!(
            session.search(&SearchQuery::smart(text)).unwrap_err().code,
            ErrorCode::Usage
        );
    }
}
