use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use serde::Serialize;
use vault_cache::{Cache, DocumentQuery};
use vault_core::{Document, GraphIndex, Link, LinkStatus};

#[derive(Debug, Serialize)]
pub struct InspectOutput {
    pub document: Document,
    pub incoming_links: Vec<Link>,
    pub outgoing_links: Vec<Link>,
    pub unresolved_outgoing_links: Vec<Link>,
}

pub fn backlinks<'a>(index: &'a GraphIndex, target_path: &Utf8PathBuf) -> Vec<&'a Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .filter(|link| link.resolved_path.as_ref() == Some(target_path))
        .collect()
}

pub fn resolve_backlink_target_path(index: &GraphIndex, target: &str) -> Result<Utf8PathBuf> {
    if let Some(file) = index.files.iter().find(|file| file.path == target) {
        return Ok(file.path.clone());
    }

    resolve_target_path(index, target)
}

pub fn resolve_target_path(index: &GraphIndex, target: &str) -> Result<Utf8PathBuf> {
    if let Some(document) = index
        .documents
        .iter()
        .find(|document| document.path == target)
    {
        return Ok(document.path.clone());
    }

    let matches = index
        .documents
        .iter()
        .filter(|document| document.stem.eq_ignore_ascii_case(target))
        .map(|document| document.path.clone())
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => bail!("no document matched path or stem: {target}"),
        many => bail!(
            "ambiguous document stem: {target}; candidates: {}",
            many.iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Cache-aware variant of `resolve_target_path`. Resolves the target as an
/// exact path first, then falls back to stem matching against the
/// documents table. Mirrors the GraphIndex variant's semantics exactly.
#[allow(dead_code)]
pub fn resolve_target_path_cache(cache: &Cache, target: &str) -> Result<Utf8PathBuf> {
    // Exact path match: ask the cache for that one path.
    let exact = cache.documents_matching(&DocumentQuery {
        path_globs: vec![target.to_string()],
        ..Default::default()
    })?;
    if exact.iter().any(|d| d.path.as_str() == target) {
        return Ok(Utf8PathBuf::from(target));
    }

    // Stem match: scan all docs (the comparison is case-insensitive and
    // requires loading all summaries to inspect stems; the underlying
    // cost is still one SELECT against documents).
    let all = cache.documents_matching(&DocumentQuery::default())?;
    let matches = all
        .iter()
        .filter(|d| d.stem.eq_ignore_ascii_case(target))
        .map(|d| d.path.clone())
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => bail!("no document matched path or stem: {target}"),
        many => bail!(
            "ambiguous document stem: {target}; candidates: {}",
            many.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Cache-aware variant of `resolve_backlink_target_path`. Prefers exact
/// file match (for non-markdown targets) before falling back to document
/// resolution.
#[allow(dead_code)]
pub fn resolve_backlink_target_path_cache(cache: &Cache, target: &str) -> Result<Utf8PathBuf> {
    let files = cache.files()?;
    if let Some(file) = files.iter().find(|f| f.path.as_str() == target) {
        return Ok(file.path.clone());
    }
    resolve_target_path_cache(cache, target)
}

pub fn inspect_document(index: &GraphIndex, target_path: &Utf8PathBuf) -> Result<InspectOutput> {
    let document = index
        .documents
        .iter()
        .find(|document| &document.path == target_path)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("document not found after resolution: {target_path}"))?;

    let incoming_links = backlinks(index, target_path)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let outgoing_links = document.links.clone();
    let unresolved_outgoing_links = document
        .links
        .iter()
        .filter(|link| link.status != LinkStatus::Resolved)
        .cloned()
        .collect::<Vec<_>>();

    Ok(InspectOutput {
        document,
        incoming_links,
        outgoing_links,
        unresolved_outgoing_links,
    })
}

#[cfg(test)]
mod cache_resolve_tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::TempDir;
    use vault_cache::Cache;

    fn synth() -> (TempDir, Utf8PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf())
            .unwrap()
            .join("vault");
        std::fs::create_dir(root.as_std_path()).unwrap();
        std::fs::write(root.join("Notes.md").as_std_path(), "---\n---\n").unwrap();
        std::fs::write(root.join("attachment.png").as_std_path(), b"png").unwrap();
        (tmp, root)
    }

    #[test]
    fn exact_path_resolves() {
        let (_tmp, root) = synth();
        let mut cache = Cache::open(&root).unwrap();
        cache.rebuild(&root).unwrap();

        let p = resolve_target_path_cache(&cache, "Notes.md").unwrap();
        assert_eq!(p.as_str(), "Notes.md");
    }

    #[test]
    fn stem_case_insensitive_resolves() {
        let (_tmp, root) = synth();
        let mut cache = Cache::open(&root).unwrap();
        cache.rebuild(&root).unwrap();

        let p = resolve_target_path_cache(&cache, "notes").unwrap();
        assert_eq!(p.as_str(), "Notes.md");
    }

    #[test]
    fn file_target_resolves_via_backlink_variant() {
        let (_tmp, root) = synth();
        let mut cache = Cache::open(&root).unwrap();
        cache.rebuild(&root).unwrap();

        let p = resolve_backlink_target_path_cache(&cache, "attachment.png").unwrap();
        assert_eq!(p.as_str(), "attachment.png");
    }
}
