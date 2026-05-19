//! Canonical-path-hash identity for the cache directory.

use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};

use crate::error::CacheError;

/// Resolves the vault root to its canonical form (symlinks resolved) and
/// returns a stable SHA-256 hex digest of the canonical path.
pub fn vault_identity(vault_root: &Utf8Path) -> Result<(Utf8PathBuf, String), CacheError> {
    let canonical = std::fs::canonicalize(vault_root.as_std_path()).map_err(|e| {
        CacheError::CannotCanonicalize {
            path: vault_root.to_owned(),
            source: e,
        }
    })?;
    let canonical = Utf8PathBuf::from_path_buf(canonical).map_err(|p| CacheError::Io {
        path: vault_root.to_owned(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("canonical path is not valid UTF-8: {}", p.display()),
        ),
    })?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_str().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    Ok((canonical, hash))
}

/// Returns the cache directory path for a given vault root.
/// Format: `<XDG_CACHE_HOME>/vault/<sha256-of-canonical-root>/`, defaulting
/// to `~/.cache/vault/<hash>/` when `XDG_CACHE_HOME` is unset.
pub fn cache_dir_for(vault_root: &Utf8Path) -> Result<(Utf8PathBuf, Utf8PathBuf), CacheError> {
    let (canonical, hash) = vault_identity(vault_root)?;
    let base = xdg_cache_home()?;
    let dir = base.join("vault").join(hash);
    Ok((canonical, dir))
}

fn xdg_cache_home() -> Result<Utf8PathBuf, CacheError> {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return Ok(Utf8PathBuf::from(xdg));
        }
    }
    let home = std::env::var("HOME").map_err(|_| CacheError::Io {
        path: Utf8PathBuf::from("$HOME"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"),
    })?;
    Ok(Utf8PathBuf::from(home).join(".cache"))
}
