//! Vault file inventory query.

use camino::Utf8PathBuf;
use vault_core::VaultFile;

use crate::error::CacheError;

impl crate::Cache {
    /// Every vault file (`vault files`), markdown included. Ordered by path.
    pub fn files(&self) -> Result<Vec<VaultFile>, CacheError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, ext FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let ext: String = row.get(1)?;
            Ok((path, ext))
        })?;
        let mut files = Vec::new();
        for row in rows {
            let (path_str, ext) = row?;
            let path = Utf8PathBuf::from(path_str);
            let stem = path.file_stem().unwrap_or_default().to_string();
            let extension = if ext.is_empty() { None } else { Some(ext) };
            files.push(VaultFile {
                path,
                stem,
                extension,
                hash: None,
            });
        }
        Ok(files)
    }
}
