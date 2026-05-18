use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct ElvishInstaller;

impl Installer for ElvishInstaller {
    fn shell_name(&self) -> &'static str {
        "elvish"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("elvish installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("elvish installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
