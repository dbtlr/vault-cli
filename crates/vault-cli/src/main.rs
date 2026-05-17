use std::fs;

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vault_core::{Diagnostic, Document, GraphIndex, Link, LinkStatus, Severity, VaultFile};
use vault_index::{
    build_index_with_options, concise_diagnostics, has_errors, pattern_matches_path,
    write_sqlite_cache, DoctorConfig, DoctorRuleConfig, IndexOptions, VaultConfig,
};

#[derive(Debug, Parser)]
#[command(name = "vault")]
#[command(about = "Deterministic Markdown vault graph tools")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Query and cache derived Markdown vault graph facts")]
    Graph(GraphCommand),
    #[command(
        about = "Emit read-only vault health findings",
        long_about = "Emit read-only vault health findings.\n\nDoctor reports reuse graph/index facts to surface unresolved links, ambiguous links, document diagnostics, and configured frontmatter requirements. Doctor does not mutate files."
    )]
    Doctor(DoctorArgs),
}

#[derive(Debug, Parser)]
#[command(
    about = "Read-only graph/index commands for Markdown vaults",
    long_about = "Read-only graph/index commands for Markdown vaults.\n\nThe graph surface is a deterministic, read-only view of raw Markdown vault structure. It emits Obsidian-compatible link facts, document metadata, file inventory, diagnostics, and cache projections without applying standards-pack semantics or mutating files."
)]
struct GraphCommand {
    #[command(subcommand)]
    command: GraphSubcommand,
}

#[derive(Debug, Subcommand)]
enum GraphSubcommand {
    #[command(
        about = "Write a SQLite graph cache and emit a build summary",
        long_about = "Write a SQLite graph cache and emit a build summary.\n\nThe cache includes inventoried files, parsed Markdown documents, headings, block IDs, graph link facts, and diagnostics. --format only controls stdout; the cache is always SQLite."
    )]
    Build(BuildArgs),
    #[command(
        about = "Emit parsed Markdown documents with frontmatter, headings, links, and diagnostics"
    )]
    Documents(DocumentsArgs),
    #[command(
        about = "Emit all parsed link facts",
        long_about = "Emit all parsed link facts.\n\nIncludes body wikilinks, embeds, frontmatter/property wikilinks, URL-decoded Markdown internal links, extensionless Markdown note links, same-note heading/block references, Markdown image links to local files, and links to existing attachments. Use source_context.area and source_context.property to distinguish body links from frontmatter links."
    )]
    Links(GraphArgs),
    #[command(
        about = "Emit inventoried vault files",
        long_about = "Emit inventoried vault files.\n\nFiles include Markdown documents and non-Markdown attachments. File records can be used with exact-path backlink queries for resolved attachment targets."
    )]
    Files(GraphArgs),
    #[command(
        about = "Emit unresolved and ambiguous link facts",
        long_about = "Emit unresolved and ambiguous link facts.\n\nRows include target-missing, anchor-missing, block-ref-missing, and ambiguous reasons. Ambiguous rows include candidate document paths."
    )]
    Unresolved(GraphArgs),
    #[command(about = "Emit document parse diagnostics")]
    Diagnostics(GraphArgs),
    #[command(
        about = "Emit incoming links for an exact path or unique stem",
        long_about = "Emit incoming links for an exact vault-relative file path or unique document stem.\n\nExact paths may target Markdown documents or non-Markdown files. Stem matching only applies to Markdown documents and is case-insensitive."
    )]
    Backlinks(TargetGraphArgs),
    #[command(about = "Emit one document plus incoming, outgoing, and unresolved outgoing links")]
    Inspect(TargetGraphArgs),
}

#[derive(Debug, Parser)]
struct BuildArgs {
    #[arg(long, default_value = ".", help = "Vault root to index")]
    root: Utf8PathBuf,
    #[arg(long, help = "YAML config file with graph.ignore patterns")]
    config: Option<Utf8PathBuf>,
    #[arg(
        long,
        help = "SQLite cache file path or directory. Directories receive graph.sqlite; --format only controls stdout"
    )]
    cache: Utf8PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json, help = "Stdout format")]
    format: OutputFormat,
    #[arg(long, help = "Include verbose diagnostic details")]
    verbose: bool,
}

#[derive(Debug, Parser)]
struct DocumentsArgs {
    #[arg(long, default_value = ".", help = "Vault root to index")]
    root: Utf8PathBuf,
    #[arg(long, help = "YAML config file with graph.ignore patterns")]
    config: Option<Utf8PathBuf>,
    #[arg(
        long = "filter",
        help = "Frontmatter-only field:value filter. Repeat to require multiple fields"
    )]
    filters: Vec<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl, help = "Stdout format")]
    format: OutputFormat,
    #[arg(long, help = "Include verbose diagnostic details")]
    verbose: bool,
}

#[derive(Debug, Parser)]
struct GraphArgs {
    #[arg(long, default_value = ".", help = "Vault root to index")]
    root: Utf8PathBuf,
    #[arg(long, help = "YAML config file with graph.ignore patterns")]
    config: Option<Utf8PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl, help = "Stdout format")]
    format: OutputFormat,
    #[arg(long, help = "Include verbose diagnostic details")]
    verbose: bool,
}

#[derive(Debug, Parser)]
struct TargetGraphArgs {
    #[arg(
        help = "Exact vault-relative path or unique document stem. Stem matching is case-insensitive"
    )]
    target: String,
    #[arg(long, default_value = ".", help = "Vault root to index")]
    root: Utf8PathBuf,
    #[arg(long, help = "YAML config file with graph.ignore patterns")]
    config: Option<Utf8PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl, help = "Stdout format")]
    format: OutputFormat,
    #[arg(long, help = "Include verbose diagnostic details")]
    verbose: bool,
}

#[derive(Debug, Parser)]
struct DoctorArgs {
    #[arg(long, default_value = ".", help = "Vault root to inspect")]
    root: Utf8PathBuf,
    #[arg(long, help = "YAML config file with graph.ignore and doctor rules")]
    config: Option<Utf8PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl, help = "Stdout format")]
    format: OutputFormat,
    #[arg(long, help = "Include verbose diagnostic details")]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
}

#[derive(Debug, Serialize)]
struct InspectOutput {
    document: Document,
    incoming_links: Vec<Link>,
    outgoing_links: Vec<Link>,
    unresolved_outgoing_links: Vec<Link>,
}

#[derive(Debug, Serialize)]
struct DocumentDiagnostic {
    path: Utf8PathBuf,
    diagnostic: Diagnostic,
}

#[derive(Debug, Serialize)]
struct DoctorFinding {
    code: String,
    severity: Severity,
    path: Utf8PathBuf,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<Diagnostic>,
}

struct LoadedConfig {
    index_options: IndexOptions,
    doctor: DoctorConfig,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let exit_code = run(cli)?;
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Graph(graph) => match graph.command {
            GraphSubcommand::Build(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let summary = write_sqlite_cache(&index, &args.cache)?;
                write_item_output(&summary, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Documents(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let documents = filter_documents(&index, &args.filters)?;
                write_output(&documents, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Links(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let links = all_links(&index);
                write_output(&links, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Files(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let files = all_files(&index);
                write_output(&files, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Unresolved(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let links = unresolved_links(&index);
                write_output(&links, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Diagnostics(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let diagnostics = all_diagnostics(&index);
                write_output(&diagnostics, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Backlinks(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let target_path = resolve_backlink_target_path(&index, &args.target)?;
                let links = backlinks(&index, &target_path);
                write_output(&links, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Inspect(args) => {
                let mut index = build_index_for(&args.root, args.config.as_ref())?;
                trim_diagnostics(&mut index, args.verbose);
                let target_path = resolve_target_path(&index, &args.target)?;
                let output = inspect_document(&index, &target_path)?;
                write_item_output(&output, args.format)?;
                Ok(exit_code_for(&index))
            }
        },
        Command::Doctor(args) => {
            let loaded_config = load_config(args.config.as_ref())?;
            let mut index = build_index_with_options(&args.root, &loaded_config.index_options)?;
            trim_diagnostics(&mut index, args.verbose);
            let findings = doctor_findings(&index, &loaded_config.doctor);
            write_output(&findings, args.format)?;
            Ok(exit_code_for(&index))
        }
    }
}

fn build_index_for(root: &Utf8PathBuf, config_path: Option<&Utf8PathBuf>) -> Result<GraphIndex> {
    let loaded_config = load_config(config_path)?;
    Ok(build_index_with_options(
        root,
        &loaded_config.index_options,
    )?)
}

fn load_config(config_path: Option<&Utf8PathBuf>) -> Result<LoadedConfig> {
    let config = match config_path {
        Some(config_path) => {
            let config_text = fs::read_to_string(config_path)
                .map_err(|error| anyhow::anyhow!("failed to read config {config_path}: {error}"))?;
            let config_value =
                serde_yaml::from_str::<serde_yaml::Value>(&config_text).map_err(|error| {
                    anyhow::anyhow!("failed to parse config {config_path}: {error}")
                })?;
            validate_config_value(config_path, &config_value)?;
            serde_yaml::from_value::<VaultConfig>(config_value)
                .map_err(|error| anyhow::anyhow!("failed to parse config {config_path}: {error}"))?
        }
        None => VaultConfig::default(),
    };

    Ok(LoadedConfig {
        index_options: IndexOptions {
            ignore: config.graph.ignore,
        },
        doctor: config.doctor,
    })
}

fn validate_config_value(config_path: &Utf8PathBuf, value: &serde_yaml::Value) -> Result<()> {
    let Some(root) = value.as_mapping() else {
        anyhow::bail!("invalid config {config_path}: root must be a mapping");
    };

    if let Some(graph) = mapping_get(root, "graph") {
        let Some(graph) = graph.as_mapping() else {
            anyhow::bail!("invalid config {config_path}: graph must be a mapping");
        };

        if let Some(ignore) = mapping_get(graph, "ignore") {
            validate_string_sequence(config_path, "graph.ignore", ignore)?;
        }
    }

    if let Some(doctor) = mapping_get(root, "doctor") {
        let Some(doctor) = doctor.as_mapping() else {
            anyhow::bail!("invalid config {config_path}: doctor must be a mapping");
        };

        if let Some(required_frontmatter) = mapping_get(doctor, "required_frontmatter") {
            validate_string_sequence(
                config_path,
                "doctor.required_frontmatter",
                required_frontmatter,
            )?;
        }

        if let Some(rules) = mapping_get(doctor, "rules") {
            let Some(rules) = rules.as_sequence() else {
                anyhow::bail!("invalid config {config_path}: doctor.rules must be a sequence");
            };

            for (index, rule) in rules.iter().enumerate() {
                let rule_path = format!("doctor.rules[{index}]");
                validate_doctor_rule_value(config_path, &rule_path, rule)?;
            }
        }
    }

    Ok(())
}

fn validate_doctor_rule_value(
    config_path: &Utf8PathBuf,
    rule_path: &str,
    value: &serde_yaml::Value,
) -> Result<()> {
    let Some(rule) = value.as_mapping() else {
        anyhow::bail!("invalid config {config_path}: {rule_path} must be a mapping");
    };

    if let Some(name) = mapping_get(rule, "name") {
        if name.as_str().is_none() {
            anyhow::bail!("invalid config {config_path}: {rule_path}.name must be a string");
        }
    }

    if let Some(rule_match) = mapping_get(rule, "match") {
        let Some(rule_match) = rule_match.as_mapping() else {
            anyhow::bail!("invalid config {config_path}: {rule_path}.match must be a mapping");
        };

        if let Some(path) = mapping_get(rule_match, "path") {
            if path.as_str().is_none() {
                anyhow::bail!(
                    "invalid config {config_path}: {rule_path}.match.path must be a string"
                );
            }
        }
    }

    if let Some(required_frontmatter) = mapping_get(rule, "required_frontmatter") {
        validate_string_sequence(
            config_path,
            &format!("{rule_path}.required_frontmatter"),
            required_frontmatter,
        )?;
    }

    Ok(())
}

fn validate_string_sequence(
    config_path: &Utf8PathBuf,
    field_path: &str,
    value: &serde_yaml::Value,
) -> Result<()> {
    let Some(items) = value.as_sequence() else {
        anyhow::bail!("invalid config {config_path}: {field_path} must be a sequence");
    };

    for (index, item) in items.iter().enumerate() {
        if item.as_str().is_none() {
            anyhow::bail!("invalid config {config_path}: {field_path}[{index}] must be a string");
        }
    }

    Ok(())
}

fn mapping_get<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Value> {
    mapping.get(&serde_yaml::Value::String(key.to_string()))
}

#[cfg(test)]
mod config_validation_tests {
    use super::load_config;
    use camino::Utf8PathBuf;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_malformed_doctor_rule_match_path() {
        let config_path = write_temp_config(
            "doctor:\n  rules:\n    - name: bad\n      match:\n        path: 123\n      required_frontmatter:\n        - type\n",
        );

        let message = match load_config(Some(&config_path)) {
            Ok(_) => panic!("config should fail validation"),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("invalid config"));
        assert!(message.contains("doctor.rules[0].match.path must be a string"));
    }

    #[test]
    fn rejects_malformed_scoped_required_frontmatter() {
        let config_path = write_temp_config(
            "doctor:\n  rules:\n    - name: bad\n      match:\n        path: Workspaces/**/*.md\n      required_frontmatter:\n        - 123\n",
        );

        let message = match load_config(Some(&config_path)) {
            Ok(_) => panic!("config should fail validation"),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("invalid config"));
        assert!(message.contains("doctor.rules[0].required_frontmatter[0] must be a string"));
    }

    fn write_temp_config(contents: &str) -> Utf8PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        path.push(format!("vault-cli-config-validation-{nanos}.yaml"));
        fs::write(&path, contents).expect("temp config should be written");
        Utf8PathBuf::from_path_buf(path).expect("temp path should be utf8")
    }
}

fn trim_diagnostics(index: &mut GraphIndex, verbose: bool) {
    if verbose {
        return;
    }

    for document in &mut index.documents {
        document.diagnostics = concise_diagnostics(document);
    }
}

fn all_links(index: &GraphIndex) -> Vec<&Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .collect()
}

fn all_files(index: &GraphIndex) -> Vec<&VaultFile> {
    index.files.iter().collect()
}

fn unresolved_links(index: &GraphIndex) -> Vec<&Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .filter(|link| link.status != LinkStatus::Resolved)
        .collect()
}

fn all_diagnostics(index: &GraphIndex) -> Vec<DocumentDiagnostic> {
    index
        .documents
        .iter()
        .flat_map(|document| {
            document
                .diagnostics
                .iter()
                .cloned()
                .map(|diagnostic| DocumentDiagnostic {
                    path: document.path.clone(),
                    diagnostic,
                })
        })
        .collect()
}

fn doctor_findings(index: &GraphIndex, config: &DoctorConfig) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    for document in &index.documents {
        for diagnostic in &document.diagnostics {
            findings.push(DoctorFinding {
                code: diagnostic.code.clone(),
                severity: diagnostic.severity.clone(),
                path: document.path.clone(),
                message: diagnostic.message.clone(),
                field: None,
                rule: None,
                link: None,
                diagnostic: Some(diagnostic.clone()),
            });
        }

        for field in &config.required_frontmatter {
            if !document_has_frontmatter_field(document, field) {
                findings.push(DoctorFinding {
                    code: "frontmatter-required-field-missing".to_string(),
                    severity: Severity::Warning,
                    path: document.path.clone(),
                    message: format!("required frontmatter field is missing: {field}"),
                    field: Some(field.clone()),
                    rule: None,
                    link: None,
                    diagnostic: None,
                });
            }
        }

        for rule in matching_doctor_rules(document, &config.rules) {
            let rule_name = rule.name.clone();
            for field in &rule.required_frontmatter {
                if !document_has_frontmatter_field(document, field) {
                    findings.push(DoctorFinding {
                        code: "frontmatter-required-field-missing".to_string(),
                        severity: Severity::Warning,
                        path: document.path.clone(),
                        message: format!("required frontmatter field is missing: {field}"),
                        field: Some(field.clone()),
                        rule: rule_name.clone(),
                        link: None,
                        diagnostic: None,
                    });
                }
            }
        }

        for link in &document.links {
            match link.status {
                LinkStatus::Resolved => {}
                LinkStatus::Unresolved => {
                    findings.push(DoctorFinding {
                        code: "link-unresolved".to_string(),
                        severity: Severity::Warning,
                        path: document.path.clone(),
                        message: format!("unresolved link target: {}", link.target),
                        field: None,
                        rule: None,
                        link: Some(link.clone()),
                        diagnostic: None,
                    });
                }
                LinkStatus::Ambiguous => {
                    findings.push(DoctorFinding {
                        code: "link-ambiguous".to_string(),
                        severity: Severity::Warning,
                        path: document.path.clone(),
                        message: format!("ambiguous link target: {}", link.target),
                        field: None,
                        rule: None,
                        link: Some(link.clone()),
                        diagnostic: None,
                    });
                }
            }
        }
    }

    findings
}

fn matching_doctor_rules<'a>(
    document: &Document,
    rules: &'a [DoctorRuleConfig],
) -> Vec<&'a DoctorRuleConfig> {
    rules
        .iter()
        .filter(|rule| doctor_rule_matches(document, rule))
        .collect()
}

fn doctor_rule_matches(document: &Document, rule: &DoctorRuleConfig) -> bool {
    match &rule.r#match.path {
        Some(path_pattern) => pattern_matches_path(path_pattern, &document.path),
        None => true,
    }
}

fn document_has_frontmatter_field(document: &Document, field: &str) -> bool {
    document
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.get(field))
        .is_some_and(|value| !value.is_null())
}

fn backlinks<'a>(index: &'a GraphIndex, target_path: &Utf8PathBuf) -> Vec<&'a Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .filter(|link| link.resolved_path.as_ref() == Some(target_path))
        .collect()
}

fn resolve_backlink_target_path(index: &GraphIndex, target: &str) -> Result<Utf8PathBuf> {
    if let Some(file) = index.files.iter().find(|file| file.path == target) {
        return Ok(file.path.clone());
    }

    resolve_target_path(index, target)
}

fn resolve_target_path(index: &GraphIndex, target: &str) -> Result<Utf8PathBuf> {
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

fn inspect_document(index: &GraphIndex, target_path: &Utf8PathBuf) -> Result<InspectOutput> {
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

fn filter_documents<'a>(index: &'a GraphIndex, filters: &[String]) -> Result<Vec<&'a Document>> {
    let parsed_filters = filters
        .iter()
        .map(|filter| parse_filter(filter))
        .collect::<Result<Vec<_>>>()?;

    Ok(index
        .documents
        .iter()
        .filter(|document| {
            parsed_filters
                .iter()
                .all(|(field, expected)| frontmatter_matches(document, field, expected))
        })
        .collect())
}

fn parse_filter(filter: &str) -> Result<(String, String)> {
    let Some((field, value)) = filter.split_once(':') else {
        bail!("invalid filter, expected field:value: {filter}");
    };

    let field = field.trim();
    let value = value.trim();
    if field.is_empty() || value.is_empty() {
        bail!("invalid filter, expected non-empty field and value: {filter}");
    }

    Ok((field.to_string(), value.to_string()))
}

fn frontmatter_matches(document: &Document, field: &str, expected: &str) -> bool {
    let Some(frontmatter) = &document.frontmatter else {
        return false;
    };
    let Some(value) = frontmatter.get(field) else {
        return false;
    };

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| scalar_value_matches(value, expected)),
        other => scalar_value_matches(other, expected),
    }
}

fn scalar_value_matches(value: &serde_json::Value, expected: &str) -> bool {
    match value {
        serde_json::Value::String(actual) => actual == expected,
        serde_json::Value::Bool(actual) => actual.to_string() == expected,
        serde_json::Value::Number(actual) => actual.to_string() == expected,
        _ => false,
    }
}

fn write_output<T: Serialize>(items: &[T], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(items)?);
        }
        OutputFormat::Jsonl => {
            for item in items {
                println!("{}", serde_json::to_string(item)?);
            }
        }
    }
    Ok(())
}

fn write_item_output<T: Serialize>(item: &T, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item)?);
        }
        OutputFormat::Jsonl => {
            println!("{}", serde_json::to_string(item)?);
        }
    }
    Ok(())
}

fn exit_code_for(index: &GraphIndex) -> i32 {
    if has_errors(index) {
        1
    } else {
        0
    }
}
