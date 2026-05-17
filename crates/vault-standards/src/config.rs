use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VaultConfig {
    #[serde(default)]
    pub files: FilesConfig,
    #[serde(default)]
    pub validate: ValidateConfig,
    #[serde(default)]
    pub repair: RepairConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FilesConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ValidateConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub required_frontmatter: Vec<String>,
    #[serde(default)]
    pub rules: Vec<ValidateRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ValidateRule {
    pub name: Option<String>,
    #[serde(default)]
    pub r#match: RuleSelector,
    #[serde(default)]
    pub required_frontmatter: Vec<String>,
    #[serde(default)]
    pub allowed_values: HashMap<String, Vec<serde_json::Value>>,
    #[serde(default)]
    pub field_types: HashMap<String, String>,
    #[serde(default)]
    pub forbidden_frontmatter: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub exclude: RuleExclude,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuleSelector {
    pub path: Option<String>,
    pub path_not: Option<String>,
    #[serde(default)]
    pub frontmatter: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuleExclude {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepairConfig {
    #[serde(default)]
    pub rules: Vec<RepairRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepairRule {
    pub name: Option<String>,
    #[serde(default)]
    pub r#match: RepairRuleMatch,
    #[serde(flatten)]
    pub action: RepairAction,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepairRuleMatch {
    pub code: Option<String>,
    pub rule: Option<String>,
    pub field: Option<String>,
    pub actual_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairAction {
    SetFrontmatter {
        field: String,
        value: serde_json::Value,
    },
    RemoveFrontmatter {
        field: String,
    },
}
