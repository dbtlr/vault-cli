pub mod apply;
mod checks;
mod config;
mod engine;
mod findings;
mod predicates;
mod repair;
mod summary;

pub use apply::{
    apply_file_changes, changes_by_path, validate_plan_for_apply, ApplyError,
    RepairApplyPlanContext, RepairApplyReport, RepairApplyVerification,
};
pub use config::{
    parse_config, ConfigError, FilesConfig, RemoveFrontmatterAction, RepairAction, RepairConfig,
    RepairRule, RepairRuleMatch, RuleExclude, RuleSelector, SetFrontmatterAction, ValidateConfig,
    ValidateRule, VaultConfig,
};
pub use engine::validate;
pub use findings::{Finding, FindingBody};
pub use repair::{
    plan_repairs, PlannedChange, RepairPlan, RepairPlanFilters, RepairPlanSummary, SkipReason,
    SkippedFinding, SkippedSummary, REPAIR_PLAN_SCHEMA_VERSION,
};
pub use summary::{summarize, Summary};
