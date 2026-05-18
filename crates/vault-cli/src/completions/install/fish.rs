use anyhow::{bail, Result};

use super::{Installer, TargetPaths};

pub(super) struct FishInstaller;

impl Installer for FishInstaller {
    fn shell_name(&self) -> &'static str {
        "fish"
    }
    fn target_paths(&self) -> Result<TargetPaths> {
        bail!("fish installer not implemented yet")
    }
    fn primary_content(&self, _today: &str) -> Result<String> {
        bail!("fish installer not implemented yet")
    }
    fn secondary_content(&self) -> Result<Option<String>> {
        Ok(None)
    }
    fn uses_marker_block(&self) -> bool {
        false
    }
}
