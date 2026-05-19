//! Performance regression test. Locks in the documented cold-rebuild target:
//! a 1000-document vault should rebuild from scratch in under 2 seconds.
//!
//! Marked `#[ignore]` so it does not run on every `cargo test` invocation.
//! Opt in via `cargo test --ignored` or in CI when locking targets.

use camino::Utf8PathBuf;
use tempfile::TempDir;

#[test]
#[ignore]
fn cold_rebuild_under_2s_on_1k_docs() {
    let tmp = TempDir::new().unwrap();
    // Nest under `vault/` so the basename is not hidden — TempDir uses
    // `.tmp...` which `vault_graph` skips.
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    for i in 0..1000 {
        std::fs::write(
            root.join(format!("doc{i}.md")).as_std_path(),
            format!("---\ntitle: Doc {i}\n---\nbody\n"),
        )
        .unwrap();
    }
    let mut cache = vault_cache::Cache::open(&root).unwrap();
    let start = std::time::Instant::now();
    cache.rebuild(&root).unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 2000,
        "cold rebuild took {}ms (target: < 2000ms)",
        elapsed.as_millis(),
    );
}
