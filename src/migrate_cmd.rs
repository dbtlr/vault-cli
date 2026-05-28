//! `norn migrate <plan>` — apply a MigrationPlan from a JSON or YAML file.
//!
//! # Format detection
//! - `.yaml` / `.yml` extensions → YAML
//! - Any other extension, or no extension → JSON
//! - stdin (`-`) → JSON unless `--input-format yaml` is given
//!
//! # Exit codes
//! - 0: success (or dry-run with no failures)
//! - 1: runtime failure (at least one op failed during apply)
//! - 2: pre-flight refusal (parse error, schema-version mismatch, expansion error)

use crate::applier::{apply_migration_plan, ApplyContext};
use crate::apply_report::ApplyReport;
use crate::cli::{InputFormat, MigrateFormat};
use crate::migration_plan::{MigrationPlan, MIGRATION_PLAN_SCHEMA_VERSION};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::io::{self, Read, Write};

pub struct MigrateRunArgs {
    pub plan_path: String,
    pub dry_run: bool,
    pub yes: bool,
    pub format: MigrateFormat,
    pub input_format: Option<InputFormat>,
    pub out: Option<String>,
}

/// Pre-flight error exit code.
pub const EXIT_PREFLIGHT: i32 = 2;
/// Runtime failure exit code.
pub const EXIT_RUNTIME: i32 = 1;
/// Success exit code.
pub const EXIT_OK: i32 = 0;

pub fn run(
    args: MigrateRunArgs,
    cwd: &Utf8PathBuf,
    no_cache_refresh: bool,
    config_path: Option<&Utf8PathBuf>,
) -> Result<i32> {
    // ------------------------------------------------------------------
    // 1. Read plan source
    // ------------------------------------------------------------------
    let raw = read_plan_source(&args.plan_path)
        .with_context(|| format!("failed to read migration plan from '{}'", args.plan_path))?;

    // ------------------------------------------------------------------
    // 2. Determine input format (extension → YAML, else JSON, stdin default JSON)
    // ------------------------------------------------------------------
    let fmt = determine_input_format(&args.plan_path, args.input_format);

    // ------------------------------------------------------------------
    // 3. Parse plan
    // ------------------------------------------------------------------
    let plan = parse_plan(&raw, fmt, &args.plan_path)?;

    // ------------------------------------------------------------------
    // 4. Validate schema version — exit 2 if mismatch
    // ------------------------------------------------------------------
    if plan.schema_version != MIGRATION_PLAN_SCHEMA_VERSION {
        eprintln!(
            "error: unsupported plan schema_version {}; this norn build supports v{}",
            plan.schema_version, MIGRATION_PLAN_SCHEMA_VERSION
        );
        return Ok(EXIT_PREFLIGHT);
    }

    // ------------------------------------------------------------------
    // 5. Build GraphIndex
    // ------------------------------------------------------------------
    let loaded_config = crate::config_loader::load_config(cwd, config_path)?;
    let index =
        crate::cache_cmd::load_graph_index(cwd, &loaded_config.index_options, no_cache_refresh)?;

    // ------------------------------------------------------------------
    // 6. Determine whether to apply
    //    - --dry-run: never apply
    //    - --yes: skip TTY confirmation
    //    - --format json: implicitly non-interactive
    //    - TTY without --yes: prompt
    //    - Non-TTY without --yes: implicit dry-run
    // ------------------------------------------------------------------
    use std::io::IsTerminal;

    let dry_run = if args.dry_run {
        true
    } else if args.yes || matches!(args.format, MigrateFormat::Json) {
        false
    } else if std::io::stdin().is_terminal() {
        // TTY interactive: prompt
        use std::io::Write;
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        let mut prompt_out = std::io::stderr();
        writeln!(prompt_out)?;
        let ok =
            crate::prompt::confirm(&mut reader, &mut prompt_out, "Apply migration plan? [y/N] ")?;
        if !ok {
            std::process::exit(1);
        }
        false
    } else {
        // Non-TTY, no --yes: implicit dry-run
        true
    };

    let ctx = ApplyContext {
        dry_run,
        parents: false,
    };

    let report = match apply_migration_plan(&plan, &index, ctx) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e:#}");
            return Ok(EXIT_PREFLIGHT);
        }
    };

    // ------------------------------------------------------------------
    // 7. Determine exit code
    // ------------------------------------------------------------------
    let exit = if report.failed > 0 {
        EXIT_RUNTIME
    } else {
        EXIT_OK
    };

    // ------------------------------------------------------------------
    // 8. Render
    // ------------------------------------------------------------------
    render_report(&report, args.format, args.out.as_deref())?;

    Ok(exit)
}

/// Read plan content from a file path or stdin (`-`).
fn read_plan_source(plan_path: &str) -> Result<String> {
    if plan_path == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("could not read migration plan from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(plan_path)
            .with_context(|| format!("could not read file '{plan_path}'"))
    }
}

/// Determine the input format from path extension or explicit override.
fn determine_input_format(plan_path: &str, override_fmt: Option<InputFormat>) -> InputFormat {
    if let Some(fmt) = override_fmt {
        return fmt;
    }
    // stdin: default JSON
    if plan_path == "-" {
        return InputFormat::Json;
    }
    // detect from extension
    let lower = plan_path.to_ascii_lowercase();
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        InputFormat::Yaml
    } else {
        InputFormat::Json
    }
}

/// Parse a `MigrationPlan` from raw text in the given format.
fn parse_plan(raw: &str, fmt: InputFormat, source: &str) -> Result<MigrationPlan> {
    match fmt {
        InputFormat::Yaml => serde_yaml::from_str(raw)
            .with_context(|| format!("failed to parse YAML migration plan from '{source}'")),
        InputFormat::Json => serde_json::from_str(raw)
            .with_context(|| format!("failed to parse JSON migration plan from '{source}'")),
    }
}

/// Render the apply report to stdout (and optionally to a file via `--out`).
fn render_report(report: &ApplyReport, format: MigrateFormat, out: Option<&str>) -> Result<()> {
    // --out: always writes JSON to a file
    if let Some(out_path) = out {
        let json = serde_json::to_string_pretty(report)?;
        std::fs::write(out_path, format!("{json}\n"))
            .with_context(|| format!("failed to write apply report to '{out_path}'"))?;
    }

    // stdout: governed by --format (silent when --out is set without explicit format request)
    // For this command, --format always applies to stdout regardless of --out.
    let stdout = io::stdout();
    let mut out_lock = stdout.lock();
    match format {
        MigrateFormat::Json => {
            let json = serde_json::to_string_pretty(report)?;
            out_lock.write_all(json.as_bytes())?;
            out_lock.write_all(b"\n")?;
        }
        MigrateFormat::Records => {
            render_records(report, &mut out_lock)?;
        }
    }
    Ok(())
}

/// Human-readable records rendering for the apply report.
fn render_records(report: &ApplyReport, out: &mut dyn Write) -> Result<()> {
    let status_label = if report.dry_run { "dry-run" } else { "applied" };
    writeln!(out, "migrate {status_label}")?;
    writeln!(
        out,
        "  applied: {}  skipped: {}  failed: {}  remaining: {}",
        report.applied, report.skipped, report.failed, report.remaining
    )?;
    for op in &report.operations {
        let status = format!("{:?}", op.status).to_lowercase();
        writeln!(out, "  [{status}] {}", op.summary)?;
    }
    if !report.warnings.is_empty() {
        writeln!(out, "warnings:")?;
        for w in &report.warnings {
            writeln!(out, "  {}: {}", w.code, w.message)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_detection_yaml_extension() {
        assert!(matches!(
            determine_input_format("plan.yaml", None),
            InputFormat::Yaml
        ));
        assert!(matches!(
            determine_input_format("plan.yml", None),
            InputFormat::Yaml
        ));
    }

    #[test]
    fn format_detection_json_extension_and_default() {
        assert!(matches!(
            determine_input_format("plan.json", None),
            InputFormat::Json
        ));
        assert!(matches!(
            determine_input_format("plan", None),
            InputFormat::Json
        ));
    }

    #[test]
    fn format_detection_stdin_defaults_json() {
        assert!(matches!(
            determine_input_format("-", None),
            InputFormat::Json
        ));
    }

    #[test]
    fn format_detection_override_wins() {
        assert!(matches!(
            determine_input_format("plan.json", Some(InputFormat::Yaml)),
            InputFormat::Yaml
        ));
    }
}
