//! Round-trip property tests: cache-direct query results must match the
//! equivalent `filter_documents` results against a `load_graph_index()` graph.

use camino::Utf8PathBuf;
use tempfile::TempDir;
use vault_cache::{Cache, DocumentQuery};
use vault_core::DocumentSummary;

fn synth_vault() -> (TempDir, Utf8PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(
        root.join("note-a.md").as_std_path(),
        "---\ntype: note\nkind: log\n---\nbody a\n",
    )
    .unwrap();
    std::fs::write(
        root.join("note-b.md").as_std_path(),
        "---\ntype: note\nkind: meeting\n---\nbody b\n",
    )
    .unwrap();
    std::fs::write(
        root.join("workspace.md").as_std_path(),
        "---\ntype: workspace\n---\nbody w\n",
    )
    .unwrap();
    std::fs::write(
        root.join("untyped.md").as_std_path(),
        "no frontmatter at all\n",
    )
    .unwrap();
    (tmp, root)
}

fn populate_cache(root: &Utf8PathBuf) -> Cache {
    let mut cache = Cache::open(root).unwrap();
    cache.rebuild(root).unwrap();
    cache
}

fn paths(docs: &[DocumentSummary]) -> Vec<&str> {
    docs.iter().map(|d| d.path.as_str()).collect()
}

#[test]
fn empty_query_returns_every_document_in_path_order() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let result = cache.documents_matching(&DocumentQuery::default()).unwrap();

    assert_eq!(
        paths(&result),
        vec!["note-a.md", "note-b.md", "untyped.md", "workspace.md"]
    );
}

#[test]
fn frontmatter_eq_string_value() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        frontmatter_eq: vec![("type".to_string(), serde_json::json!("note"))],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["note-a.md", "note-b.md"]);
}

#[test]
fn frontmatter_eq_multiple_fields_all_must_match() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        frontmatter_eq: vec![
            ("type".to_string(), serde_json::json!("note")),
            ("kind".to_string(), serde_json::json!("log")),
        ],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["note-a.md"]);
}

#[test]
fn frontmatter_has_present_field() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        frontmatter_has: vec!["kind".to_string()],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["note-a.md", "note-b.md"]);
}

#[test]
fn frontmatter_missing_absent_field() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        frontmatter_missing: vec!["kind".to_string()],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["untyped.md", "workspace.md"]);
}

#[test]
fn path_globs_post_filter() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        path_globs: vec!["note-*.md".to_string()],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["note-a.md", "note-b.md"]);
}

#[test]
fn path_globs_combined_with_frontmatter() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let query = DocumentQuery {
        path_globs: vec!["note-*.md".to_string()],
        frontmatter_eq: vec![("kind".to_string(), serde_json::json!("meeting"))],
        ..Default::default()
    };
    let result = cache.documents_matching(&query).unwrap();

    assert_eq!(paths(&result), vec!["note-b.md"]);
}

#[test]
fn hyphenated_and_dotted_frontmatter_keys() {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(
        root.join("hyph.md").as_std_path(),
        "---\ncreated-at: 2026-01-01\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("dotted.md").as_std_path(),
        "---\nschema.version: 3\n---\n",
    )
    .unwrap();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let query = DocumentQuery {
        frontmatter_has: vec!["created-at".to_string()],
        ..Default::default()
    };
    assert_eq!(
        paths(&cache.documents_matching(&query).unwrap()),
        vec!["hyph.md"]
    );

    let query = DocumentQuery {
        frontmatter_has: vec!["schema.version".to_string()],
        ..Default::default()
    };
    assert_eq!(
        paths(&cache.documents_matching(&query).unwrap()),
        vec!["dotted.md"]
    );
}

#[test]
fn document_by_path_returns_full_document() {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(
        root.join("doc.md").as_std_path(),
        "---\ntype: note\n---\n\n# Heading\n\n^block-1\n\n[link](other.md)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("other.md").as_std_path(),
        "---\ntype: note\n---\nbody\n",
    )
    .unwrap();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let doc = cache
        .document_by_path(camino::Utf8Path::new("doc.md"))
        .unwrap();

    let doc = doc.expect("doc.md should be present");
    assert_eq!(doc.path.as_str(), "doc.md");
    assert!(doc.headings.iter().any(|h| h.text == "Heading"));
    assert!(doc.block_ids.iter().any(|b| b == "block-1"));
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].target, "other.md");
}

#[test]
fn document_by_path_missing_returns_none() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    let result = cache
        .document_by_path(camino::Utf8Path::new("nope.md"))
        .unwrap();

    assert!(result.is_none());
}

#[test]
fn files_returns_full_inventory_including_markdown() {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(root.join("doc.md").as_std_path(), "body\n").unwrap();
    std::fs::write(root.join("image.png").as_std_path(), b"png-bytes").unwrap();
    std::fs::write(root.join("notes.txt").as_std_path(), "plain\n").unwrap();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let files = cache.files().unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();

    // All vault files appear in `index.files` and therefore in cache.files() —
    // matches v1's `vault files` output. See graph_files_jsonl_contract test.
    assert!(paths.contains(&"image.png"));
    assert!(paths.contains(&"notes.txt"));
    assert!(paths.contains(&"doc.md"));
}

fn synth_link_vault() -> (TempDir, Utf8PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(
        root.join("a.md").as_std_path(),
        "---\n---\n[to b](b.md) [to nowhere](missing.md)\n",
    )
    .unwrap();
    std::fs::write(root.join("b.md").as_std_path(), "---\n---\n[to a](a.md)\n").unwrap();
    (tmp, root)
}

#[test]
fn links_returns_every_link_in_order() {
    let (_tmp, root) = synth_link_vault();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let links = cache.links().unwrap();

    assert_eq!(links.len(), 3);
    assert_eq!(links[0].source_path.as_str(), "a.md");
    assert_eq!(links[1].source_path.as_str(), "a.md");
    assert_eq!(links[2].source_path.as_str(), "b.md");
}

#[test]
fn links_unresolved_filters_by_status() {
    let (_tmp, root) = synth_link_vault();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let unresolved = cache.links_unresolved().unwrap();

    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].target, "missing.md");
}

#[test]
fn backlinks_to_returns_incoming_resolved_links() {
    let (_tmp, root) = synth_link_vault();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    let incoming = cache.backlinks_to(camino::Utf8Path::new("a.md")).unwrap();

    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source_path.as_str(), "b.md");
    assert_eq!(
        incoming[0].resolved_path.as_deref().map(|p| p.as_str()),
        Some("a.md")
    );
}

#[test]
fn has_diagnostic_errors_false_for_clean_vault() {
    let (_tmp, root) = synth_vault();
    let cache = populate_cache(&root);

    assert!(!cache.has_diagnostic_errors().unwrap());
}

#[test]
fn has_diagnostic_errors_true_when_read_error_present() {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    // Invalid UTF-8 bytes with a .md extension trip read_to_string,
    // which vault-frontmatter surfaces as a Severity::Error diagnostic
    // (code "read-failed").
    std::fs::write(
        root.join("bad-utf8.md").as_std_path(),
        b"\xff\xfe\xfd\xfc invalid utf-8 here",
    )
    .unwrap();
    let mut cache = Cache::open(&root).unwrap();
    cache.rebuild(&root).unwrap();

    assert!(cache.has_diagnostic_errors().unwrap());
}
