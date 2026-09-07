use notes_core::{Document, NoteId};

const ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

#[test]
fn repository_fixture_is_preserved_by_adoption_and_body_update() {
    let source = include_str!("fixtures/preservation.md");
    let parsed = Document::parse(source.as_bytes(), "fallback").unwrap();
    assert_eq!(parsed.title, "Actual title");
    let adopted = parsed.with_id(&NoteId::parse(ID).unwrap());
    assert_eq!(adopted.replacen(&format!("id: {ID}\n"), "", 1), source);
    let document = Document::parse(adopted.as_bytes(), "fallback").unwrap();
    let updated = document.with_body("replacement");
    assert_eq!(
        updated,
        format!(
            "{}replacement",
            adopted.strip_suffix(&document.body).unwrap()
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
        assert_eq!(parsed.id, doc.id);
    }
}

#[test]
fn invalid_metadata_and_resource_limits_are_errors() {
    for yaml in [
        "custom: \"multiline\ntags: [not metadata]\nend\"",
        "id: null",
        "tags: scalar",
        "tags: [1]",
        "key: 1\nkey: 2",
        "custom:\n  key: 1\n  'key': 2",
        "tags: &shared [one]\ncustom: *shared",
        "id: 80000000000000000000000000",
        "id: 01ARZ3NDEKTSV4RRFFQ69G5FAI",
    ] {
        assert!(
            Document::parse(format!("---\n{yaml}\n---\nbody").as_bytes(), "x").is_err(),
            "{yaml}"
        );
    }
    let deep = format!("---\nx: {}0{}\n---\n", "[".repeat(100), "]".repeat(100));
    assert!(Document::parse(deep.as_bytes(), "x").is_err());
    for bad in [
        "",
        "short",
        " 01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "01arz3ndektsv4rrffq69g5fav",
    ] {
        assert!(NoteId::parse(bad).is_err());
    }
}

#[test]
fn lossless_adoption_and_owned_field_patches() {
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
                let adopted = doc.with_id(&NoteId::parse(ID).unwrap());
                let doc = Document::parse(adopted.as_bytes(), "fallback").unwrap();
                assert_eq!(doc.body, body);
                assert!(adopted.starts_with(bom));
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
