use std::fs;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use vault_core::GraphIndex;
use vault_standards::apply::{
    apply_file_changes, apply_link_rewrites, apply_move, apply_rewrite_link, changes_by_path,
    validate_plan_for_apply, ApplyError, LinkRewriteResult, MoveResult, RepairApplyWarning,
};
use vault_standards::{Finding, PlannedChange, RepairPlan};

#[allow(unused_imports)]
pub use vault_standards::apply::{
    RepairApplyPlanContext, RepairApplyReport, RepairApplyVerification,
};

pub fn apply_repair_plan(
    cwd: &Utf8PathBuf,
    index: &GraphIndex,
    plan: &RepairPlan,
    dry_run: bool,
) -> Result<RepairApplyReport> {
    validate_plan_for_apply(cwd, plan)?;

    // Pass 1: per-file frontmatter edits. changes_by_path skips
    // move_document, so the grouped map only contains set/remove/add
    // frontmatter changes.
    let grouped = changes_by_path(plan)?;

    let mut report = RepairApplyReport::new(plan, dry_run);

    let current_hashes: std::collections::BTreeMap<Utf8PathBuf, String> = index
        .documents
        .iter()
        .map(|d| (d.path.clone(), d.hash.clone()))
        .collect();

    for (rel_path, changes) in &grouped {
        let current_hash = current_hashes.get(rel_path).ok_or_else(|| {
            anyhow::anyhow!(ApplyError::UnknownPath {
                path: rel_path.clone(),
            })
        })?;
        let plan_hash = &changes[0].document_hash;
        if current_hash != plan_hash {
            return Err(anyhow::anyhow!(ApplyError::StaleDocumentHash {
                path: rel_path.clone(),
                expected: plan_hash.clone(),
                actual: current_hash.clone(),
            }));
        }

        let absolute_path = cwd.join(rel_path);
        let original =
            fs::read_to_string(&absolute_path).with_context(|| format!("read {absolute_path}"))?;
        let updated = apply_file_changes(&original, changes)?;

        if updated != original {
            report.changed_files.push(rel_path.clone());
            if !dry_run {
                fs::write(&absolute_path, updated)
                    .with_context(|| format!("write {absolute_path}"))?;
            }
        }
    }

    // Pass 1b: rewrite_link operations (broken wikilink target rewrites).
    // Hash check uses the index snapshot (same as Pass 1 frontmatter check).
    for change in plan
        .changes
        .iter()
        .filter(|c| c.operation == "rewrite_link")
    {
        let current_hash = current_hashes.get(&change.path).ok_or_else(|| {
            anyhow::anyhow!(ApplyError::UnknownPath {
                path: change.path.clone(),
            })
        })?;
        if current_hash != &change.document_hash {
            return Err(anyhow::anyhow!(ApplyError::StaleDocumentHash {
                path: change.path.clone(),
                expected: change.document_hash.clone(),
                actual: current_hash.clone(),
            }));
        }

        if dry_run {
            // Record what would be rewritten without touching the file.
            if let (Some(from), Some(to)) = (
                change.expected_old_value.as_ref().and_then(|v| v.as_str()),
                change.new_value.as_ref().and_then(|v| v.as_str()),
            ) {
                report.rewritten_links.push(LinkRewriteResult {
                    file: change.path.clone(),
                    from: from.to_string(),
                    to: to.to_string(),
                });
                if !report.changed_files.contains(&change.path) {
                    report.changed_files.push(change.path.clone());
                }
            }
            continue;
        }

        let absolute_path = cwd.join(&change.path);
        let content =
            fs::read_to_string(&absolute_path).with_context(|| format!("read {absolute_path}"))?;
        let updated = apply_rewrite_link(&content, change)?;
        if updated != content {
            fs::write(&absolute_path, &updated)
                .with_context(|| format!("write {absolute_path}"))?;
            if !report.changed_files.contains(&change.path) {
                report.changed_files.push(change.path.clone());
            }
            if let (Some(from), Some(to)) = (
                change.expected_old_value.as_ref().and_then(|v| v.as_str()),
                change.new_value.as_ref().and_then(|v| v.as_str()),
            ) {
                report.rewritten_links.push(LinkRewriteResult {
                    file: change.path.clone(),
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        }
    }

    // Collect move_document changes for passes 2 and 3.
    let move_changes: Vec<&PlannedChange> = plan
        .changes
        .iter()
        .filter(|c| c.operation == "move_document")
        .collect();

    // Pass 2: filesystem moves.
    let mut moves: Vec<MoveResult> = Vec::new();
    for change in &move_changes {
        if dry_run {
            if let Some(destination) = change.destination.as_ref() {
                moves.push(MoveResult {
                    from: change.path.clone(),
                    to: destination.clone(),
                });
            }
        } else {
            moves.push(apply_move(cwd, change)?);
        }
    }

    // Pass 3: link rewrites (only after every move succeeded).
    let mut rewrites: Vec<LinkRewriteResult> = Vec::new();
    for change in &move_changes {
        if dry_run {
            if let Some(risk) = &change.link_risk {
                for affected in risk
                    .stem_links
                    .iter()
                    .chain(risk.path_qualified_wikilinks.iter())
                    .chain(risk.markdown_links.iter())
                {
                    rewrites.push(LinkRewriteResult {
                        file: affected.source_path.clone(),
                        from: affected.raw.clone(),
                        to: affected.rewritten.clone(),
                    });
                }
            }
        } else {
            rewrites.extend(apply_link_rewrites(cwd, change)?);
        }
    }

    let warnings: Vec<RepairApplyWarning> = move_changes
        .iter()
        .flat_map(|c| {
            c.warnings.iter().map(|w| RepairApplyWarning {
                path: c.path.clone(),
                warning: w.clone(),
            })
        })
        .collect();

    report.moved_files = moves;
    // Extend (not replace): Pass 1b may have already populated rewritten_links
    // with rewrite_link results; Pass 3 appends move-induced backlink rewrites.
    report.rewritten_links.extend(rewrites);
    report.warnings = warnings;

    Ok(report)
}

pub fn with_verification(report: RepairApplyReport, findings: &[Finding]) -> RepairApplyReport {
    report.with_verification(findings)
}
