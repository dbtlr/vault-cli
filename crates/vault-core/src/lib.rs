pub mod display;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpan {
    pub line: usize,
    pub column: usize,
    pub byte_offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LinkSourceArea {
    Body,
    Frontmatter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkSourceContext {
    pub area: LinkSourceArea,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
}

impl Diagnostic {
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LinkKind {
    Markdown,
    Wikilink,
    Embed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LinkStatus {
    Resolved,
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UnresolvedReason {
    TargetMissing,
    AnchorMissing,
    BlockRefMissing,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub source_path: Utf8PathBuf,
    pub raw: String,
    pub kind: LinkKind,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<LinkSourceContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_reason: Option<UnresolvedReason>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub candidates: Vec<Utf8PathBuf>,
    pub status: LinkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultFile {
    pub path: Utf8PathBuf,
    pub stem: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub path: Utf8PathBuf,
    pub stem: String,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<Value>,
    /// The post-frontmatter body of the document, retained for downstream
    /// indexing (cache writer, future FTS5). Empty when the file could not
    /// be read.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_text: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub headings: Vec<Heading>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub block_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub links: Vec<Link>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// A lean Document projection — Document minus the joined tables (headings,
/// block_ids, outgoing links, diagnostics). Sufficient for every query
/// command except `docs inspect`, which needs the joined data and uses
/// `Document` directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentSummary {
    pub path: Utf8PathBuf,
    pub stem: String,
    pub hash: String,
    pub frontmatter: Option<Value>,
    pub body_text: String,
}

impl From<&Document> for DocumentSummary {
    fn from(doc: &Document) -> Self {
        DocumentSummary {
            path: doc.path.clone(),
            stem: doc.stem.clone(),
            hash: doc.hash.clone(),
            frontmatter: doc.frontmatter.clone(),
            body_text: doc.body_text.clone(),
        }
    }
}

impl From<Document> for DocumentSummary {
    fn from(doc: Document) -> Self {
        DocumentSummary {
            path: doc.path,
            stem: doc.stem,
            hash: doc.hash,
            frontmatter: doc.frontmatter,
            body_text: doc.body_text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphIndex {
    pub root: Utf8PathBuf,
    pub files: Vec<VaultFile>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ignored_files: Vec<Utf8PathBuf>,
    pub documents: Vec<Document>,
}

#[cfg(test)]
mod document_summary_tests {
    use super::*;
    use camino::Utf8PathBuf;
    use serde_json::json;

    #[test]
    fn from_document_drops_joined_tables() {
        let doc = Document {
            path: Utf8PathBuf::from("notes/a.md"),
            stem: "a".to_string(),
            hash: "abc".to_string(),
            frontmatter: Some(json!({"type": "note"})),
            body_text: "hello".to_string(),
            headings: vec![],
            block_ids: vec![],
            links: vec![],
            diagnostics: vec![],
        };

        let summary: DocumentSummary = (&doc).into();

        assert_eq!(summary.path, doc.path);
        assert_eq!(summary.stem, doc.stem);
        assert_eq!(summary.hash, doc.hash);
        assert_eq!(summary.frontmatter, doc.frontmatter);
        assert_eq!(summary.body_text, doc.body_text);
    }

    #[test]
    fn from_owned_document_matches_ref_conversion() {
        let doc = Document {
            path: camino::Utf8PathBuf::from("notes/a.md"),
            stem: "a".to_string(),
            hash: "abc".to_string(),
            frontmatter: Some(serde_json::json!({"type": "note"})),
            body_text: "hello".to_string(),
            headings: vec![],
            block_ids: vec![],
            links: vec![],
            diagnostics: vec![],
        };
        let from_ref: DocumentSummary = (&doc).into();
        let from_owned: DocumentSummary = doc.into();
        assert_eq!(from_owned, from_ref);
    }
}
