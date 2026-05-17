use std::{collections::BTreeMap, fs};

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct RegistryEntry {
    pub name: String,
    pub path: Utf8PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RegistryFile {
    #[serde(default)]
    vaults: BTreeMap<String, Utf8PathBuf>,
}

pub fn add_vault(name: &str, path: &Utf8PathBuf) -> Result<()> {
    validate_name(name)?;
    let path = absolute_existing_dir(path)?;
    let mut registry = load_registry()?;
    registry.vaults.insert(name.to_string(), path);
    save_registry(&registry)
}

pub fn remove_vault(name: &str) -> Result<()> {
    let mut registry = load_registry()?;
    if registry.vaults.remove(name).is_none() {
        bail!("vault is not registered: {name}");
    }
    save_registry(&registry)
}

pub fn list_vaults() -> Result<Vec<RegistryEntry>> {
    Ok(load_registry()?
        .vaults
        .into_iter()
        .map(|(name, path)| RegistryEntry { name, path })
        .collect())
}

pub fn resolve_vault(name: &str) -> Result<Utf8PathBuf> {
    load_registry()?
        .vaults
        .remove(name)
        .ok_or_else(|| anyhow::anyhow!("vault is not registered: {name}"))
}

fn load_registry() -> Result<RegistryFile> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(RegistryFile::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| anyhow::anyhow!("failed to read registry {path}: {error}"))?;
    serde_yaml::from_str(&text)
        .map_err(|error| anyhow::anyhow!("failed to parse registry {path}: {error}"))
}

fn save_registry(registry: &RegistryFile) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            anyhow::anyhow!("failed to create registry directory {parent}: {error}")
        })?;
    }
    let text = serde_yaml::to_string(registry)
        .map_err(|error| anyhow::anyhow!("failed to serialize registry: {error}"))?;
    fs::write(&path, text)
        .map_err(|error| anyhow::anyhow!("failed to write registry {path}: {error}"))
}

fn registry_path() -> Result<Utf8PathBuf> {
    let config_home = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(path) => Utf8PathBuf::from_path_buf(path.into()).map_err(|path| {
            anyhow::anyhow!("XDG_CONFIG_HOME is not valid UTF-8: {}", path.display())
        })?,
        None => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot locate registry"))?;
            Utf8PathBuf::from_path_buf(home.into())
                .map_err(|path| anyhow::anyhow!("HOME is not valid UTF-8: {}", path.display()))?
                .join(".config")
        }
    };
    Ok(config_home.join("vault/registry.yaml"))
}

fn absolute_existing_dir(path: &Utf8PathBuf) -> Result<Utf8PathBuf> {
    let path = if path.is_absolute() {
        path.clone()
    } else {
        let current_dir = std::env::current_dir()
            .map_err(|error| anyhow::anyhow!("failed to read current directory: {error}"))?;
        let current_dir = Utf8PathBuf::from_path_buf(current_dir).map_err(|path| {
            anyhow::anyhow!("current directory is not valid UTF-8: {}", path.display())
        })?;
        current_dir.join(path)
    };

    if !path.exists() {
        bail!("vault root does not exist: {path}");
    }
    if !path.is_dir() {
        bail!("vault root is not a directory: {path}");
    }
    Ok(path)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name
            .chars()
            .any(|character| character == '/' || character == '\\' || character.is_whitespace())
    {
        bail!("vault name must be non-empty and may not contain whitespace or path separators");
    }
    Ok(())
}
