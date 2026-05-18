use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct ZshInstaller;

impl Installer for ZshInstaller {
    fn shell_name(&self) -> &'static str {
        "zsh"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("zsh installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("zsh installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
