//! Integration tests for the Phase 3 LIVE EXAMPLES block on `vault find --help`.
//!
//! Drives the real `vault` binary against on-disk fixture vaults so the full
//! interceptor path (arg parse → cwd resolve → cache open → generator →
//! renderer) is exercised. The existing test convention uses
//! `env!("CARGO_BIN_EXE_vault")` to locate the binary; this file follows
//! the same pattern.

use std::process::Command;

use camino::Utf8PathBuf;
use rusqlite::params;
use tempfile::TempDir;
use vault_cache::Cache;

fn vault_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vault"))
}

/// Build a small fixture vault on disk: empty `.vault/` so it's recognized
/// as a vault root, plus a cache pre-seeded with documents whose top-level
/// frontmatter matches the Phase 3 algorithm.
fn fixture_vault() -> TempDir {
    let tmp = tempfile::Builder::new()
        .prefix("vault-cli-help-live-integ-")
        .tempdir()
        .unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    std::fs::create_dir_all(root.join(".vault").as_std_path()).unwrap();
    let cache = Cache::open(&root).unwrap();
    let docs: &[(&str, &str)] = &[
        (
            "a.md",
            r#"{"type":"note","workspace":"vault-cli","modified":"2026-05-21"}"#,
        ),
        (
            "b.md",
            r#"{"type":"note","workspace":"vault-cli","modified":"2026-05-20"}"#,
        ),
        (
            "c.md",
            r#"{"type":"note","workspace":"vault-cli","modified":"2026-05-19"}"#,
        ),
        (
            "d.md",
            r#"{"type":"task","workspace":"vault-cli","modified":"2026-05-18"}"#,
        ),
        (
            "e.md",
            r#"{"type":"task","workspace":"atlas","modified":"2026-05-17"}"#,
        ),
    ];
    for (path, fm) in docs {
        cache
            .conn()
            .execute(
                "INSERT INTO documents (path, stem, hash, frontmatter_json, body_text, mtime_ns, size_bytes) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![path, path.trim_end_matches(".md"), "h", fm, "", 0i64, 0i64],
            )
            .unwrap();
    }
    drop(cache);
    tmp
}

#[test]
fn long_help_inside_vault_emits_live_examples_block() {
    let vault = fixture_vault();
    let out = vault_bin()
        .env("NO_COLOR", "1")
        .env("PAGER", "cat")
        .args(["--cwd", vault.path().to_str().unwrap(), "find", "--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("LIVE EXAMPLES"),
        "expected LIVE EXAMPLES; got:\n{stdout}"
    );
    // The find_live composer ranks enum-like fields by
    // `(docs_with_field/total) * (top_value/docs_with_field)` =
    // `top_value_doc_count / total_documents`. For this fixture:
    //   workspace: top_value=4 ("vault-cli") → 4/5 = 0.8
    //   type:      top_value=3 ("note")      → 3/5 = 0.6
    // So workspace wins P1, type wins P2.
    assert!(
        stdout.contains(
            "vault find --eq workspace:vault-cli --eq type:note --sort modified --limit 5"
        ),
        "expected composed query; got:\n{stdout}"
    );
    assert!(
        stdout.contains("3 documents match"),
        "expected match count; got:\n{stdout}"
    );
}

#[test]
fn long_help_deterministic_across_runs() {
    let vault = fixture_vault();
    let run = || {
        let out = vault_bin()
            .env("NO_COLOR", "1")
            .env("PAGER", "cat")
            .args(["--cwd", vault.path().to_str().unwrap(), "find", "--help"])
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    let a = run();
    let b = run();
    assert_eq!(
        a, b,
        "consecutive --help invocations must produce identical output"
    );
}

#[test]
fn long_help_outside_vault_has_no_live_examples() {
    let tmp = tempfile::Builder::new()
        .prefix("vault-cli-help-no-vault-")
        .tempdir()
        .unwrap();
    let out = vault_bin()
        .env("NO_COLOR", "1")
        .env("PAGER", "cat")
        .args(["--cwd", tmp.path().to_str().unwrap(), "find", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("LIVE EXAMPLES"),
        "no-vault path must omit LIVE EXAMPLES; got:\n{stdout}"
    );
}

#[test]
fn short_help_never_emits_live_examples() {
    let vault = fixture_vault();
    let out = vault_bin()
        .env("NO_COLOR", "1")
        .args(["--cwd", vault.path().to_str().unwrap(), "find", "-h"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("LIVE EXAMPLES"),
        "short form must omit LIVE EXAMPLES; got:\n{stdout}"
    );
}

#[test]
fn long_help_ascii_marker_under_norn_ascii() {
    let vault = fixture_vault();
    let out = vault_bin()
        .env("NO_COLOR", "1")
        .env("NORN_ASCII", "1")
        .env("PAGER", "cat")
        .args(["--cwd", vault.path().to_str().unwrap(), "find", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("> vault find"),
        "expected '> vault find' under NORN_ASCII; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("▸ vault find"),
        "must not emit UTF marker under NORN_ASCII; got:\n{stdout}"
    );
}

#[test]
fn long_help_no_color_includes_live_tag() {
    let vault = fixture_vault();
    let out = vault_bin()
        .env("NO_COLOR", "1")
        .env("PAGER", "cat")
        .args(["--cwd", vault.path().to_str().unwrap(), "find", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("3 documents match (live)"),
        "expected '(live)' suffix under NO_COLOR; got:\n{stdout}"
    );
}
