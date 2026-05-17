mod cache;
mod config;
mod graph;
mod links;
mod pattern;

use camino::Utf8PathBuf;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("vault root does not exist: {0}")]
    MissingRoot(Utf8PathBuf),
    #[error("vault root is not a directory: {0}")]
    RootNotDirectory(Utf8PathBuf),
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),
    #[error("failed to write SQLite cache: {0}")]
    CacheWriteFailed(#[from] rusqlite::Error),
    #[error("failed to create cache directory {path}: {source}")]
    CacheDirectoryCreateFailed {
        path: Utf8PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheSummary {
    pub cache_path: Utf8PathBuf,
    pub files: usize,
    pub ignored_files: usize,
    pub documents: usize,
    pub links: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    pub ignore: Vec<String>,
}

pub use cache::write_sqlite_cache;
pub use config::{
    GraphConfig, ValidateConfig, ValidateRuleConfig, ValidateRuleExcludeConfig,
    ValidateRuleMatchConfig, VaultConfig,
};
pub use graph::{build_index, build_index_with_options, concise_diagnostics, has_errors};
pub use pattern::pattern_matches_path;
