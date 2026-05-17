use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use rusqlite::{params, Connection};
use vault_core::{GraphIndex, LinkKind, LinkSourceArea, LinkStatus, Severity, UnresolvedReason};

use crate::{CacheSummary, IndexError};

const CACHE_SCHEMA_VERSION: &str = "2";

pub fn write_sqlite_cache(
    index: &GraphIndex,
    cache: impl AsRef<Utf8Path>,
) -> Result<CacheSummary, IndexError> {
    let cache_path = cache_file_path(cache.as_ref());
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|source| IndexError::CacheDirectoryCreateFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut connection = Connection::open(cache_path.as_str())?;
    initialize_cache_schema(&connection)?;
    let transaction = connection.transaction()?;
    clear_cache(&transaction)?;
    insert_index(&transaction, index)?;
    transaction.commit()?;

    Ok(CacheSummary {
        cache_path,
        files: index.files.len(),
        ignored_files: index.ignored_files.len(),
        documents: index.documents.len(),
        links: index
            .documents
            .iter()
            .map(|document| document.links.len())
            .sum(),
        diagnostics: index
            .documents
            .iter()
            .map(|document| document.diagnostics.len())
            .sum(),
    })
}

fn cache_file_path(cache: &Utf8Path) -> Utf8PathBuf {
    if cache.extension().is_some() {
        cache.to_path_buf()
    } else {
        cache.join("graph.sqlite")
    }
}

fn initialize_cache_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        DROP TABLE IF EXISTS diagnostics;
        DROP TABLE IF EXISTS metadata;
        DROP TABLE IF EXISTS links;
        DROP TABLE IF EXISTS block_ids;
        DROP TABLE IF EXISTS headings;
        DROP TABLE IF EXISTS documents;
        DROP TABLE IF EXISTS files;

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY NOT NULL,
            hash TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS documents (
            path TEXT PRIMARY KEY NOT NULL,
            stem TEXT NOT NULL,
            frontmatter_json TEXT
        );

        CREATE TABLE IF NOT EXISTS headings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            level INTEGER NOT NULL,
            text TEXT NOT NULL,
            slug TEXT NOT NULL,
            line INTEGER,
            column INTEGER,
            byte_offset INTEGER
        );

        CREATE TABLE IF NOT EXISTS block_ids (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            block_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_path TEXT NOT NULL,
            raw TEXT NOT NULL,
            kind TEXT NOT NULL,
            target TEXT NOT NULL,
            label TEXT,
            anchor TEXT,
            block_ref TEXT,
            status TEXT NOT NULL,
            resolved_path TEXT,
            unresolved_reason TEXT,
            candidates_json TEXT,
            line INTEGER,
            column INTEGER,
            byte_offset INTEGER,
            source_area TEXT,
            source_property TEXT
        );

        CREATE TABLE IF NOT EXISTS diagnostics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            severity TEXT NOT NULL,
            code TEXT NOT NULL,
            message TEXT NOT NULL,
            detail TEXT
        );

        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        "#,
    )
}

fn clear_cache(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        DELETE FROM diagnostics;
        DELETE FROM metadata;
        DELETE FROM links;
        DELETE FROM block_ids;
        DELETE FROM headings;
        DELETE FROM documents;
        DELETE FROM files;
        "#,
    )
}

fn insert_index(connection: &Connection, index: &GraphIndex) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO metadata (key, value) VALUES ('schema_version', ?1)",
        params![CACHE_SCHEMA_VERSION],
    )?;

    for file in &index.files {
        connection.execute(
            "INSERT INTO files (path, hash) VALUES (?1, ?2)",
            params![file.path.as_str(), file.hash],
        )?;
    }

    for document in &index.documents {
        connection.execute(
            "INSERT INTO documents (path, stem, frontmatter_json) VALUES (?1, ?2, ?3)",
            params![
                document.path.as_str(),
                document.stem,
                document
                    .frontmatter
                    .as_ref()
                    .map(|frontmatter| frontmatter.to_string())
            ],
        )?;

        for heading in &document.headings {
            connection.execute(
                "INSERT INTO headings (path, level, text, slug, line, column, byte_offset)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    document.path.as_str(),
                    heading.level,
                    heading.text,
                    heading.slug,
                    heading.source_span.as_ref().map(|span| span.line),
                    heading.source_span.as_ref().map(|span| span.column),
                    heading.source_span.as_ref().map(|span| span.byte_offset),
                ],
            )?;
        }

        for block_id in &document.block_ids {
            connection.execute(
                "INSERT INTO block_ids (path, block_id) VALUES (?1, ?2)",
                params![document.path.as_str(), block_id],
            )?;
        }

        for link in &document.links {
            connection.execute(
                "INSERT INTO links (
                    source_path, raw, kind, target, label, anchor, block_ref, status,
                    resolved_path, unresolved_reason, candidates_json, line, column, byte_offset,
                    source_area, source_property
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    link.source_path.as_str(),
                    link.raw,
                    link_kind_name(&link.kind),
                    link.target,
                    link.label,
                    link.anchor,
                    link.block_ref,
                    link_status_name(&link.status),
                    link.resolved_path.as_ref().map(|path| path.as_str()),
                    link.unresolved_reason.as_ref().map(unresolved_reason_name),
                    serde_json::to_string(&link.candidates).unwrap_or_else(|_| "[]".to_string()),
                    link.source_span.as_ref().map(|span| span.line),
                    link.source_span.as_ref().map(|span| span.column),
                    link.source_span.as_ref().map(|span| span.byte_offset),
                    link.source_context
                        .as_ref()
                        .map(|context| link_source_area_name(&context.area)),
                    link.source_context
                        .as_ref()
                        .and_then(|context| context.property.as_deref()),
                ],
            )?;
        }

        for diagnostic in &document.diagnostics {
            connection.execute(
                "INSERT INTO diagnostics (path, severity, code, message, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    document.path.as_str(),
                    severity_name(&diagnostic.severity),
                    diagnostic.code,
                    diagnostic.message,
                    diagnostic.detail,
                ],
            )?;
        }
    }

    Ok(())
}

fn link_kind_name(kind: &LinkKind) -> &'static str {
    match kind {
        LinkKind::Markdown => "markdown",
        LinkKind::Wikilink => "wikilink",
        LinkKind::Embed => "embed",
    }
}

fn link_status_name(status: &LinkStatus) -> &'static str {
    match status {
        LinkStatus::Resolved => "resolved",
        LinkStatus::Unresolved => "unresolved",
        LinkStatus::Ambiguous => "ambiguous",
    }
}

fn link_source_area_name(area: &LinkSourceArea) -> &'static str {
    match area {
        LinkSourceArea::Body => "body",
        LinkSourceArea::Frontmatter => "frontmatter",
    }
}

fn unresolved_reason_name(reason: &UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::TargetMissing => "target-missing",
        UnresolvedReason::AnchorMissing => "anchor-missing",
        UnresolvedReason::BlockRefMissing => "block-ref-missing",
        UnresolvedReason::Ambiguous => "ambiguous",
    }
}

fn severity_name(severity: &Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}
