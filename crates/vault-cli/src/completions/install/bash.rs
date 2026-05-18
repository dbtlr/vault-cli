use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct BashInstaller;

impl Installer for BashInstaller {
    fn shell_name(&self) -> &'static str {
        "bash"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("bash installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("bash installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
