use notes_core::Document;

const ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

#[test]
fn repository_fixture_is_preserved_by_parse_and_body_update() {
    let source = include_str!("fixtures/preservation.md");
    let parsed = Document::parse(source.as_bytes(), "fallback").unwrap();
    assert_eq!(parsed.title, "Actual title");
    let document = parsed;
    let updated = document.with_body("replacement");
    assert_eq!(
        updated,
        format!(
            "{}replacement",
            source.strip_suffix(&document.body).unwrap()
        )
    );
}

#[test]
fn body_update_after_eof_delimiter_stays_parseable() {
    for close in ["---", "..."] {
        let source = format!("---\nid: {ID}\n{close}");
        let doc = Document::parse(source.as_bytes(), "fallback").unwrap();
        let updated = doc.with_body("hello");
        let parsed = Document::parse(updated.as_bytes(), "fallback").unwrap();
        assert_eq!(parsed.body, "hello");
        assert!(updated.starts_with(&source));
    }
}

#[test]
fn invalid_metadata_and_resource_limits_are_errors() {
    for yaml in [
        "custom: \"multiline\ntags: [not metadata]\nend\"",
        "tags: scalar",
        "tags: [1]",
        "key: 1\nkey: 2",
        "custom:\n  key: 1\n  'key': 2",
        "tags: &shared [one]\ncustom: *shared",
    ] {
        assert!(
            Document::parse(format!("---\n{yaml}\n---\nbody").as_bytes(), "x").is_err(),
            "{yaml}"
        );
    }
    let deep = format!("---\nx: {}0{}\n---\n", "[".repeat(100), "]".repeat(100));
    assert!(Document::parse(deep.as_bytes(), "x").is_err());
}

#[test]
fn lossless_owned_field_patches() {
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            for tags in [
                "tags: [one, 'a*b'] # keep inline\n",
                "tags:\n- one # keep inline\n- 'a*b'\n",
            ] {
                let metadata = format!("---\n# retain\ncustom:\n  nested: true\n{tags}---\n")
                    .replace('\n', newline);
                let body = "```\n# Not a heading\n---\n```\n# Real title\n[[wiki]] ![image](x)"
                    .replace('\n', newline);
                let source = format!("{bom}{metadata}{body}");
                let doc = Document::parse(source.as_bytes(), "fallback").unwrap();
                assert_eq!(doc.title, "Real title");
                assert_eq!(doc.body, body);
                assert!(doc.source.starts_with(bom));
                let changed = doc.with_tags(&["new".into()]);
                assert!(changed.contains("# keep inline"));
                assert!(changed.contains(&format!("custom:{newline}  nested: true{newline}")));
                let changed = Document::parse(changed.as_bytes(), "fallback").unwrap();
                assert_eq!(changed.body, body);
                assert_eq!(changed.tags, ["new"]);
            }
        }
    }
}

#[test]
fn id_is_unowned_metadata_of_any_supported_yaml_type() {
    for value in [
        "null",
        "42",
        "short",
        "'lower-case arbitrary'",
        "[one, two]",
        "\n  nested: true",
    ] {
        let source = format!("---\nid: {value}\ncustom: retain\n---\n# Body");
        let doc = Document::parse(source.as_bytes(), "fallback").unwrap();
        assert_eq!(doc.source, source);
        let tagged = doc.with_tags(&["work".into()]);
        assert!(tagged.contains(&format!("id: {value}\ncustom: retain")));
        let updated = doc.with_body("replacement");
        assert!(updated.contains(&format!("id: {value}\ncustom: retain")));
        assert!(serde_json::to_value(&doc).unwrap().get("id").is_none());
    }
    assert_eq!(
        Document::parse(b"plain Markdown", "fallback")
            .unwrap()
            .source,
        "plain Markdown"
    );
}
