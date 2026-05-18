use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct PowershellInstaller;

impl Installer for PowershellInstaller {
    fn shell_name(&self) -> &'static str {
        "powershell"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("powershell installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("powershell installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
