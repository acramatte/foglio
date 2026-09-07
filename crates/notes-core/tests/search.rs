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
