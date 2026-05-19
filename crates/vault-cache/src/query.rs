//! Predicate set for `Cache::documents_matching` and the JSON-path encoder
//! that translates frontmatter field names into safe SQLite JSON paths.

use serde_json::Value;

/// Predicate set for `Cache::documents_matching`. ANY-of within `path_globs`;
/// ALL-of across the frontmatter vectors and between vectors.
///
/// Mirrors `vault_cli::filter::DocumentFilterOptions` semantics. The CLI
/// supplies the conversion via `impl From<&DocumentFilterOptions>` in a
/// later task.
#[derive(Default, Debug, Clone)]
pub struct DocumentQuery {
    /// Path glob patterns in `vault_graph::pattern_matches_path` syntax.
    /// ANY-of. Empty = no path narrowing. Applied as a Rust post-pass.
    pub path_globs: Vec<String>,
    /// Frontmatter equality predicates `(field, value)`. ALL-of.
    pub frontmatter_eq: Vec<(String, Value)>,
    /// Required-present fields. ALL-of. Exact null-vs-missing semantics
    /// match v1's `filter_documents` `--has` behavior — verified via
    /// round-trip property tests.
    pub frontmatter_has: Vec<String>,
    /// Required-absent fields. ALL-of. Exact null-vs-missing semantics
    /// match v1's `filter_documents` `--missing` behavior.
    pub frontmatter_missing: Vec<String>,
}

/// Encode a frontmatter field name as a single quoted JSON-path segment for
/// SQLite's `json_extract`. Returns the full path string `$."<escaped>"`.
///
/// SQLite parses the path at statement execution; binding this as a parameter
/// (not interpolating it) is what closes the SQL-injection vector and lets
/// frontmatter keys contain any character.
pub fn json_path_for(field: &str) -> String {
    let escaped = field.replace('\\', r"\\").replace('"', r#"\""#);
    format!(r#"$."{}""#, escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_field() {
        assert_eq!(json_path_for("type"), r#"$."type""#);
    }

    #[test]
    fn hyphenated_field() {
        assert_eq!(json_path_for("created-at"), r#"$."created-at""#);
    }

    #[test]
    fn dotted_field() {
        // Keys with dots are flat keys (single quoted segment), not nested paths.
        assert_eq!(json_path_for("schema.version"), r#"$."schema.version""#);
    }

    #[test]
    fn embedded_quote_is_escaped() {
        assert_eq!(json_path_for(r#"a"b"#), r#"$."a\"b""#);
    }

    #[test]
    fn embedded_backslash_is_escaped() {
        assert_eq!(json_path_for(r"a\b"), r#"$."a\\b""#);
    }
}
