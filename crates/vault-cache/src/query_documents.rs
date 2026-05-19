//! SQL-direct document query — `Cache::documents_matching` and
//! `Cache::document_by_path`.

use camino::{Utf8Path, Utf8PathBuf};
use rusqlite::params_from_iter;
use rusqlite::types::Value as SqlValue;
use rusqlite::OptionalExtension;
use vault_core::DocumentSummary;
use vault_graph::pattern_matches_path;

use crate::error::CacheError;
use crate::query::{json_path_for, DocumentQuery};

impl crate::Cache {
    /// Document summaries matching the predicate set. Empty predicate set
    /// returns every document. Result ordered by `path ASC`.
    ///
    /// Frontmatter predicates push into SQL via `json_extract` with the JSON
    /// path bound as a parameter; path globs post-filter via
    /// `vault_graph::pattern_matches_path`.
    pub fn documents_matching(
        &self,
        query: &DocumentQuery,
    ) -> Result<Vec<DocumentSummary>, CacheError> {
        let (sql, binds) = build_documents_matching_sql(query);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(binds.iter()), |row| {
            let path: String = row.get(0)?;
            let stem: String = row.get(1)?;
            let hash: String = row.get(2)?;
            let frontmatter_json: Option<String> = row.get(3)?;
            let body_text: String = row.get(4)?;
            let frontmatter = frontmatter_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            Ok(DocumentSummary {
                path: Utf8PathBuf::from(path),
                stem,
                hash,
                frontmatter,
                body_text,
            })
        })?;

        let mut docs: Vec<DocumentSummary> = Vec::new();
        for row in rows {
            docs.push(row?);
        }

        if !query.path_globs.is_empty() {
            docs.retain(|doc| {
                query
                    .path_globs
                    .iter()
                    .any(|pattern| pattern_matches_path(pattern, &doc.path))
            });
        }

        Ok(docs)
    }

    /// Single document by exact vault-relative path, fully populated with
    /// headings, block_ids, outgoing links, and diagnostics. Returns `None`
    /// if the path is not in the cache.
    ///
    /// Used by `docs inspect`. Callers wanting many documents should use
    /// `documents_matching` instead — looping `document_by_path` per row
    /// triggers per-document sub-queries against the join tables and
    /// defeats the purpose of the v2 narrowing.
    pub fn document_by_path(
        &self,
        path: &Utf8Path,
    ) -> Result<Option<vault_core::Document>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT path, stem, hash, frontmatter_json, body_text \
             FROM documents WHERE path = ?",
        )?;
        let row = stmt
            .query_row([path.as_str()], |row| {
                let path: String = row.get(0)?;
                let stem: String = row.get(1)?;
                let hash: String = row.get(2)?;
                let frontmatter_json: Option<String> = row.get(3)?;
                let body_text: String = row.get(4)?;
                Ok((path, stem, hash, frontmatter_json, body_text))
            })
            .optional()?;

        let Some((path_str, stem, hash, fm_json, body_text)) = row else {
            return Ok(None);
        };

        let frontmatter = fm_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        let path_buf = Utf8PathBuf::from(path_str);
        let headings = crate::reader::load_headings(&self.conn, path_buf.as_str())?;
        let block_ids = crate::reader::load_block_ids(&self.conn, path_buf.as_str())?;
        let links = crate::reader::load_links(&self.conn, path_buf.as_str())?;
        let diagnostics = crate::reader::load_diagnostics(&self.conn, path_buf.as_str())?;

        Ok(Some(vault_core::Document {
            path: path_buf,
            stem,
            hash,
            frontmatter,
            body_text,
            headings,
            block_ids,
            links,
            diagnostics,
        }))
    }
}

fn build_documents_matching_sql(query: &DocumentQuery) -> (String, Vec<SqlValue>) {
    let mut where_clauses: Vec<&'static str> = Vec::new();
    let mut binds: Vec<SqlValue> = Vec::new();

    for (field, value) in &query.frontmatter_eq {
        where_clauses.push("json_extract(frontmatter_json, ?) = ?");
        binds.push(SqlValue::Text(json_path_for(field)));
        binds.push(json_value_to_sql(value));
    }
    for field in &query.frontmatter_has {
        where_clauses.push("json_extract(frontmatter_json, ?) IS NOT NULL");
        binds.push(SqlValue::Text(json_path_for(field)));
    }
    for field in &query.frontmatter_missing {
        where_clauses.push("json_extract(frontmatter_json, ?) IS NULL");
        binds.push(SqlValue::Text(json_path_for(field)));
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT path, stem, hash, frontmatter_json, body_text \
         FROM documents{} ORDER BY path",
        where_sql
    );
    (sql, binds)
}

/// Convert a `serde_json::Value` scalar to the native SQLite type that
/// `json_extract` returns for that same value.  This lets the `= ?` predicate
/// compare apples-to-apples: `json_extract` strips JSON encoding and returns
/// TEXT for strings, INTEGER for integers/booleans, REAL for floats, and NULL
/// for JSON null.  Objects and arrays are left JSON-encoded (TEXT) because
/// `json_extract` on an object/array column also returns JSON text.
fn json_value_to_sql(v: &serde_json::Value) -> SqlValue {
    match v {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => SqlValue::Text(s.clone()),
        // Objects/arrays: json_extract returns JSON text for these.
        _ => SqlValue::Text(serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())),
    }
}
