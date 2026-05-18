use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct NushellInstaller;

impl Installer for NushellInstaller {
    fn shell_name(&self) -> &'static str {
        "nushell"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("nushell installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("nushell installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
