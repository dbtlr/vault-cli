//! Property test: any sequence of filesystem operations must produce the
//! same final cache state via incremental update as from-scratch rebuild.
//!
//! Catches invalidation bugs that scenario tests miss by running random
//! sequences of (Create, Modify, Delete) ops against two parallel vaults
//! and asserting the indices match.

use camino::Utf8PathBuf;
use tempfile::TempDir;

#[derive(Debug, Clone)]
enum Op {
    Create(String),
    Modify(String),
    Delete(String),
}

/// Builds an isolated vault rooted at `<tmpdir>/vault/`. `vault_graph` treats
/// directories whose basename starts with `.` as hidden, and `TempDir` itself
/// uses a `.tmp...` prefix — so we nest under a non-hidden subdirectory.
fn fresh_vault() -> (TempDir, Utf8PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    (tmp, root)
}

fn run_sequence(ops: &[Op]) {
    let (_tmp1, root1) = fresh_vault();
    let (_tmp2, root2) = fresh_vault();

    // Apply ops to both vaults identically.
    // root1 gets an incremental update after each op.
    // root2 only gets a single from-scratch rebuild at the end.
    for op in ops {
        apply_op(&root1, op);
        apply_op(&root2, op);
        let mut cache1 = vault_cache::Cache::open(&root1).unwrap();
        cache1
            .index_incremental(&root1, &Default::default())
            .unwrap();
    }

    let mut cache2 = vault_cache::Cache::open(&root2).unwrap();
    cache2.rebuild(&root2).unwrap();

    let cache1 = vault_cache::Cache::open(&root1).unwrap();
    let index1 = cache1.load_graph_index().unwrap();
    let index2 = cache2.load_graph_index().unwrap();

    assert_eq!(
        index1.documents.len(),
        index2.documents.len(),
        "doc count drift: {} (incremental) vs {} (from-scratch); ops: {:?}",
        index1.documents.len(),
        index2.documents.len(),
        ops,
    );

    let paths1: std::collections::BTreeSet<_> =
        index1.documents.iter().map(|d| d.path.clone()).collect();
    let paths2: std::collections::BTreeSet<_> =
        index2.documents.iter().map(|d| d.path.clone()).collect();
    assert_eq!(paths1, paths2, "path set drift; ops: {:?}", ops);

    let links1: usize = index1.documents.iter().map(|d| d.links.len()).sum();
    let links2: usize = index2.documents.iter().map(|d| d.links.len()).sum();
    assert_eq!(
        links1, links2,
        "link count drift: {links1} (incremental) vs {links2} (from-scratch); ops: {ops:?}",
    );
}

fn apply_op(root: &camino::Utf8Path, op: &Op) {
    match op {
        Op::Create(name) => {
            std::fs::write(
                root.join(format!("{name}.md")).as_std_path(),
                format!("---\ntitle: {name}\n---\nbody [link]({name}-target.md)\n"),
            )
            .unwrap();
        }
        Op::Modify(name) => {
            std::fs::write(
                root.join(format!("{name}.md")).as_std_path(),
                format!("---\ntitle: {name}\n---\nupdated body\n"),
            )
            .unwrap();
        }
        Op::Delete(name) => {
            let _ = std::fs::remove_file(root.join(format!("{name}.md")).as_std_path());
        }
    }
}

#[test]
fn incremental_matches_from_scratch_simple() {
    run_sequence(&[
        Op::Create("a".into()),
        Op::Create("b".into()),
        Op::Modify("a".into()),
        Op::Delete("b".into()),
    ]);
}

#[test]
fn incremental_matches_from_scratch_create_delete_create() {
    run_sequence(&[
        Op::Create("foo".into()),
        Op::Delete("foo".into()),
        Op::Create("foo".into()),
    ]);
}

#[test]
fn incremental_matches_from_scratch_many_creates() {
    let ops: Vec<Op> = (0..20).map(|i| Op::Create(format!("doc{i}"))).collect();
    run_sequence(&ops);
}

#[test]
fn incremental_matches_from_scratch_interleaved() {
    let mut ops = Vec::new();
    for i in 0..10 {
        ops.push(Op::Create(format!("doc{i}")));
        if i % 2 == 0 {
            ops.push(Op::Modify(format!("doc{i}")));
        }
        if i % 3 == 0 && i > 0 {
            ops.push(Op::Delete(format!("doc{}", i - 1)));
        }
    }
    run_sequence(&ops);
}
