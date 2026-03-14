use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{
    CURRENT_CONFIG_VERSION, Config, config_path, legacy_json_config_path, legacy_plugin_dir_path,
    plugin_dir_path,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub migrated: bool,
    pub backups_created: Vec<String>,
    pub actions: Vec<String>,
    pub status: String,
}

#[derive(Default, Deserialize)]
struct LegacyJsonConfig {
    #[serde(default, alias = "location", alias = "default_city")]
    location: Option<String>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
}

pub fn run_startup_migration() -> Result<MigrationStatus, String> {
    let mut status = MigrationStatus {
        status: "up-to-date".to_string(),
        ..MigrationStatus::default()
    };

    migrate_legacy_json_config(&mut status)?;
    migrate_existing_toml_schema(&mut status)?;
    migrate_legacy_plugin_dir(&mut status)?;

    if status.migrated {
        status.status = "migrated".to_string();
    }

    Ok(status)
}

pub fn inspect_migration_status() -> Result<MigrationStatus, String> {
    let mut status = MigrationStatus {
        status: "up-to-date".to_string(),
        ..MigrationStatus::default()
    };

    if legacy_json_config_path()?.exists() {
        status.status = "migration-available".to_string();
        status
            .actions
            .push("legacy ~/.tw/config.json can be migrated".to_string());
    }

    if legacy_plugin_dir_path()?.exists() {
        status.status = "migration-available".to_string();
        status
            .actions
            .push("legacy ~/.tinfo/plugins can be migrated".to_string());
    }

    let config = Config::load_or_create()?;
    if config.config_version < CURRENT_CONFIG_VERSION {
        status.status = "migration-available".to_string();
        status.actions.push(format!(
            "config schema v{} can be migrated to v{}",
            config.config_version, CURRENT_CONFIG_VERSION
        ));
    }

    Ok(status)
}

fn migrate_legacy_json_config(status: &mut MigrationStatus) -> Result<(), String> {
    let legacy = legacy_json_config_path()?;
    let target = config_path()?;
    if !legacy.exists() || target.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(&legacy)
        .map_err(|err| format!("Failed to read legacy config {}: {err}", legacy.display()))?;
    if contents.trim().is_empty() {
        return Ok(());
    }

    let legacy_config: LegacyJsonConfig = serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse legacy config {}: {err}", legacy.display()))?;

    let mut config = Config {
        config_version: CURRENT_CONFIG_VERSION,
        default_city: legacy_config.location,
        api_key: legacy_config.api_key,
        ..Config::default()
    };
    if let Some(units) = legacy_config.units {
        config.units = if units.eq_ignore_ascii_case("imperial") {
            crate::config::Units::Imperial
        } else {
            crate::config::Units::Metric
        };
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create config directory: {err}"))?;
    }
    config.save()?;
    let backup = backup_file(&legacy)?;
    status.backups_created.push(backup.display().to_string());
    status
        .actions
        .push("migrated legacy ~/.tw/config.json to ~/.tinfo/config.toml".to_string());
    status.migrated = true;
    Ok(())
}

fn migrate_existing_toml_schema(status: &mut MigrationStatus) -> Result<(), String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read config file {}: {err}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(());
    }
    let mut config: Config = toml::from_str(&contents)
        .map_err(|err| format!("Failed to parse config file {}: {err}", path.display()))?;
    let original_version = config.config_version;
    config.ensure_current_version();
    if original_version == config.config_version {
        return Ok(());
    }

    let backup = backup_file(&path)?;
    status.backups_created.push(backup.display().to_string());
    config.save()?;
    status.actions.push(format!(
        "migrated config schema from v{} to v{}",
        original_version, config.config_version
    ));
    status.migrated = true;
    Ok(())
}

fn migrate_legacy_plugin_dir(status: &mut MigrationStatus) -> Result<(), String> {
    let legacy = legacy_plugin_dir_path()?;
    let target = plugin_dir_path()?;
    if !legacy.exists() || legacy == target {
        return Ok(());
    }

    fs::create_dir_all(&target).map_err(|err| {
        format!(
            "Failed to create plugin directory {}: {err}",
            target.display()
        )
    })?;

    let mut moved_any = false;
    for entry in fs::read_dir(&legacy)
        .map_err(|err| format!("Failed to read legacy plugin directory: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("Failed to read legacy plugin directory: {err}"))?;
        let path = entry.path();
        let target_path = target.join(entry.file_name());
        if target_path.exists() {
            continue;
        }
        fs::rename(&path, &target_path).map_err(|err| {
            format!(
                "Failed to migrate plugin path '{}' to '{}': {err}",
                path.display(),
                target_path.display()
            )
        })?;
        moved_any = true;
    }

    if moved_any {
        status
            .actions
            .push("migrated legacy ~/.tinfo/plugins to ~/.terminal-info/plugins".to_string());
        status.migrated = true;
    }

    Ok(())
}

fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = PathBuf::from(format!("{}.bak.{stamp}", path.display()));
    fs::copy(path, &backup).map_err(|err| {
        format!(
            "Failed to create backup '{}' for '{}': {err}",
            backup.display(),
            path.display()
        )
    })?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_name_contains_original_path() {
        let path = PathBuf::from("/tmp/example.toml");
        let backup = PathBuf::from(format!("{}.bak.1", path.display()));
        assert!(backup.display().to_string().contains("example.toml.bak."));
    }
}
