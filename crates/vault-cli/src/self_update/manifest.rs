//! cargo-dist `dist-manifest.json` parsing.
//!
//! Only the fields we use are modeled. `serde` ignores everything else
//! by default.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DistManifest {
    pub announcement_tag: String,
    pub artifacts: BTreeMap<String, Artifact>,
}

impl DistManifest {
    /// Strip a leading `v` from `announcement_tag` (`v0.33.1` -> `0.33.1`).
    pub fn announcement_version(&self) -> &str {
        self.announcement_tag
            .strip_prefix('v')
            .unwrap_or(&self.announcement_tag)
    }
}

#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub target_triples: Vec<String>,
    #[serde(default)]
    pub checksums: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"{
        "dist_version": "0.32.0",
        "announcement_tag": "v0.33.1",
        "announcement_title": "Version 0.33.1",
        "artifacts": {
            "vault-aarch64-apple-darwin.tar.xz": {
                "name": "vault-aarch64-apple-darwin.tar.xz",
                "kind": "executable-zip",
                "target_triples": ["aarch64-apple-darwin"],
                "checksums": {
                    "sha256": "abc123def456"
                }
            },
            "vault-x86_64-apple-darwin.tar.xz": {
                "name": "vault-x86_64-apple-darwin.tar.xz",
                "kind": "executable-zip",
                "target_triples": ["x86_64-apple-darwin"],
                "checksums": {
                    "sha256": "deadbeef"
                }
            },
            "vault-aarch64-apple-darwin.tar.xz.sha256": {
                "name": "vault-aarch64-apple-darwin.tar.xz.sha256",
                "kind": "checksum"
            }
        }
    }"#;

    #[test]
    fn parses_manifest() {
        let m: DistManifest = serde_json::from_str(SAMPLE_MANIFEST).unwrap();
        assert_eq!(m.announcement_tag, "v0.33.1");
        assert_eq!(m.artifacts.len(), 3);
    }

    #[test]
    fn announcement_version_strips_v_prefix() {
        let m: DistManifest = serde_json::from_str(SAMPLE_MANIFEST).unwrap();
        assert_eq!(m.announcement_version(), "0.33.1");
    }
}
