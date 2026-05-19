//! Concurrency integration test: two simultaneous `rebuild` calls must
//! both complete successfully, with the second one serializing behind
//! the first via the advisory write lock.

use camino::Utf8PathBuf;
use tempfile::TempDir;

#[test]
fn two_simultaneous_rebuilds_serialize() {
    let tmp = TempDir::new().unwrap();
    // vault_graph treats hidden directories (basename starts with `.`) as
    // skipped — TempDir's own basename starts with `.tmp`, so nest the
    // vault under a non-hidden subdirectory.
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
        .unwrap()
        .join("vault");
    std::fs::create_dir(root.as_std_path()).unwrap();
    std::fs::write(
        root.join("a.md").as_std_path(),
        "---\ntitle: A\n---\nbody\n",
    )
    .unwrap();
    std::fs::write(
        root.join("b.md").as_std_path(),
        "---\ntitle: B\n---\nbody [[a]]\n",
    )
    .unwrap();

    let root1 = root.clone();
    let handle1 = std::thread::spawn(move || {
        let mut cache = vault_cache::Cache::open(&root1).unwrap();
        cache.rebuild(&root1)
    });

    // Tiny stagger so handle1 has reached `rebuild` and acquired the lock
    // before handle2 races for it. Without this the test still asserts both
    // succeed, but with the stagger we exercise the "second writer waits"
    // path deterministically.
    std::thread::sleep(std::time::Duration::from_millis(10));

    let root2 = root.clone();
    let handle2 = std::thread::spawn(move || {
        let mut cache = vault_cache::Cache::open(&root2).unwrap();
        cache.rebuild(&root2)
    });

    let r1 = handle1.join().unwrap();
    let r2 = handle2.join().unwrap();
    assert!(r1.is_ok(), "first rebuild failed: {r1:?}");
    assert!(r2.is_ok(), "second rebuild failed: {r2:?}");
}
