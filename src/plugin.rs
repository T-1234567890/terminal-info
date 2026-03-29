use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, Cursor, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dialoguer::{Input, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use minisign_verify::{PublicKey, Signature};
use reqwest::blocking::Client;
use reqwest::header::ACCEPT_ENCODING;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder};
use zip::ZipArchive;

use crate::config::home_dir_path;
use crate::output::{error_prefix, success_prefix};

pub const RESERVED_COMMANDS: &[&str] = &[
    "weather",
    "ping",
    "network",
    "system",
    "time",
    "diagnostic",
    "config",
    "profile",
    "completion",
    "plugin",
    "update",
];

const REGISTRY_OWNER: &str = "T-1234567890";
const REGISTRY_REPO: &str = "terminal-info";
const REGISTRY_BRANCH: &str = "main";
const INDEX_CACHE_TTL_SECS: u64 = 10 * 60;
const PLUGIN_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    #[serde(default = "default_plugin_license")]
    pub license: String,
    #[serde(alias = "repo")]
    pub repository: String,
    pub binary: String,
    pub entry: String,
    pub platform: Vec<String>,
    #[serde(default = "default_plugin_type")]
    pub r#type: String,
    #[serde(default)]
    pub requires_network: bool,
    #[serde(default)]
    pub short_description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "default_plugin_api")]
    pub plugin_api: u32,
    pub checksums: std::collections::BTreeMap<String, String>,
    pub pubkey: String,
}

#[derive(Clone, Serialize)]
pub struct PluginSearchEntry {
    pub name: String,
    pub description: String,
    pub short_description: String,
    pub repository: String,
    pub homepage: String,
    pub icon: String,
    pub screenshots: Vec<String>,
    pub version: String,
    pub trusted: bool,
    pub installed: bool,
}

#[derive(Clone, Serialize)]
struct PluginSearchView {
    query: Option<String>,
    installed: Vec<PluginSearchEntry>,
    available: Vec<PluginSearchEntry>,
}

#[derive(Serialize)]
struct RegistryJsonOutput {
    name: String,
    version: String,
    description: String,
    author: String,
    license: String,
    repository: String,
    binary: String,
    entry: String,
    platform: Vec<String>,
    #[serde(rename = "type")]
    type_name: String,
    requires_network: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    short_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    screenshots: Vec<String>,
    plugin_api: u32,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    capabilities: Vec<String>,
    pubkey: String,
    checksums: std::collections::BTreeMap<String, String>,
}

fn default_plugin_license() -> String {
    "MIT".to_string()
}

fn default_plugin_type() -> String {
    "local".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PluginIndexEntry {
    name: String,
    registry: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PluginIndexFile {
    version: u32,
    plugins: Vec<PluginIndexEntry>,
}

#[derive(Serialize, Deserialize)]
struct PluginIndexCache {
    fetched_at: u64,
    index: PluginIndexFile,
}

#[derive(Serialize, Deserialize)]
struct PluginMetadataCache {
    fetched_at: u64,
    plugin: PluginMetadata,
}

#[derive(Serialize, Deserialize)]
struct PluginManifest {
    plugin: PluginSection,
    command: CommandSection,
    compatibility: CompatibilitySection,
    #[serde(skip_serializing_if = "Option::is_none")]
    requirements: Option<RequirementsSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    install: Option<InstallSection>,
}

#[derive(Serialize, Deserialize)]
struct PluginSection {
    name: String,
    version: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CommandSection {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct CompatibilitySection {
    terminal_info: String,
    #[serde(default = "default_plugin_api")]
    plugin_api: u32,
}

#[derive(Serialize, Deserialize, Default)]
struct RequirementsSection {
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct InstallSection {
    version: String,
    target: String,
    asset_checksum: String,
}

#[derive(Serialize, Deserialize, Default)]
struct TrustedPlugins {
    trusted: Vec<String>,
}

#[derive(Serialize)]
struct PluginInfoView {
    name: String,
    repository: String,
    installed_version: Option<String>,
    pinned_version: Option<String>,
    checksum: String,
    trusted: bool,
    install_path: String,
    manifest: Option<toml::Value>,
}

#[derive(Serialize)]
struct PluginVerifyView {
    name: String,
    version_ok: bool,
    checksum_ok: bool,
    manifest_ok: bool,
    binary_ok: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginWidget {
    pub title: String,
    #[serde(default)]
    pub refresh_interval_secs: Option<u64>,
    pub full: PluginWidgetBody,
    #[serde(default)]
    pub compact: Option<PluginWidgetBody>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PluginWidgetBody {
    Text { content: String },
    List { items: Vec<String> },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

#[derive(Deserialize)]
struct LegacyPluginWidget {
    title: String,
    content: String,
}

impl PluginWidget {
    pub fn body(&self, compact: bool) -> &PluginWidgetBody {
        if compact {
            self.compact.as_ref().unwrap_or(&self.full)
        } else {
            &self.full
        }
    }

    pub fn refresh_interval_secs(&self) -> u64 {
        self.refresh_interval_secs.unwrap_or(5).max(1)
    }
}

#[derive(Serialize)]
struct PluginDoctorCheck {
    name: String,
    status: String,
    detail: String,
    fix: String,
}

#[derive(Serialize, Deserialize)]
struct PluginRuntimeMetadata {
    name: String,
    version: String,
    description: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    commands: Vec<String>,
    compatibility: PluginRuntimeCompatibility,
    #[serde(default)]
    capabilities: Vec<String>,
    api_version: u32,
}

#[derive(Serialize, Deserialize)]
struct PluginRuntimeCompatibility {
    tinfo: String,
    plugin_api: u32,
}

#[derive(Serialize)]
struct PluginInspectView {
    manifest: Option<toml::Value>,
    metadata: Option<PluginRuntimeMetadata>,
    compatibility_ok: bool,
    binary: Option<String>,
}

fn default_plugin_api() -> u32 {
    1
}

pub struct PluginDiagnosticSummary {
    pub unknown_plugins: Vec<String>,
    pub broken_paths: Vec<String>,
}

pub fn run_plugin(command: &str, args: &[String]) -> Result<(), String> {
    if !is_plugin_trusted(command)? {
        return Err(format!(
            "Plugin \"{command}\" is not trusted.\n\nRun:\ntinfo plugin trust {command}\n\nto allow it."
        ));
    }
    let binary_name = format!("tinfo-{command}");
    let binary_path = resolve_plugin_binary(&binary_name).ok_or_else(|| {
        format!("Unknown command '{command}'. No plugin named '{binary_name}' found.")
    })?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(args);
    for (key, value) in simulated_host_env() {
        cmd.env(key, value);
    }
    let status = cmd
        .status()
        .map_err(|err| format!("Failed to execute plugin '{}': {err}", binary_name))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Plugin '{}' exited with status {}.",
            binary_name,
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))
    }
}

pub fn set_plugin_trust(name: &str, trusted: bool) -> Result<(), String> {
    validate_plugin_name(name)?;
    let mut allowlist = load_trusted_plugins()?;
    allowlist.trusted.retain(|entry| entry != name);
    if trusted {
        allowlist.trusted.push(name.to_string());
        allowlist.trusted.sort();
    }
    save_trusted_plugins(&allowlist)?;
    println!(
        "{} plugin '{}'.",
        if trusted { "Trusted" } else { "Untrusted" },
        name
    );
    Ok(())
}

pub fn list_trusted_plugins() -> Result<(), String> {
    let allowlist = load_trusted_plugins()?;
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&allowlist).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }
    if allowlist.trusted.is_empty() {
        println!("No trusted plugins.");
        return Ok(());
    }
    for name in allowlist.trusted {
        println!("{name}");
    }
    Ok(())
}

pub fn info_plugin(name: &str) -> Result<(), String> {
    validate_plugin_name(name)?;
    let registry = load_plugin_by_name(name).ok();
    let install_path = plugin_home_path(name)?;
    let manifest = read_installed_manifest(name).ok();
    let installed_version = manifest
        .as_ref()
        .and_then(|value| value.get("plugin"))
        .and_then(|section| section.get("version"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let checksum_status = checksum_status(name, registry.as_ref(), manifest.as_ref());
    let view = PluginInfoView {
        name: name.to_string(),
        repository: registry
            .as_ref()
            .map(|plugin| plugin.repository.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        installed_version,
        pinned_version: registry.as_ref().map(|plugin| plugin.version.clone()),
        checksum: checksum_status,
        trusted: is_plugin_trusted(name)?,
        install_path: install_path.display().to_string(),
        manifest,
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("Plugin: {}", view.name);
        println!();
        println!("Repository: {}", view.repository);
        println!(
            "Installed version: {}",
            view.installed_version
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!(
            "Pinned version: {}",
            view.pinned_version.unwrap_or_else(|| "unknown".to_string())
        );
        println!("Checksum: {}", view.checksum);
        println!("Trusted: {}", if view.trusted { "yes" } else { "no" });
        println!("Install path:");
        println!("{}", view.install_path);
    }
    Ok(())
}

pub fn verify_plugins() -> Result<(), String> {
    let installed = installed_plugin_names()?;
    let mut results = Vec::new();
    for name in installed {
        let registry = load_plugin_by_name(&name).ok();
        let manifest = read_installed_manifest(&name).ok();
        let binary_path = plugin_home_path(&name)?.join(binary_filename(&format!("tinfo-{name}")));
        let version_ok = registry
            .as_ref()
            .zip(manifest.as_ref())
            .and_then(|(plugin, manifest)| {
                manifest
                    .get("plugin")
                    .and_then(|section| section.get("version"))
                    .and_then(|value| value.as_str())
                    .map(|version| version == plugin.version)
            })
            .unwrap_or(false);
        let checksum_ok =
            checksum_status(&name, registry.as_ref(), manifest.as_ref()) == "verified";
        let manifest_ok = manifest.is_some();
        let binary_ok = binary_path.exists();
        results.push(PluginVerifyView {
            name,
            version_ok,
            checksum_ok,
            manifest_ok,
            binary_ok,
        });
    }

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string())
        );
    } else if results.is_empty() {
        println!("No plugins installed.");
    } else {
        for item in results {
            println!(
                "{} version={} checksum={} manifest={} binary={}",
                item.name, item.version_ok, item.checksum_ok, item.manifest_ok, item.binary_ok
            );
        }
    }
    Ok(())
}

pub fn plugin_doctor() -> Result<(), String> {
    let installed = installed_plugin_names()?;
    let mut checks = Vec::new();

    if installed.is_empty() {
        checks.push(PluginDoctorCheck {
            name: "Installed plugins".to_string(),
            status: "WARN".to_string(),
            detail: "no plugins installed".to_string(),
            fix: "install a plugin with `tinfo plugin install <name>`".to_string(),
        });
    }

    for name in installed {
        let manifest_path = plugin_manifest_path(&name)?;
        let binary_path = plugin_home_path(&name)?.join(binary_filename(&format!("tinfo-{name}")));
        let registry = load_plugin_by_name(&name).ok();
        let manifest_ok = manifest_path.exists() && read_installed_manifest(&name).is_ok();
        let binary_ok = binary_path.exists();
        let signature_ok = checksum_status(
            &name,
            registry.as_ref(),
            read_installed_manifest(&name).ok().as_ref(),
        ) == "verified";
        checks.push(PluginDoctorCheck {
            name: format!("{name} manifest"),
            status: if manifest_ok { "PASS" } else { "FAIL" }.to_string(),
            detail: if manifest_ok {
                manifest_path.display().to_string()
            } else {
                "plugin manifest missing or invalid".to_string()
            },
            fix: "reinstall the plugin or restore plugin.toml".to_string(),
        });
        checks.push(PluginDoctorCheck {
            name: format!("{name} binary"),
            status: if binary_ok { "PASS" } else { "FAIL" }.to_string(),
            detail: if binary_ok {
                binary_path.display().to_string()
            } else {
                "plugin binary missing".to_string()
            },
            fix: "reinstall the plugin".to_string(),
        });
        checks.push(PluginDoctorCheck {
            name: format!("{name} registry"),
            status: if registry.is_some() { "PASS" } else { "WARN" }.to_string(),
            detail: if registry.is_some() {
                "registry metadata found".to_string()
            } else {
                "plugin not found in reviewed registry".to_string()
            },
            fix: "run `tinfo plugin search` or update the registry entry".to_string(),
        });
        checks.push(PluginDoctorCheck {
            name: format!("{name} signature"),
            status: if signature_ok { "PASS" } else { "WARN" }.to_string(),
            detail: if signature_ok {
                "installed checksum matches registry".to_string()
            } else {
                "installed asset could not be matched to registry metadata".to_string()
            },
            fix: "run `tinfo plugin verify` or reinstall the plugin".to_string(),
        });
    }

    render_plugin_checks(&checks)
}

pub fn plugin_lint() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?;
    let checks = plugin_project_checks(&cwd, false)?;
    render_plugin_checks(&checks)
}

pub fn plugin_publish_check() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?;
    let mut checks = plugin_project_checks(&cwd, true)?;
    let dist = cwd.join("dist");
    checks.push(PluginDoctorCheck {
        name: "Release artifacts".to_string(),
        status: if dist.exists() { "PASS" } else { "WARN" }.to_string(),
        detail: if dist.exists() {
            dist.display().to_string()
        } else {
            "dist/ not found".to_string()
        },
        fix: "build release artifacts before publishing".to_string(),
    });
    if let Ok(metadata) = run_local_plugin_metadata(&cwd) {
        checks.push(PluginDoctorCheck {
            name: "Metadata protocol".to_string(),
            status: if metadata.api_version == default_plugin_api() {
                "PASS"
            } else {
                "WARN"
            }
            .to_string(),
            detail: format!("api_version={}", metadata.api_version),
            fix: "return plugin_api 1 from the --metadata command".to_string(),
        });
    }
    let manifest = read_project_manifest(&cwd).ok();
    let plugin_name = manifest.as_ref().and_then(plugin_name_from_manifest);
    let plugin_version = manifest.as_ref().and_then(plugin_version_from_manifest);
    if let (Some(name), Some(version)) = (plugin_name, plugin_version) {
        let archive = dist.join(format!("{name}-v{version}.tar.gz"));
        let signature = dist.join(format!("{name}-v{version}.tar.gz.minisig"));
        let checksum = dist.join(format!("{name}-v{version}.tar.gz.sha256"));
        let registry_json = dist.join("registry").join(format!("{name}.json"));
        checks.push(project_file_check(
            &archive.display().to_string(),
            &archive,
            "run `tinfo plugin pack` to create the release bundle",
        ));
        checks.push(project_file_check(
            &signature.display().to_string(),
            &signature,
            "sign the release bundle with `tinfo plugin sign` or `tinfo plugin pack`",
        ));
        checks.push(project_file_check(
            &checksum.display().to_string(),
            &checksum,
            "generate the release checksum with `tinfo plugin pack`",
        ));
        checks.push(project_file_check(
            &registry_json.display().to_string(),
            &registry_json,
            "run `tinfo plugin pack` to generate registry JSON",
        ));
    }
    render_plugin_checks(&checks)
}

pub fn plugin_inspect() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?;
    let manifest_path = cwd.join("plugin.toml");
    let manifest = if manifest_path.exists() {
        let contents = fs::read_to_string(&manifest_path)
            .map_err(|err| format!("Failed to read {}: {err}", manifest_path.display()))?;
        Some(
            toml::from_str::<toml::Value>(&contents)
                .map_err(|err| format!("Failed to parse {}: {err}", manifest_path.display()))?,
        )
    } else {
        None
    };

    let binary = manifest
        .as_ref()
        .and_then(plugin_name_from_manifest)
        .map(|name| cwd.join("target").join("debug").join(binary_filename(&format!("tinfo-{name}"))))
        .filter(|path| path.exists());
    let metadata = if cwd.join("Cargo.toml").exists() {
        run_local_plugin_metadata(&cwd).ok()
    } else {
        None
    };
    let compatibility_ok = metadata
        .as_ref()
        .map(|meta| meta.api_version == default_plugin_api())
        .unwrap_or(false);
    let view = PluginInspectView {
        manifest,
        metadata,
        compatibility_ok,
        binary: binary.map(|path| path.display().to_string()),
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("Plugin project inspection");
    println!();
    println!(
        "Manifest: {}",
        if view.manifest.is_some() { "found" } else { "missing" }
    );
    println!(
        "Metadata command: {}",
        if view.metadata.is_some() { "available" } else { "unavailable" }
    );
    println!(
        "Plugin API compatibility: {}",
        if view.compatibility_ok { "ok" } else { "needs review" }
    );
    if let Some(binary) = view.binary {
        println!("Debug binary: {binary}");
    }
    if let Some(metadata) = view.metadata {
        println!();
        println!("Name: {}", metadata.name);
        println!("Version: {}", metadata.version);
        println!("Description: {}", metadata.description);
        println!(
            "Capabilities: {}",
            if metadata.capabilities.is_empty() {
                "none".to_string()
            } else {
                metadata.capabilities.join(", ")
            }
        );
    }
    Ok(())
}

pub fn plugin_test() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?;
    let mut checks = plugin_project_checks(&cwd, false)?;
    let metadata = run_local_plugin_metadata(&cwd)?;
    checks.push(PluginDoctorCheck {
        name: "Metadata command".to_string(),
        status: "PASS".to_string(),
        detail: format!("{} v{}", metadata.name, metadata.version),
        fix: "none".to_string(),
    });

    let preview = run_local_plugin_preview(&cwd)?;
    checks.push(PluginDoctorCheck {
        name: "Preview output".to_string(),
        status: "PASS".to_string(),
        detail: preview.lines().next().unwrap_or("no output").to_string(),
        fix: "none".to_string(),
    });
    render_plugin_checks(&checks)
}

pub fn plugin_pack(from_dist: bool) -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?;
    let manifest = read_project_manifest(&cwd)?;
    let plugin_name = plugin_name_from_manifest(&manifest)
        .ok_or_else(|| "plugin.toml is missing [plugin].name".to_string())?;
    let plugin_version = plugin_version_from_manifest(&manifest)
        .ok_or_else(|| "plugin.toml is missing [plugin].version".to_string())?;
    let dist = cwd.join("dist");
    fs::create_dir_all(&dist).map_err(|err| format!("Failed to create dist/: {err}"))?;

    let checksums = if from_dist {
        release_checksums_from_dist(&dist, &plugin_name)?
    } else {
        let binary_name = format!("tinfo-{plugin_name}");
        run_command(
            Command::new("cargo")
                .arg("build")
                .arg("--release")
                .current_dir(&cwd),
            "Failed to build plugin release binary",
        )?;

        let release_binary = cwd.join("target").join("release").join(binary_filename(&binary_name));
        if !release_binary.exists() {
            return Err(format!(
                "Expected release binary '{}' was not found.",
                release_binary.display()
            ));
        }

        let archive_name = format!("{plugin_name}-v{plugin_version}.tar.gz");
        let archive_path = dist.join(&archive_name);
        let tar_file = File::create(&archive_path)
            .map_err(|err| format!("Failed to create archive {}: {err}", archive_path.display()))?;
        let encoder = GzEncoder::new(tar_file, Compression::default());
        let mut archive = Builder::new(encoder);
        archive
            .append_path_with_name(&release_binary, binary_filename(&binary_name))
            .map_err(|err| format!("Failed to append binary to archive: {err}"))?;
        archive
            .append_path_with_name(cwd.join("plugin.toml"), "plugin.toml")
            .map_err(|err| format!("Failed to append plugin.toml to archive: {err}"))?;
        archive
            .finish()
            .map_err(|err| format!("Failed to finish archive: {err}"))?;

        let archive_bytes = fs::read(&archive_path)
            .map_err(|err| format!("Failed to read {}: {err}", archive_path.display()))?;
        let checksum = sha256_hex(&archive_bytes);
        let checksum_path = dist.join(format!("{archive_name}.sha256"));
        fs::write(&checksum_path, format!("{checksum}  {archive_name}\n"))
            .map_err(|err| format!("Failed to write checksum file: {err}"))?;

        let key = default_project_signing_key(&cwd)?;
        plugin_sign(&archive_path, Some(&key))?;

        let mut checksums = std::collections::BTreeMap::new();
        checksums.insert(target_triple().to_string(), checksum);

        println!("Created plugin bundle:");
        println!("  {}", archive_path.display());
        println!("  {}", checksum_path.display());
        println!("  {}.minisig", archive_path.display());
        checksums
    };

    let registry_json = build_registry_json_output(&cwd, &manifest, &plugin_name, checksums)?;
    let registry_dir = dist.join("registry");
    fs::create_dir_all(&registry_dir)
        .map_err(|err| format!("Failed to create {}: {err}", registry_dir.display()))?;
    let registry_path = registry_dir.join(format!("{plugin_name}.json"));
    let registry_contents = serde_json::to_string_pretty(&registry_json)
        .map_err(|err| format!("Failed to serialize registry JSON: {err}"))?;
    fs::write(&registry_path, format!("{registry_contents}\n"))
        .map_err(|err| format!("Failed to write {}: {err}", registry_path.display()))?;

    println!("Generated registry JSON:");
    println!("  {}", registry_path.display());
    Ok(())
}

fn plugin_project_checks(
    project_dir: &Path,
    include_workflow: bool,
) -> Result<Vec<PluginDoctorCheck>, String> {
    let plugin_manifest = project_dir.join("plugin.toml");
    let cargo_toml = project_dir.join("Cargo.toml");
    let readme = project_dir.join("README.md");
    let workflow = project_dir
        .join(".github")
        .join("workflows")
        .join("release.yml");

    let mut checks = vec![
        project_file_check("plugin.toml", &plugin_manifest, "create plugin.toml"),
        project_file_check("Cargo.toml", &cargo_toml, "create Cargo.toml"),
        project_file_check("README.md", &readme, "create README.md"),
    ];
    if include_workflow {
        checks.push(project_file_check(
            ".github/workflows/release.yml",
            &workflow,
            "add a release workflow for publishing",
        ));
    }

    if plugin_manifest.exists() {
        let contents = fs::read_to_string(&plugin_manifest)
            .map_err(|err| format!("Failed to read {}: {err}", plugin_manifest.display()))?;
        let parsed = toml::from_str::<toml::Value>(&contents).ok();
        let valid = parsed.is_some();
        checks.push(PluginDoctorCheck {
            name: "Manifest schema".to_string(),
            status: if valid { "PASS" } else { "FAIL" }.to_string(),
            detail: if valid {
                "plugin.toml parsed".to_string()
            } else {
                "plugin.toml failed to parse".to_string()
            },
            fix: "fix plugin.toml syntax and required sections".to_string(),
        });
        if let Some(value) = parsed {
            let compatibility_api = value
                .get("compatibility")
                .and_then(|section| section.get("plugin_api"))
                .and_then(|value| value.as_integer())
                .unwrap_or_default();
            checks.push(PluginDoctorCheck {
                name: "Plugin API version".to_string(),
                status: if compatibility_api == default_plugin_api() as i64 {
                    "PASS"
                } else {
                    "WARN"
                }
                .to_string(),
                detail: if compatibility_api == 0 {
                    "missing".to_string()
                } else {
                    compatibility_api.to_string()
                },
                fix: "set [compatibility].plugin_api = 1".to_string(),
            });
        }
    }

    Ok(checks)
}

fn project_file_check(name: &str, path: &Path, fix: &str) -> PluginDoctorCheck {
    PluginDoctorCheck {
        name: name.to_string(),
        status: if path.exists() { "PASS" } else { "FAIL" }.to_string(),
        detail: if path.exists() {
            path.display().to_string()
        } else {
            "missing".to_string()
        },
        fix: fix.to_string(),
    }
}

fn render_plugin_checks(checks: &[PluginDoctorCheck]) -> Result<(), String> {
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    for check in checks {
        println!("{}: {} ({})", check.status, check.name, check.detail);
        if check.fix != "none" {
            println!("FIX: {}", check.fix);
        }
    }
    Ok(())
}

fn read_project_manifest(project_dir: &Path) -> Result<toml::Value, String> {
    let manifest_path = project_dir.join("plugin.toml");
    let contents = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("Failed to read {}: {err}", manifest_path.display()))?;
    toml::from_str(&contents).map_err(|err| format!("Failed to parse {}: {err}", manifest_path.display()))
}

fn plugin_name_from_manifest(manifest: &toml::Value) -> Option<String> {
    manifest
        .get("plugin")
        .and_then(|section| section.get("name"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn plugin_version_from_manifest(manifest: &toml::Value) -> Option<String> {
    manifest
        .get("plugin")
        .and_then(|section| section.get("version"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn build_registry_json_output(
    project_dir: &Path,
    manifest: &toml::Value,
    plugin_name: &str,
    checksums: std::collections::BTreeMap<String, String>,
) -> Result<RegistryJsonOutput, String> {
    let description = manifest_string(manifest, &["plugin", "description"])
        .ok_or_else(|| "plugin.toml is missing [plugin].description".to_string())?;
    let version = manifest_string(manifest, &["plugin", "version"])
        .ok_or_else(|| "plugin.toml is missing [plugin].version".to_string())?;
    let repository = manifest_string(manifest, &["release", "repository"])
        .or_else(|| manifest_string(manifest, &["release", "repo"]))
        .ok_or_else(|| "plugin.toml is missing [release].repository".to_string())?;
    let pubkey = manifest_string(manifest, &["release", "pubkey"])
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_release_pubkey(project_dir, manifest).ok())
        .ok_or_else(|| {
            "Unable to determine plugin public key. Set [release].pubkey or [release].pubkey_path."
                .to_string()
        })?;
    let plugin_api = manifest
        .get("compatibility")
        .and_then(|section| section.get("plugin_api"))
        .and_then(|value| value.as_integer())
        .unwrap_or(default_plugin_api() as i64) as u32;
    let capabilities = manifest_string_array(manifest, &["requirements", "capabilities"]);
    let license = manifest_string(manifest, &["plugin", "license"])
        .unwrap_or_else(|| "MIT".to_string());
    let binary = manifest_string(manifest, &["release", "binary"])
        .unwrap_or_else(|| format!("tinfo-{plugin_name}"));
    let entry = manifest_string(manifest, &["command", "name"])
        .unwrap_or_else(|| plugin_name.to_string());
    let platform = platforms_from_checksums(&checksums);
    let requires_network = capabilities.iter().any(|item| item == "network");
    let type_name = manifest_string(manifest, &["release", "type"]).unwrap_or_else(|| {
        if requires_network {
            "cloud".to_string()
        } else {
            "local".to_string()
        }
    });
    let author = manifest_string(manifest, &["plugin", "author"])
        .unwrap_or_else(|| "Plugin Author".to_string());

    Ok(RegistryJsonOutput {
        name: plugin_name.to_string(),
        version,
        description,
        author,
        license,
        repository,
        binary,
        entry,
        platform,
        type_name,
        requires_network,
        homepage: manifest_string(manifest, &["release", "homepage"]),
        short_description: manifest_string(manifest, &["release", "short_description"]),
        icon: manifest_string(manifest, &["release", "icon"]),
        screenshots: manifest_string_array(manifest, &["release", "screenshots"]),
        plugin_api,
        capabilities,
        pubkey,
        checksums,
    })
}

fn platforms_from_checksums(
    checksums: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut values = Vec::new();
    for target in checksums.keys() {
        let platform = if target.contains("linux") {
            Some("linux")
        } else if target.contains("apple-darwin") {
            Some("macos")
        } else if target.contains("windows") {
            Some("windows")
        } else {
            None
        };
        if let Some(platform) = platform {
            if !values.iter().any(|value| value == platform) {
                values.push(platform.to_string());
            }
        }
    }
    values
}

fn release_checksums_from_dist(
    dist: &Path,
    plugin_name: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut checksums = std::collections::BTreeMap::new();
    if !dist.exists() {
        return Err("dist/ was not found.".to_string());
    }

    let prefix = format!("tinfo-{plugin_name}-");
    for entry in fs::read_dir(dist).map_err(|err| format!("Failed to read dist/: {err}"))? {
        let entry = entry.map_err(|err| format!("Failed to read dist/: {err}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with(&prefix) || !file_name.ends_with(".sha256") {
            continue;
        }

        let asset_name = file_name.trim_end_matches(".sha256");
        let target = asset_name
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(".tar.gz").or_else(|| value.strip_suffix(".zip")))
            .ok_or_else(|| format!("Unable to determine target triple from '{}'.", asset_name))?;
        let contents = fs::read_to_string(entry.path())
            .map_err(|err| format!("Failed to read {}: {err}", entry.path().display()))?;
        let checksum = contents
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("Malformed checksum file '{}'.", file_name))?;
        validate_sha256_hex(checksum)?;
        checksums.insert(target.to_string(), checksum.to_string());
    }

    if checksums.is_empty() {
        return Err(
            "No workflow release checksum files were found in dist/. Run this in CI after downloading build artifacts, or use `tinfo plugin pack` without `--from-dist`."
                .to_string(),
        );
    }

    Ok(checksums)
}

fn manifest_string(value: &toml::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::trim).filter(|value| !value.is_empty()).map(str::to_string)
}

fn manifest_string_array(value: &toml::Value, path: &[&str]) -> Vec<String> {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return Vec::new();
        };
        current = next;
    }

    current
        .as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| item.as_str().map(str::trim))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn read_release_pubkey(project_dir: &Path, manifest: &toml::Value) -> Result<String, String> {
    let pubkey_path = manifest_string(manifest, &["release", "pubkey_path"])
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("keys/minisign.pub"));
    let path = if pubkey_path.is_absolute() {
        pubkey_path
    } else {
        project_dir.join(pubkey_path)
    };
    fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read public key {}: {err}", path.display()))
        .map(|value| value.trim().to_string())
}

fn run_local_plugin_metadata(project_dir: &Path) -> Result<PluginRuntimeMetadata, String> {
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--quiet")
        .arg("--")
        .arg("--metadata")
        .current_dir(project_dir);
    for (key, value) in simulated_host_env() {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|err| format!("Failed to run local plugin metadata command: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "Local plugin metadata command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("Failed to parse plugin metadata JSON: {err}"))
}

fn run_local_plugin_preview(project_dir: &Path) -> Result<String, String> {
    let mut command = Command::new("cargo");
    command.arg("run").arg("--quiet").current_dir(project_dir);
    for (key, value) in simulated_host_env() {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|err| format!("Failed to run local plugin preview: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "Local plugin preview failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn simulated_host_env() -> Vec<(String, String)> {
    let mut items = vec![
        ("TINFO_HOST_VERSION".to_string(), env!("CARGO_PKG_VERSION").to_string()),
        (
            "TINFO_PLUGIN_DIR".to_string(),
            plugin_dir_path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| ".terminal-info/plugins".to_string()),
        ),
        (
            "TINFO_PLUGIN_CACHE_DIR".to_string(),
            plugin_cache_root()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| ".terminal-info/cache".to_string()),
        ),
        (
            "TINFO_CONFIG_PATH".to_string(),
            env::var("TINFO_CONFIG_DIR")
                .map(PathBuf::from)
                .map(|path| path.join("config.toml").display().to_string())
                .unwrap_or_else(|_| {
                    env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(".tinfo")
                        .join("config.toml")
                        .display()
                        .to_string()
                }),
        ),
    ];
    if let Ok(config) = crate::config::Config::load_or_create() {
        if let Ok(json) = serde_json::to_string(&config) {
            items.push(("TINFO_PLUGIN_CONFIG_JSON".to_string(), json));
        }
    }
    items
}

fn run_command(command: &mut Command, context: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|err| format!("{context}: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(format!("{context}: {detail}"))
    }
}

fn default_project_signing_key(project_dir: &Path) -> Result<PathBuf, String> {
    let candidates = [
        project_dir.join("minisign.key"),
        project_dir.join("keys").join("minisign.key"),
    ];
    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            "No Minisign secret key found. Create one with `tinfo plugin keygen` or place minisign.key in the project root.".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_file_check_marks_missing_files() {
        let check =
            project_file_check("plugin.toml", Path::new("/tmp/does-not-exist"), "create it");
        assert_eq!(check.status, "FAIL");
        assert_eq!(check.fix, "create it");
    }
}

pub fn dashboard_widgets(compact: bool) -> Vec<PluginWidget> {
    let Ok(installed) = installed_plugin_names() else {
        return Vec::new();
    };
    let mut widgets = Vec::new();
    for name in installed {
        if !matches!(is_plugin_trusted(&name), Ok(true)) {
            continue;
        }
        let binary = match find_in_plugin_dir(&format!("tinfo-{name}")) {
            Some(path) => path,
            None => continue,
        };
        let mut command = Command::new(binary);
        command.arg("--widget");
        if compact {
            command.arg("--compact");
        }
        let output = match command.output() {
            Ok(output) if output.status.success() => output,
            _ => continue,
        };
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(widget) = parse_dashboard_widget_payload(&text) {
            widgets.push(widget);
        }
    }
    widgets
}

fn parse_dashboard_widget_payload(text: &str) -> Option<PluginWidget> {
    serde_json::from_str::<PluginWidget>(text).ok().or_else(|| {
        serde_json::from_str::<LegacyPluginWidget>(text)
            .ok()
            .map(|legacy| PluginWidget {
                title: legacy.title,
                refresh_interval_secs: None,
                full: PluginWidgetBody::Text {
                    content: legacy.content.clone(),
                },
                compact: Some(PluginWidgetBody::Text {
                    content: legacy.content,
                }),
            })
    })
}

pub fn search_plugins(query: Option<&str>) -> Result<(), String> {
    let view = plugin_search_view(query)?;
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view)
                .unwrap_or_else(|_| "{\"installed\":[],\"available\":[]}".to_string())
        );
        return Ok(());
    }

    let has_query = query.map(|value| !value.trim().is_empty()).unwrap_or(false);
    if !has_query && view.installed.is_empty() && view.available.is_empty() {
        println!("No plugins available.");
        return Ok(());
    }

    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        println!("Plugin search for \"{}\"", query.trim());
        println!();
    } else {
        println!("Plugin catalog");
        println!();
    }

    if !view.installed.is_empty() {
        println!("Installed");
        for plugin in &view.installed {
            print_plugin_search_line(plugin);
        }
        println!();
    }

    if !view.available.is_empty() {
        println!("Available from registry");
        for plugin in &view.available {
            print_plugin_search_line(plugin);
        }
    } else if has_query {
        println!("No registry matches.");
    }

    Ok(())
}

pub fn registry_plugin_search_entries() -> Result<Vec<PluginSearchEntry>, String> {
    load_plugin_index()?
        .into_iter()
        .map(|entry| {
            let plugin = load_plugin_metadata(&entry)?;
            Ok(PluginSearchEntry {
                name: plugin.name,
                description: plugin.description,
                short_description: plugin.short_description,
                repository: plugin.repository,
                homepage: plugin.homepage,
                icon: plugin.icon,
                screenshots: plugin.screenshots,
                version: plugin.version,
                trusted: false,
                installed: false,
            })
        })
        .collect()
}

pub fn installed_plugin_search_entries() -> Result<Vec<PluginSearchEntry>, String> {
    let names = installed_plugin_names()?;
    let mut entries = Vec::new();

    for name in names {
        let manifest = read_installed_manifest(&name).ok();
        let description = manifest
            .as_ref()
            .and_then(|manifest| manifest.get("plugin"))
            .and_then(|plugin| plugin.get("description"))
            .and_then(|value| value.as_str())
            .unwrap_or("Installed plugin")
            .to_string();
        let version = manifest
            .as_ref()
            .and_then(|manifest| manifest.get("plugin"))
            .and_then(|plugin| plugin.get("version"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        entries.push(PluginSearchEntry {
            trusted: is_plugin_trusted(&name).unwrap_or(false),
            short_description: description.clone(),
            repository: String::new(),
            homepage: String::new(),
            icon: String::new(),
            screenshots: Vec::new(),
            version,
            installed: true,
            name,
            description,
        });
    }

    Ok(entries)
}

pub fn plugin_browse(no_open: bool) -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("Failed to start local plugin browser: {err}"))?;
    let address = listener
        .local_addr()
        .map_err(|err| format!("Failed to determine local plugin browser address: {err}"))?;
    let url = format!("http://{address}");

    println!("Plugin browser running at {url}");
    println!("Press Ctrl-C to stop the server.");

    if !no_open {
        if let Err(err) = open_browser(&url) {
            println!("Unable to open a browser automatically: {err}");
        }
    }

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("Plugin browser connection error: {err}");
                continue;
            }
        };
        if let Err(err) = handle_plugin_browser_request(&mut stream) {
            let _ = write_http_response(
                &mut stream,
                "500 Internal Server Error",
                "text/plain; charset=utf-8",
                &format!("Plugin browser error: {err}"),
            );
        }
    }

    Ok(())
}

fn plugin_search_view(query: Option<&str>) -> Result<PluginSearchView, String> {
    let query = query.map(str::trim).filter(|value| !value.is_empty());
    let mut registry = registry_plugin_search_entries()?;
    let mut installed = installed_plugin_search_entries()?;
    let installed_names = installed
        .iter()
        .map(|plugin| plugin.name.clone())
        .collect::<HashSet<_>>();

    for plugin in &registry {
        if let Some(installed_plugin) = installed.iter_mut().find(|item| item.name == plugin.name) {
            if installed_plugin.repository.is_empty() {
                installed_plugin.repository = plugin.repository.clone();
            }
            if installed_plugin.homepage.is_empty() {
                installed_plugin.homepage = plugin.homepage.clone();
            }
            if installed_plugin.icon.is_empty() {
                installed_plugin.icon = plugin.icon.clone();
            }
            if installed_plugin.screenshots.is_empty() {
                installed_plugin.screenshots = plugin.screenshots.clone();
            }
            if installed_plugin.version.is_empty() {
                installed_plugin.version = plugin.version.clone();
            }
            if installed_plugin.short_description == installed_plugin.description
                && !plugin.short_description.trim().is_empty()
            {
                installed_plugin.short_description = plugin.short_description.clone();
            }
        }
    }

    registry.retain(|plugin| !installed_names.contains(&plugin.name));

    if let Some(query) = query {
        installed = filter_and_rank_plugins(installed, query);
        registry = filter_and_rank_plugins(registry, query);
    } else {
        installed.sort_by(|a, b| a.name.cmp(&b.name));
        registry.sort_by(|a, b| a.name.cmp(&b.name));
    }

    Ok(PluginSearchView {
        query: query.map(str::to_string),
        installed,
        available: registry,
    })
}

fn filter_and_rank_plugins(
    plugins: Vec<PluginSearchEntry>,
    query: &str,
) -> Vec<PluginSearchEntry> {
    let mut scored = plugins
        .into_iter()
        .filter_map(|plugin| {
            let score = plugin_match_score(query, &plugin);
            (score > 0).then_some((score, plugin))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.installed.cmp(&left.installed))
            .then_with(|| right.trusted.cmp(&left.trusted))
            .then_with(|| left.name.cmp(&right.name))
    });
    scored.into_iter().map(|(_, plugin)| plugin).collect()
}

fn plugin_match_score(query: &str, plugin: &PluginSearchEntry) -> i32 {
    let query = query.to_ascii_lowercase();
    let name = plugin.name.to_ascii_lowercase();
    let short = plugin.short_description.to_ascii_lowercase();
    let description = plugin.description.to_ascii_lowercase();
    let homepage = plugin.homepage.to_ascii_lowercase();
    let mut score = 0;

    if name == query {
        score += 120;
    } else if name.starts_with(&query) {
        score += 90;
    } else if name.contains(&query) {
        score += 70;
    }

    if short.contains(&query) {
        score += 30;
    }
    if description.contains(&query) {
        score += 20;
    }
    if homepage.contains(&query) {
        score += 10;
    }

    for token in query.split_whitespace() {
        if name == token {
            score += 40;
        } else if name.starts_with(token) {
            score += 24;
        } else if name.contains(token) {
            score += 14;
        } else if short.contains(token) {
            score += 8;
        } else if description.contains(token) {
            score += 5;
        }
    }

    if plugin.installed {
        score += 14;
    }
    if plugin.trusted {
        score += 4;
    }

    score
}

fn print_plugin_search_line(plugin: &PluginSearchEntry) {
    let summary = if !plugin.short_description.trim().is_empty() {
        plugin.short_description.as_str()
    } else {
        plugin.description.as_str()
    };
    let mut flags = Vec::new();
    if !plugin.version.trim().is_empty() {
        flags.push(format!("v{}", plugin.version));
    }
    if plugin.trusted {
        flags.push("trusted".to_string());
    }
    if !plugin.homepage.trim().is_empty() {
        flags.push(plugin.homepage.clone());
    } else if !plugin.repository.trim().is_empty() {
        flags.push(plugin.repository.clone());
    }

    if flags.is_empty() {
        println!("  {:<18} {}", plugin.name, summary);
    } else {
        println!("  {:<18} {} [{}]", plugin.name, summary, flags.join(", "));
    }
}

fn handle_plugin_browser_request(stream: &mut TcpStream) -> Result<(), String> {
    let mut buffer = [0_u8; 8192];
    let read = stream
        .read(&mut buffer)
        .map_err(|err| format!("Failed to read plugin browser request: {err}"))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| "Malformed HTTP request.".to_string())?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" {
        return write_http_response(
            stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Only GET is supported.",
        );
    }

    let (path, query) = split_http_target(target);
    match path {
        "/" => {
            let search_query = http_query_value(query, "q");
            let body = render_plugin_browser_page(search_query.as_deref())?;
            write_http_response(stream, "200 OK", "text/html; charset=utf-8", &body)
        }
        "/install" => {
            let name = http_query_value(query, "name")
                .ok_or_else(|| "Missing plugin name.".to_string())?;
            let body = match install_plugin(&name) {
                Ok(()) => render_message_page(
                    "Plugin installed",
                    &format!(
                        "Installed plugin '{}'. Review it with `tinfo plugin info {}` and trust it explicitly before execution.",
                        name, name
                    ),
                ),
                Err(err) => render_message_page("Install failed", &err),
            };
            write_http_response(stream, "200 OK", "text/html; charset=utf-8", &body)
        }
        _ => write_http_response(
            stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found.",
        ),
    }
}

fn render_plugin_browser_page(query: Option<&str>) -> Result<String, String> {
    let view = plugin_search_view(query)?;
    let mut html = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Terminal Info Plugins</title>\
<style>body{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f4ee;color:#1d2a22;margin:0;padding:24px;}\
.wrap{max-width:1040px;margin:0 auto;}h1{margin:0 0 8px;font-size:32px;}p{color:#4c5a52;}\
form{margin:20px 0 24px;display:flex;gap:12px;}input{flex:1;padding:12px 14px;border:1px solid #b9c2ba;border-radius:10px;font-size:16px;}\
button,.button{display:inline-block;padding:10px 14px;border:none;border-radius:10px;background:#185c37;color:#fff;text-decoration:none;cursor:pointer;}\
.button.secondary{background:#d9dfd8;color:#203026;}section{margin:28px 0;}\
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:16px;}\
.card{background:#fff;border:1px solid #d6ddd6;border-radius:16px;padding:16px;box-shadow:0 8px 24px rgba(17,24,20,0.06);}\
.meta{font-size:13px;color:#5b685f;margin-top:10px;}img.icon{width:48px;height:48px;border-radius:12px;object-fit:cover;background:#eef2ee;border:1px solid #d6ddd6;}\
.hero{display:flex;gap:14px;align-items:center;} .links{margin-top:12px;display:flex;gap:10px;flex-wrap:wrap;}\
.shots{display:flex;gap:8px;overflow:auto;margin-top:12px;} .shots img{height:110px;border-radius:10px;border:1px solid #d6ddd6;}\
.empty{padding:16px;background:#fff;border:1px dashed #c5cec6;border-radius:12px;}</style></head><body><div class=\"wrap\">\
<h1>Terminal Info Plugins</h1><p>Local browser view for discovery and inspection. Installation here still follows the normal registry and trust model.</p>",
    );

    html.push_str("<form method=\"GET\" action=\"/\"><input name=\"q\" placeholder=\"Search plugins\" value=\"");
    html.push_str(&html_escape(query.unwrap_or("")));
    html.push_str("\"><button type=\"submit\">Search</button><a class=\"button secondary\" href=\"/\">Clear</a></form>");

    render_plugin_section(&mut html, "Installed", &view.installed, true);
    render_plugin_section(&mut html, "Available from registry", &view.available, false);
    html.push_str("</div></body></html>");
    Ok(html)
}

fn render_plugin_section(
    html: &mut String,
    title: &str,
    plugins: &[PluginSearchEntry],
    installed: bool,
) {
    html.push_str("<section><h2>");
    html.push_str(title);
    html.push_str("</h2>");
    if plugins.is_empty() {
        html.push_str("<div class=\"empty\">No plugins in this section.</div></section>");
        return;
    }

    html.push_str("<div class=\"grid\">");
    for plugin in plugins {
        html.push_str("<article class=\"card\">");
        html.push_str("<div class=\"hero\">");
        if !plugin.icon.trim().is_empty() {
            html.push_str("<img class=\"icon\" src=\"");
            html.push_str(&html_escape(&plugin.icon));
            html.push_str("\" alt=\"\">");
        }
        html.push_str("<div><strong>");
        html.push_str(&html_escape(&plugin.name));
        html.push_str("</strong><div class=\"meta\">");
        if !plugin.version.trim().is_empty() {
            html.push_str("v");
            html.push_str(&html_escape(&plugin.version));
        } else {
            html.push_str("version unknown");
        }
        if plugin.trusted {
            html.push_str(" · trusted");
        }
        html.push_str("</div></div></div><p>");
        html.push_str(&html_escape(if !plugin.short_description.trim().is_empty() {
            &plugin.short_description
        } else {
            &plugin.description
        }));
        html.push_str("</p>");
        if !plugin.description.trim().is_empty()
            && plugin.short_description.trim() != plugin.description.trim()
        {
            html.push_str("<div class=\"meta\">");
            html.push_str(&html_escape(&plugin.description));
            html.push_str("</div>");
        }
        html.push_str("<div class=\"links\">");
        if !installed {
            html.push_str("<a class=\"button\" href=\"/install?name=");
            html.push_str(&url_encode(&plugin.name));
            html.push_str("\">Install</a>");
        }
        if !plugin.homepage.trim().is_empty() {
            html.push_str("<a class=\"button secondary\" href=\"");
            html.push_str(&html_escape(&plugin.homepage));
            html.push_str("\">Homepage</a>");
        }
        if !plugin.repository.trim().is_empty() {
            html.push_str("<a class=\"button secondary\" href=\"");
            html.push_str(&html_escape(&plugin.repository));
            html.push_str("\">Repository</a>");
        }
        html.push_str("</div>");
        if !plugin.screenshots.is_empty() {
            html.push_str("<div class=\"shots\">");
            for screenshot in &plugin.screenshots {
                html.push_str("<img src=\"");
                html.push_str(&html_escape(screenshot));
                html.push_str("\" alt=\"plugin screenshot\">");
            }
            html.push_str("</div>");
        }
        html.push_str("</article>");
    }
    html.push_str("</div></section>");
}

fn render_message_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
<style>body{{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f4ee;color:#1d2a22;padding:32px;}}\
.card{{max-width:720px;margin:0 auto;background:#fff;border:1px solid #d6ddd6;border-radius:16px;padding:20px;}}\
a{{display:inline-block;margin-top:16px;color:#185c37;}}</style></head><body><div class=\"card\"><h1>{}</h1><p>{}</p><a href=\"/\">Back to plugin browser</a></div></body></html>",
        html_escape(title),
        html_escape(title),
        html_escape(body)
    )
}

fn write_http_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("Failed to write HTTP response: {err}"))
}

fn split_http_target(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    }
}

fn http_query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| url_decode(value))
    })
}

fn url_decode(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let mut iter = value.as_bytes().iter().copied();
    while let Some(byte) = iter.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let high = iter.next().unwrap_or(b'0');
                let low = iter.next().unwrap_or(b'0');
                let decoded = [high, low];
                if let Ok(hex) = std::str::from_utf8(&decoded) {
                    if let Ok(value) = u8::from_str_radix(hex, 16) {
                        bytes.push(value);
                    }
                }
            }
            other => bytes.push(other),
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{:02X}", byte).chars().collect(),
        })
        .collect()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("start").arg("").arg(url);
        command
    };

    let status = command
        .status()
        .map_err(|err| format!("Failed to open browser: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Browser launcher exited with a non-zero status.".to_string())
    }
}

pub fn install_plugin(name: &str) -> Result<(), String> {
    let plugin = load_plugin_by_name(name)?;
    install_or_update_plugin(&plugin, "Installed")
}

pub fn update_plugin(name: &str) -> Result<(), String> {
    let plugin = load_plugin_by_name(name)?;
    install_or_update_plugin(&plugin, "Updated")
}

pub fn upgrade_all_plugins() -> Result<(), String> {
    let installed = installed_plugin_names()?;
    if installed.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    let mut updated_any = false;
    for name in installed {
        match load_plugin_by_name(&name) {
            Ok(plugin) => {
                install_or_update_plugin(&plugin, "Updated")?;
                updated_any = true;
            }
            Err(_) => {
                println!("Skipping '{}': not found in plugin index.", name);
            }
        }
    }

    if !updated_any {
        println!("No indexed plugins were updated.");
    }

    Ok(())
}

pub fn remove_plugin(name: &str) -> Result<(), String> {
    validate_plugin_name(name)?;
    let plugin_home = plugin_home_path(name)?;
    let legacy_binary = plugin_dir_path()?.join(binary_filename(&format!("tinfo-{name}")));

    if plugin_home.exists() {
        fs::remove_dir_all(&plugin_home)
            .map_err(|err| format!("Failed to remove plugin '{}': {err}", name))?;
    } else if legacy_binary.exists() {
        fs::remove_file(&legacy_binary)
            .map_err(|err| format!("Failed to remove plugin '{}': {err}", name))?;
    } else {
        return Err(format!("Plugin '{}' is not installed.", name));
    }

    println!("Removed plugin '{}'.", name);
    Ok(())
}

pub fn list_plugins() -> Result<(), String> {
    let entries = installed_plugin_names()?;
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }
    if entries.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    for entry in entries {
        println!("{entry}");
    }

    Ok(())
}

pub fn init_plugin_template(name: Option<String>) -> Result<(), String> {
    let default_name = name.unwrap_or_default();
    let plugin_name = prompt_value("Plugin name", &default_name)?;
    let plugin_name = plugin_name.trim().to_string();
    validate_plugin_name(&plugin_name)?;

    let default_path = format!("./tinfo-{plugin_name}");
    let project_path = prompt_value("Project path", &default_path)?;
    let project_path = project_path.trim();
    if project_path.is_empty() {
        return Err("Project path cannot be empty.".to_string());
    }

    let default_description = format!("{plugin_name} tools for Terminal Info");
    let description = prompt_value("Description", &default_description)?;
    let description = description.trim().to_string();
    if description.is_empty() {
        return Err("Description cannot be empty.".to_string());
    }

    let directory = env::current_dir()
        .map_err(|err| format!("Failed to read current directory: {err}"))?
        .join(project_path);

    if directory.exists() {
        return Err(format!(
            "Target directory '{}' already exists.",
            directory.display()
        ));
    }

    fs::create_dir_all(directory.join("src"))
        .map_err(|err| format!("Failed to create plugin template: {err}"))?;
    fs::create_dir_all(directory.join("tests"))
        .map_err(|err| format!("Failed to create plugin test directory: {err}"))?;

    fs::write(
        directory.join("plugin.toml"),
        plugin_manifest_template(&plugin_name, &description),
    )
    .map_err(|err| format!("Failed to write plugin.toml: {err}"))?;
    fs::write(directory.join("Cargo.toml"), cargo_template(&plugin_name))
        .map_err(|err| format!("Failed to write Cargo.toml: {err}"))?;
    fs::write(
        directory.join("src").join("main.rs"),
        main_template(&plugin_name, &description),
    )
        .map_err(|err| format!("Failed to write src/main.rs: {err}"))?;
    fs::write(
        directory.join("README.md"),
        readme_template(&plugin_name, &description),
    )
    .map_err(|err| format!("Failed to write README.md: {err}"))?;
    fs::write(
        directory.join("tests").join("smoke.rs"),
        tests_template(&plugin_name),
    )
    .map_err(|err| format!("Failed to write tests/smoke.rs: {err}"))?;
    fs::create_dir_all(directory.join(".github").join("workflows"))
        .map_err(|err| format!("Failed to create workflow directory: {err}"))?;
    fs::write(
        directory
            .join(".github")
            .join("workflows")
            .join("release.yml"),
        workflow_template(&plugin_name),
    )
    .map_err(|err| format!("Failed to write .github/workflows/release.yml: {err}"))?;

    println!("Created plugin template at {}.", directory.display());
    println!("Next steps:");
    println!("  cd {}", directory.display());
    println!("  cargo run -- --help");
    println!("  cargo build --release");
    println!("  ./target/release/tinfo-{}", plugin_name);
    Ok(())
}

pub fn plugin_keygen(output_dir: Option<PathBuf>) -> Result<(), String> {
    let output_dir = match output_dir {
        Some(path) => path,
        None => {
            env::current_dir().map_err(|err| format!("Failed to read current directory: {err}"))?
        }
    };
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("Failed to create key output directory: {err}"))?;

    let secret_key = output_dir.join("minisign.key");
    let public_key = output_dir.join("minisign.pub");

    if secret_key.exists() || public_key.exists() {
        return Err(format!(
            "Refusing to overwrite existing Minisign keys in '{}'.",
            output_dir.display()
        ));
    }

    let secret_key_str = secret_key.to_string_lossy().to_string();
    let public_key_str = public_key.to_string_lossy().to_string();
    let args = [
        "-G".to_string(),
        "-W".to_string(),
        "-s".to_string(),
        secret_key_str,
        "-p".to_string(),
        public_key_str,
    ];
    run_minisign(&args)?;

    println!("Created Minisign keys:");
    println!("  {}", secret_key.display());
    println!("  {}", public_key.display());
    Ok(())
}

pub fn plugin_sign(file: &Path, key: Option<&Path>) -> Result<(), String> {
    if !file.exists() {
        return Err(format!("File '{}' does not exist.", file.display()));
    }
    if !file.is_file() {
        return Err(format!("'{}' is not a file.", file.display()));
    }

    let key_path = match key {
        Some(path) => path.to_path_buf(),
        None => env::current_dir()
            .map_err(|err| format!("Failed to read current directory: {err}"))?
            .join("minisign.key"),
    };

    if !key_path.exists() {
        return Err(format!(
            "Minisign secret key '{}' was not found.",
            key_path.display()
        ));
    }

    let signature_path = PathBuf::from(format!("{}.minisig", file.display()));
    let trusted_comment = file
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin-artifact")
        .to_string();

    let key_path_str = key_path.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let signature_path_str = signature_path.to_string_lossy().to_string();
    let args = [
        "-S".to_string(),
        "-s".to_string(),
        key_path_str,
        "-m".to_string(),
        file_str,
        "-x".to_string(),
        signature_path_str,
        "-t".to_string(),
        trusted_comment,
    ];
    run_minisign(&args)?;

    println!("Created plugin signature:");
    println!("  {}", signature_path.display());
    Ok(())
}

fn run_minisign(args: &[String]) -> Result<(), String> {
    let output = Command::new("minisign")
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                "minisign was not found in PATH. Install minisign and try again.".to_string()
            } else {
                format!("Failed to run minisign: {err}")
            }
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "minisign exited with an unknown error".to_string()
    };

    Err(format!("minisign failed: {detail}"))
}

fn prompt_value(prompt: &str, default: &str) -> Result<String, String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        let theme = ColorfulTheme::default();
        return Input::with_theme(&theme)
            .with_prompt(prompt)
            .with_initial_text(default.to_string())
            .interact_text()
            .map_err(|err| format!("Failed to read {}: {err}", prompt.to_ascii_lowercase()));
    }

    print!("{prompt}: ");
    io::Write::flush(&mut io::stdout()).map_err(|err| format!("Failed to flush stdout: {err}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("Failed to read {}: {err}", prompt.to_ascii_lowercase()))?;
    let value = input.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

pub fn run_diagnostic_plugins() -> Result<(), String> {
    let dir = plugin_dir_path()?;
    if dir.exists() && dir.is_dir() {
        println!("{} Plugin directory OK", success_prefix());
    } else {
        println!("{} Plugin directory missing", error_prefix());
        return Ok(());
    }

    let installed = installed_plugin_names()?;

    if installed.is_empty() {
        println!("{} No installed plugins", success_prefix());
        return Ok(());
    }

    for name in installed {
        let binary_path = plugin_home_path(&name)?.join(binary_filename(&format!("tinfo-{name}")));
        if !binary_path.exists() {
            let legacy_binary = dir.join(binary_filename(&format!("tinfo-{name}")));
            if !legacy_binary.exists() {
                println!("{} Plugin \"{name}\" missing binary", error_prefix());
                continue;
            }
        }

        match load_plugin_by_name(&name) {
            Ok(plugin) if plugin_binary_name(&plugin) == format!("tinfo-{name}") => {
                println!("{} Plugin \"{name}\" metadata OK", success_prefix());
            }
            _ => println!("{} Plugin \"{name}\" version mismatch", error_prefix()),
        }
    }

    let summary = plugin_diagnostic_summary()?;
    if summary.unknown_plugins.is_empty() {
        println!("{} No unknown plugins", success_prefix());
    } else {
        println!(
            "{} Unknown plugins: {}",
            error_prefix(),
            summary.unknown_plugins.join(", ")
        );
    }
    if summary.broken_paths.is_empty() {
        println!("{} No broken plugin paths", success_prefix());
    } else {
        for path in summary.broken_paths {
            println!("{} Broken plugin path: {}", error_prefix(), path);
        }
    }

    Ok(())
}

pub fn plugin_diagnostic_summary() -> Result<PluginDiagnosticSummary, String> {
    let installed = installed_plugin_names()?;
    let index = load_plugin_index().unwrap_or_default();
    let known: HashSet<_> = index.iter().map(|plugin| plugin.name.as_str()).collect();

    let mut unknown_plugins = Vec::new();
    let mut broken_paths = Vec::new();

    for name in &installed {
        if !known.contains(name.as_str()) {
            unknown_plugins.push(name.clone());
        }

        let home = plugin_home_path(name)?;
        let binary = home.join(binary_filename(&format!("tinfo-{name}")));
        let manifest = plugin_manifest_path(name)?;

        if home.exists() {
            if !binary.exists() {
                broken_paths.push(binary.display().to_string());
            }
            if !manifest.exists() {
                broken_paths.push(manifest.display().to_string());
            }
        } else {
            let legacy_binary = plugin_dir_path()?.join(binary_filename(&format!("tinfo-{name}")));
            if !legacy_binary.exists() {
                broken_paths.push(home.display().to_string());
            }
        }
    }

    Ok(PluginDiagnosticSummary {
        unknown_plugins,
        broken_paths,
    })
}

fn install_or_update_plugin(plugin: &PluginMetadata, action: &str) -> Result<(), String> {
    let plugin_home = plugin_home_path(&plugin.name)?;
    fs::create_dir_all(&plugin_home)
        .map_err(|err| format!("Failed to create plugin directory: {err}"))?;

    let (owner, repo) = parse_github_repo(&plugin.repository)?;
    let binary = plugin_binary_name(plugin);
    let asset_name = release_asset_name(&binary);
    let asset_url = release_download_url(&owner, &repo, &plugin.version, &asset_name);
    let signature_url = release_download_url(
        &owner,
        &repo,
        &plugin.version,
        &format!("{asset_name}.minisig"),
    );

    let bytes = download_binary_bytes(&asset_url, "plugin asset")?;
    let signature = github_client()?
        .get(&signature_url)
        .send()
        .map_err(|err| format!("Failed to download plugin signature: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download plugin signature: {err}"))?
        .text()
        .map_err(|err| format!("Failed to read plugin signature: {err}"))?;

    verify_plugin_checksum(plugin, bytes.as_ref())?;
    verify_minisign_signature(bytes.as_ref(), &signature, &plugin.pubkey)
        .map_err(|err| format!("Plugin signature verification failed: {err}"))?;

    let destination = plugin_home.join(binary_filename(&binary));
    extract_asset(&asset_name, &binary, bytes.as_ref(), &destination)?;
    set_executable(&destination)?;
    write_plugin_manifest(plugin, &plugin.version, &sha256_hex(bytes.as_ref()))?;

    let legacy_path = plugin_dir_path()?.join(binary_filename(&binary));
    if legacy_path.exists() && legacy_path != destination {
        let _ = fs::remove_file(legacy_path);
    }

    println!(
        "{action} plugin '{}' at {}.",
        plugin.name,
        destination.display()
    );
    Ok(())
}

fn resolve_plugin_binary(binary_name: &str) -> Option<PathBuf> {
    find_in_plugin_dir(binary_name)
}

fn find_in_plugin_dir(binary_name: &str) -> Option<PathBuf> {
    let dir = plugin_dir_path().ok()?;
    let name = binary_name.strip_prefix("tinfo-").unwrap_or(binary_name);
    let candidate = dir.join(name).join(binary_filename(binary_name));
    if is_executable_file(&candidate) {
        return Some(candidate);
    }

    let legacy_candidate = dir.join(binary_filename(binary_name));
    if is_executable_file(&legacy_candidate) {
        return Some(legacy_candidate);
    }

    None
}

fn plugin_dir_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Ok(home_dir_path().join(".terminal-info").join("plugins"))
}

fn plugin_cache_root() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("TINFO_PLUGIN_CACHE_DIR") {
        return Ok(PathBuf::from(path));
    }

    Ok(home_dir_path().join(".terminal-info").join("cache"))
}

fn plugin_index_cache_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("TINFO_PLUGIN_CACHE_PATH") {
        return Ok(PathBuf::from(path));
    }

    Ok(plugin_cache_root()?.join("plugin-index.json"))
}

fn plugin_registry_cache_path(name: &str) -> Result<PathBuf, String> {
    Ok(plugin_cache_root()?.join("plugins").join(format!("{name}.json")))
}

fn plugin_index_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_INDEX_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Err("No local plugin index override configured. Falling back to raw registry.".to_string())
}

fn load_plugin_index() -> Result<Vec<PluginIndexEntry>, String> {
    if let Ok(dir) = plugin_index_dir() {
        return load_plugin_index_from_local_dir(&dir);
    }

    load_plugin_index_cached()
}

fn load_plugin_index_from_local_dir(dir: &Path) -> Result<Vec<PluginIndexEntry>, String> {
    let index_path = dir.join("index.json");
    let contents = fs::read_to_string(&index_path)
        .map_err(|err| format!("Failed to read {}: {err}", index_path.display()))?;
    let index: PluginIndexFile = serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse {}: {err}", index_path.display()))?;
    validate_plugin_index(&index)?;
    Ok(index
        .plugins
        .into_iter()
        .map(|mut plugin| {
            if !plugin.registry.starts_with("http://")
                && !plugin.registry.starts_with("https://")
            {
                plugin.registry = dir.join(&plugin.registry).display().to_string();
            }
            plugin
        })
        .collect())
}

fn load_plugin_index_cached() -> Result<Vec<PluginIndexEntry>, String> {
    let cache_path = plugin_index_cache_path()?;
    let cache = read_plugin_index_cache(&cache_path).ok();

    if let Some(cache) = cache.as_ref() {
        if !cache_is_expired(cache.fetched_at, INDEX_CACHE_TTL_SECS) {
            return Ok(cache.index.plugins.clone());
        }
    }

    match fetch_plugin_index_from_registry() {
        Ok(index) => {
            write_plugin_index_cache(&cache_path, &index)?;
            Ok(index.plugins)
        }
        Err(err) => {
            if let Some(cache) = cache {
                Ok(cache.index.plugins)
            } else {
                Err(err)
            }
        }
    }
}

fn fetch_plugin_index_from_registry() -> Result<PluginIndexFile, String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/plugins/index.json",
        REGISTRY_OWNER, REGISTRY_REPO, REGISTRY_BRANCH
    );
    let contents = github_client()?
        .get(url)
        .send()
        .map_err(|err| format!("Failed to fetch plugin index: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to fetch plugin index: {err}"))?
        .text()
        .map_err(|err| format!("Failed to read plugin index: {err}"))?;
    let index: PluginIndexFile = serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse plugin index: {err}"))?;
    validate_plugin_index(&index)?;
    Ok(index)
}

fn load_plugin_by_name(name: &str) -> Result<PluginMetadata, String> {
    let entry = load_plugin_index()?
        .into_iter()
        .find(|plugin| plugin.name == name)
        .ok_or_else(|| format!("Plugin '{}' not found in plugin index.", name))?;
    load_plugin_metadata(&entry)
}

fn load_plugin_metadata(entry: &PluginIndexEntry) -> Result<PluginMetadata, String> {
    let cache_path = plugin_registry_cache_path(&entry.name)?;
    let cache = read_plugin_metadata_cache(&cache_path).ok();

    if let Some(cache) = cache.as_ref() {
        if !cache_is_expired(cache.fetched_at, PLUGIN_CACHE_TTL_SECS) {
            return Ok(cache.plugin.clone());
        }
    }

    match fetch_plugin_metadata(entry) {
        Ok(plugin) => {
            write_plugin_metadata_cache(&cache_path, &plugin)?;
            Ok(plugin)
        }
        Err(err) => {
            if let Some(cache) = cache {
                Ok(cache.plugin)
            } else {
                Err(err)
            }
        }
    }
}

fn fetch_plugin_metadata(entry: &PluginIndexEntry) -> Result<PluginMetadata, String> {
    let contents = if entry.registry.starts_with("http://") || entry.registry.starts_with("https://")
    {
        github_client()?
            .get(&entry.registry)
            .send()
            .map_err(|err| format!("Failed to fetch plugin metadata '{}': {err}", entry.name))?
            .error_for_status()
            .map_err(|err| format!("Failed to fetch plugin metadata '{}': {err}", entry.name))?
            .text()
            .map_err(|err| format!("Failed to read plugin metadata '{}': {err}", entry.name))?
    } else {
        fs::read_to_string(&entry.registry)
            .map_err(|err| format!("Failed to read plugin metadata '{}': {err}", entry.name))?
    };

    let plugin: PluginMetadata = serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse plugin metadata '{}': {err}", entry.name))?;
    if plugin.name != entry.name {
        return Err(format!(
            "Plugin registry name mismatch: index entry '{}' points to '{}'.",
            entry.name, plugin.name
        ));
    }
    validate_plugin_metadata(&plugin)?;
    Ok(plugin)
}

fn read_plugin_index_cache(path: &Path) -> Result<PluginIndexCache, String> {
    let contents =
        fs::read_to_string(path).map_err(|err| format!("Failed to read plugin index cache: {err}"))?;
    serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse plugin index cache: {err}"))
}

fn write_plugin_index_cache(path: &Path, index: &PluginIndexFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create plugin index cache directory: {err}"))?;
    }

    let payload = PluginIndexCache {
        fetched_at: now_unix(),
        index: index.clone(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Failed to serialize plugin index cache: {err}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|err| format!("Failed to write plugin index cache: {err}"))
}

fn read_plugin_metadata_cache(path: &Path) -> Result<PluginMetadataCache, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read plugin metadata cache: {err}"))?;
    serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse plugin metadata cache: {err}"))
}

fn write_plugin_metadata_cache(path: &Path, plugin: &PluginMetadata) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create plugin metadata cache directory: {err}"))?;
    }

    let payload = PluginMetadataCache {
        fetched_at: now_unix(),
        plugin: plugin.clone(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Failed to serialize plugin metadata cache: {err}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|err| format!("Failed to write plugin metadata cache: {err}"))
}

fn cache_is_expired(fetched_at: u64, ttl_secs: u64) -> bool {
    now_unix().saturating_sub(fetched_at) > ttl_secs
}

fn validate_plugin_index(index: &PluginIndexFile) -> Result<(), String> {
    if index.version == 0 {
        return Err("Plugin index version must be non-zero.".to_string());
    }

    let mut seen = HashSet::new();
    for plugin in &index.plugins {
        validate_plugin_name(&plugin.name)?;
        if plugin.registry.trim().is_empty() {
            return Err(format!(
                "Plugin '{}' is missing a registry URL.",
                plugin.name
            ));
        }
        if !seen.insert(plugin.name.clone()) {
            return Err(format!(
                "Duplicate plugin name '{}' in plugin index.",
                plugin.name
            ));
        }
    }
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn validate_plugin_metadata(plugin: &PluginMetadata) -> Result<(), String> {
    validate_plugin_name(&plugin.name)?;

    if plugin.name.trim().is_empty()
        || plugin.author.trim().is_empty()
        || plugin.license.trim().is_empty()
        || plugin.description.trim().is_empty()
        || plugin.repository.trim().is_empty()
        || plugin.binary.trim().is_empty()
        || plugin.entry.trim().is_empty()
        || plugin.version.trim().is_empty()
    {
        return Err(format!(
            "Plugin '{}' has missing required fields.",
            plugin.name
        ));
    }

    if RESERVED_COMMANDS.contains(&plugin.name.as_str()) {
        return Err(format!(
            "Plugin '{}' conflicts with a reserved built-in command.",
            plugin.name
        ));
    }

    if !plugin.repository.starts_with("https://github.com/") {
        return Err(format!(
            "Plugin '{}' must use a GitHub repository URL.",
            plugin.name
        ));
    }

    match plugin.license.as_str() {
        "MIT" | "Apache-2.0" => {}
        _ => {
            return Err(format!(
                "Plugin '{}' must use license MIT or Apache-2.0.",
                plugin.name
            ));
        }
    }

    if !plugin
        .name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "Plugin '{}' must use lowercase ASCII letters, digits, '-' or '_'.",
            plugin.name
        ));
    }

    if plugin.platform.is_empty() {
        return Err(format!(
            "Plugin '{}' must declare at least one platform.",
            plugin.name
        ));
    }

    for platform in &plugin.platform {
        match platform.as_str() {
            "linux" | "macos" | "windows" => {}
            _ => {
                return Err(format!(
                    "Plugin '{}' has unsupported platform '{}'.",
                    plugin.name, platform
                ));
            }
        }
    }

    match plugin.r#type.as_str() {
        "local" | "cloud" => {}
        _ => {
            return Err(format!(
                "Plugin '{}' must set type to 'local' or 'cloud'.",
                plugin.name
            ));
        }
    }

    if plugin.version == "latest" {
        return Err(format!(
            "Plugin '{}' must pin an exact reviewed version, not 'latest'.",
            plugin.name
        ));
    }

    if plugin.plugin_api == 0 {
        return Err(format!(
            "Plugin '{}' must declare a non-zero plugin API version.",
            plugin.name
        ));
    }

    let checksum = plugin.checksums.get(target_triple()).ok_or_else(|| {
        format!(
            "Plugin '{}' is missing a checksum for '{}'.",
            plugin.name,
            target_triple()
        )
    })?;
    validate_sha256_hex(checksum).map_err(|err| format!("Plugin '{}': {err}", plugin.name))?;
    if plugin.pubkey.trim().is_empty() {
        return Err(format!(
            "Plugin '{}' is missing a minisign public key.",
            plugin.name
        ));
    }

    for capability in &plugin.capabilities {
        validate_capability(capability)
            .map_err(|err| format!("Plugin '{}': {err}", plugin.name))?;
    }

    Ok(())
}

fn validate_plugin_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Plugin name cannot be empty.".to_string());
    }

    if RESERVED_COMMANDS.contains(&name) {
        return Err(format!(
            "Plugin '{}' conflicts with a reserved built-in command.",
            name
        ));
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "Plugin '{}' must use lowercase ASCII letters, digits, '-' or '_'.",
            name
        ));
    }

    Ok(())
}

fn validate_capability(value: &str) -> Result<(), String> {
    match value {
        "network" | "config" | "cache" | "filesystem" => Ok(()),
        _ => Err(format!(
            "unsupported capability '{}'; expected one of network, config, cache, filesystem.",
            value
        )),
    }
}

fn parse_github_repo(url: &str) -> Result<(String, String), String> {
    let trimmed = url.trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() < 5 {
        return Err(format!("Invalid GitHub repository URL: {url}"));
    }

    Ok((
        parts[parts.len() - 2].to_string(),
        parts[parts.len() - 1].to_string(),
    ))
}

fn release_asset_name(binary: &str) -> String {
    let target = target_triple();
    if target.contains("windows") {
        format!("{binary}-{target}.zip")
    } else {
        format!("{binary}-{target}.tar.gz")
    }
}

fn release_download_url(owner: &str, repo: &str, version: &str, asset_name: &str) -> String {
    format!("https://github.com/{owner}/{repo}/releases/download/{version}/{asset_name}")
}

fn github_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))
}

fn download_binary_bytes(url: &str, label: &str) -> Result<Vec<u8>, String> {
    let mut response = github_client()?
        .get(url)
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .map_err(|err| format!("Failed to download {label}: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download {label}: {err}"))?;

    let mut bytes = Vec::new();
    response
        .copy_to(&mut bytes)
        .map_err(|err| format!("Failed to read {label}: {err}"))?;
    Ok(bytes)
}

fn extract_asset(
    asset_name: &str,
    binary: &str,
    bytes: &[u8],
    destination: &Path,
) -> Result<(), String> {
    if asset_name.ends_with(".tar.gz") {
        extract_tar_gz(binary, bytes, destination)
    } else if asset_name.ends_with(".zip") {
        extract_zip(binary, bytes, destination)
    } else {
        fs::write(destination, bytes).map_err(|err| format!("Failed to write plugin binary: {err}"))
    }
}

fn extract_tar_gz(binary: &str, bytes: &[u8], destination: &Path) -> Result<(), String> {
    let expected = binary_filename(binary);
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|err| format!("Failed to read tar archive: {err}"))?;

    for entry in entries {
        let mut entry = entry.map_err(|err| format!("Failed to read tar entry: {err}"))?;
        let path = entry
            .path()
            .map_err(|err| format!("Failed to read tar entry path: {err}"))?;
        if path.file_name().and_then(|name| name.to_str()) == Some(expected.as_str()) {
            let mut output = File::create(destination)
                .map_err(|err| format!("Failed to create plugin binary: {err}"))?;
            io::copy(&mut entry, &mut output)
                .map_err(|err| format!("Failed to extract plugin binary: {err}"))?;
            return Ok(());
        }
    }

    Err(format!("Archive did not contain binary '{}'.", expected))
}

fn extract_zip(binary: &str, bytes: &[u8], destination: &Path) -> Result<(), String> {
    let expected = binary_filename(binary);
    let reader = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|err| format!("Failed to read zip archive: {err}"))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("Failed to read zip entry: {err}"))?;
        let name = Path::new(file.name())
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();

        if name == expected {
            let mut output = File::create(destination)
                .map_err(|err| format!("Failed to create plugin binary: {err}"))?;
            io::copy(&mut file, &mut output)
                .map_err(|err| format!("Failed to extract plugin binary: {err}"))?;
            return Ok(());
        }
    }

    Err(format!("Archive did not contain binary '{}'.", expected))
}

fn binary_filename(binary: &str) -> String {
    #[cfg(windows)]
    {
        if binary.ends_with(".exe") {
            binary.to_string()
        } else {
            format!("{binary}.exe")
        }
    }

    #[cfg(not(windows))]
    {
        binary.to_string()
    }
}

fn plugin_binary_name(plugin: &PluginMetadata) -> String {
    if plugin.binary.trim().is_empty() {
        format!("tinfo-{}", plugin.name)
    } else {
        plugin.binary.clone()
    }
}

fn verify_plugin_checksum(plugin: &PluginMetadata, bytes: &[u8]) -> Result<(), String> {
    let expected = plugin.checksums.get(target_triple()).ok_or_else(|| {
        format!(
            "Plugin '{}' is missing a checksum for '{}'.",
            plugin.name,
            target_triple()
        )
    })?;
    let actual = sha256_hex(bytes);
    if &actual != expected {
        return Err(format!(
            "Checksum verification failed for plugin '{}'.",
            plugin.name
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn validate_sha256_hex(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("checksum must be a 64-character SHA-256 hex string.".to_string());
    }
    Ok(())
}

fn verify_minisign_signature(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
) -> Result<(), String> {
    let key = PublicKey::from_base64(public_key)
        .map_err(|err| format!("invalid minisign public key: {err}"))?;
    let sig =
        Signature::decode(signature).map_err(|err| format!("invalid minisign signature: {err}"))?;
    key.verify(bytes, &sig, false)
        .map_err(|err| format!("signature mismatch: {err}"))
}

fn target_triple() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    {
        "unknown-target"
    }
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata =
            fs::metadata(path).map_err(|err| format!("Failed to read permissions: {err}"))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|err| format!("Failed to set plugin executable permissions: {err}"))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn installed_plugin_names() -> Result<Vec<String>, String> {
    let dir = plugin_dir_path()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("Failed to read plugins: {err}"))? {
        let entry = entry.map_err(|err| format!("Failed to read plugins: {err}"))?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            let manifest_path = path.join("plugin.toml");
            if manifest_path.exists() {
                if let Ok(contents) = fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = toml::from_str::<toml::Value>(&contents) {
                        if let Some(name) = manifest
                            .get("plugin")
                            .and_then(|section| section.get("name"))
                            .and_then(|value| value.as_str())
                        {
                            names.push(name.to_string());
                            continue;
                        }
                    }
                }
            }

            names.push(file_name);
            continue;
        }

        if let Some(stripped) = file_name.strip_prefix("tinfo-") {
            let stripped = stripped.strip_suffix(".exe").unwrap_or(stripped);
            names.push(stripped.to_string());
        }
    }

    names.sort();
    names.dedup();
    Ok(names)
}

fn plugin_home_path(name: &str) -> Result<PathBuf, String> {
    Ok(plugin_dir_path()?.join(name))
}

fn plugin_manifest_path(name: &str) -> Result<PathBuf, String> {
    Ok(plugin_home_path(name)?.join("plugin.toml"))
}

fn write_plugin_manifest(
    plugin: &PluginMetadata,
    version: &str,
    asset_checksum: &str,
) -> Result<(), String> {
    let manifest = PluginManifest {
        plugin: PluginSection {
            name: plugin.name.clone(),
            version: version.to_string(),
            description: plugin.description.clone(),
            author: if plugin.author.trim().is_empty() {
                None
            } else {
                Some(plugin.author.clone())
            },
        },
        command: CommandSection {
            name: plugin.name.clone(),
        },
        compatibility: CompatibilitySection {
            terminal_info: format!(">={}", env!("CARGO_PKG_VERSION")),
            plugin_api: plugin.plugin_api,
        },
        requirements: if plugin.capabilities.is_empty() {
            None
        } else {
            Some(RequirementsSection {
                capabilities: plugin.capabilities.clone(),
            })
        },
        install: Some(InstallSection {
            version: version.to_string(),
            target: target_triple().to_string(),
            asset_checksum: asset_checksum.to_string(),
        }),
    };

    let toml = toml::to_string_pretty(&manifest)
        .map_err(|err| format!("Failed to serialize plugin manifest: {err}"))?;
    fs::write(plugin_manifest_path(&plugin.name)?, format!("{toml}\n"))
        .map_err(|err| format!("Failed to write plugin manifest: {err}"))
}

fn plugin_manifest_template(name: &str, description: &str) -> String {
    format!(
        r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "{description}"
author = "Plugin Author"

[command]
name = "{name}"

[compatibility]
terminal_info = ">={version}"
plugin_api = 1

[requirements]
capabilities = ["config", "cache"]

[release]
repository = "https://github.com/OWNER/tinfo-{name}"
pubkey_path = "keys/minisign.pub"
short_description = "{description}"
"#,
        version = env!("CARGO_PKG_VERSION")
    )
}

fn cargo_template(name: &str) -> String {
    format!(
        r#"[package]
name = "tinfo-{name}"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "tinfo-{name}"
path = "src/main.rs"

[dependencies]
tinfo-plugin = {{ git = "https://github.com/T-1234567890/terminal-info", package = "tinfo-plugin", tag = "{version}" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#,
        version = env!("CARGO_PKG_VERSION")
    )
}

fn main_template(name: &str, description: &str) -> String {
    format!(
        r#"use serde::Serialize;
use tinfo_plugin::{{Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel, Table}};

#[derive(Serialize)]
struct InspectView {{
    plugin: &'static str,
    host_version: String,
    configured_location: Option<String>,
}}

fn status(ctx: tinfo_plugin::Context, args: CommandInput) -> PluginResult<()> {{
    let location = args
        .option("--city")
        .map(str::to_string)
        .or(ctx.config.string("location")?)
        .unwrap_or_else(|| "auto".to_string());
    ctx.cache.write_string("last-city", &location)?;

    ctx.output().section("Status");
    ctx.output().status(StatusLevel::Ok, format!("plugin {name} is ready"));
    ctx.output().kv("Location", &location);
    ctx.output().kv("Host", ctx.host.version());
    ctx.output().table(
        Table::new(["Field", "Value"])
            .row(["OS", ctx.system.os()])
            .row(["Arch", ctx.system.arch()])
            .row(["Cache", &ctx.cache.plugin_dir().display().to_string()]),
    );
    ctx.output().list([
        "Try `tinfo {name} inspect` for JSON output",
        "Pass `--city <name>` to override the configured location",
    ]);
    Ok(())
}}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {{
    ctx.output().section("Inspect");
    ctx.output().progress("collecting plugin state");
    ctx.output().json(&InspectView {{
        plugin: "{name}",
        host_version: ctx.host.version(),
        configured_location: ctx.config.string("location")?,
    }})?;
    Ok(())
}}

fn main() {{
    Plugin::new("{name}")
        .description("{description}")
        .author("Plugin Author")
        .compatibility(">={version}")
        .capability(Capability::Config)
        .capability(Capability::Cache)
        .command(
            PluginCommand::new("status")
                .description("Show the plugin status using SDK output helpers")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Print a JSON inspection view")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}}
"#,
        name = name,
        description = description,
        version = env!("CARGO_PKG_VERSION")
    )
}

fn readme_template(name: &str, description: &str) -> String {
    format!(
        r#"# tinfo-{name}

{description}

This plugin uses the Terminal Info SDK crate `tinfo-plugin`.

## Build

```bash
cargo build --release
```

## Run With Terminal Info

```bash
tinfo {name}
```

Terminal Info will route `tinfo {name}` to the `tinfo-{name}` executable.

## Inspect Metadata

```bash
cargo run -- --metadata
cargo run -- --manifest
```

## Local Plugin Development

```bash
tinfo plugin inspect
tinfo plugin test
tinfo plugin pack
```

## Test

```bash
cargo test
```

## Submit To The Plugin Registry

1. Publish a GitHub release for this plugin
2. Download the generated registry JSON artifact or use `dist/registry/{name}.json`
3. Add or update `plugins/{name}.json` in the Terminal Info repository
4. Open a pull request for registry review
"#
    )
}

fn tests_template(name: &str) -> String {
    format!(
        r#"use serde_json::json;
use tinfo_plugin::{{testing::{{MockHost, TestRunner}}, Capability, CommandInput, Plugin, PluginCommand, PluginResult}};

fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {{
    let location = ctx.config.string("location")?.unwrap_or_else(|| "unknown".to_string());
    ctx.output().kv("Location", location);
    Ok(())
}}

#[test]
fn status_command_reads_typed_config() {{
    let plugin = Plugin::new("{name}")
        .capability(Capability::Config)
        .command(PluginCommand::new("status").handler(status));

    let run = TestRunner::new(plugin)
        .host(MockHost::default().config_json(json!({{ "location": "tokyo" }})))
        .args(["status"])
        .run()
        .expect("plugin should run");

    assert!(run.stdout.contains("Location: tokyo"));
}}
"#
    )
}

fn workflow_template(name: &str) -> String {
    format!(
        r#"name: Release

on:
  push:
    tags:
      - "*.*.*"

permissions:
  contents: write

jobs:
  build:
    name: Build ${{{{ matrix.target }}}}
    runs-on: ${{{{ matrix.os }}}}
    env:
      MINISIGN_SECRET_KEY: ${{{{ secrets.MINISIGN_SECRET_KEY }}}}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            binary_name: tinfo-{name}
          - os: macos-latest
            target: x86_64-apple-darwin
            binary_name: tinfo-{name}
          - os: macos-latest
            target: aarch64-apple-darwin
            binary_name: tinfo-{name}
          - os: windows-2022
            target: x86_64-pc-windows-msvc
            binary_name: tinfo-{name}.exe

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{{{ matrix.target }}}}

      - name: Build release binary
        run: cargo build --release --target ${{{{ matrix.target }}}}

      - name: Install minisign
        if: ${{{{ runner.os != 'Windows' && env.MINISIGN_SECRET_KEY != '' }}}}
        run: |
          if [ -z "$MINISIGN_SECRET_KEY" ]; then
            echo "MINISIGN_SECRET_KEY is required for plugin release signing."
            exit 1
          fi
          if command -v minisign >/dev/null 2>&1; then
            exit 0
          fi
          if command -v brew >/dev/null 2>&1; then
            brew install minisign
          elif command -v apt-get >/dev/null 2>&1; then
            sudo apt-get update
            sudo apt-get install -y minisign || {{
              sudo apt-get install -y build-essential pkg-config libssl-dev libsodium-dev git
              git clone --depth 1 https://github.com/jedisct1/minisign.git /tmp/minisign-src
              make -C /tmp/minisign-src
              sudo install /tmp/minisign-src/minisign /usr/local/bin/minisign
            }}
          else
            echo "Unable to install minisign on this runner."
            exit 1
          fi

      - name: Install minisign (Windows)
        if: ${{{{ runner.os == 'Windows' && env.MINISIGN_SECRET_KEY != '' }}}}
        shell: pwsh
        run: |
          if (-not $env:MINISIGN_SECRET_KEY) {{
            throw "MINISIGN_SECRET_KEY is required for plugin release signing."
          }}
          $zipUrl = "https://github.com/jedisct1/minisign/releases/download/0.11/minisign-0.11-win64.zip"
          Invoke-WebRequest $zipUrl -OutFile minisign.zip
          Expand-Archive minisign.zip -DestinationPath minisign
          $minisignExe = Get-ChildItem -Path minisign -Recurse -Filter minisign.exe | Select-Object -First 1
          echo $minisignExe.DirectoryName >> $env:GITHUB_PATH

      - name: Package asset (Unix)
        if: runner.os != 'Windows'
        run: |
          mkdir -p dist
          mkdir -p bundle
          cp target/${{{{ matrix.target }}}}/release/${{{{ matrix.binary_name }}}} bundle/${{{{ matrix.binary_name }}}}
          cp plugin.toml bundle/plugin.toml
          tar -czf dist/tinfo-{name}-${{{{ matrix.target }}}}.tar.gz -C bundle .
          shasum -a 256 dist/tinfo-{name}-${{{{ matrix.target }}}}.tar.gz > dist/tinfo-{name}-${{{{ matrix.target }}}}.tar.gz.sha256
          printf '%s' "$MINISIGN_SECRET_KEY" > minisign.key
          chmod 600 minisign.key
          minisign -S -s minisign.key -m dist/tinfo-{name}-${{{{ matrix.target }}}}.tar.gz -x dist/tinfo-{name}-${{{{ matrix.target }}}}.tar.gz.minisig -t "tinfo-{name}-${{{{ matrix.target }}}}.tar.gz"

      - name: Package asset (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          New-Item -ItemType Directory -Force -Path dist | Out-Null
          New-Item -ItemType Directory -Force -Path bundle | Out-Null
          Copy-Item "target/${{{{ matrix.target }}}}/release/${{{{ matrix.binary_name }}}}" "bundle/${{{{ matrix.binary_name }}}}"
          Copy-Item "plugin.toml" "bundle/plugin.toml"
          Compress-Archive -Path "bundle/*" -DestinationPath "dist/tinfo-{name}-${{{{ matrix.target }}}}.zip" -Force
          $hash = (Get-FileHash "dist/tinfo-{name}-${{{{ matrix.target }}}}.zip" -Algorithm SHA256).Hash.ToLower()
          Set-Content -Path "dist/tinfo-{name}-${{{{ matrix.target }}}}.zip.sha256" -Value "$hash  tinfo-{name}-${{{{ matrix.target }}}}.zip"
          if ($env:MINISIGN_SECRET_KEY) {{
            $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
            [System.IO.File]::WriteAllText("minisign.key", $env:MINISIGN_SECRET_KEY, $utf8NoBom)
            minisign -S -s minisign.key -m "dist/tinfo-{name}-${{{{ matrix.target }}}}.zip" -x "dist/tinfo-{name}-${{{{ matrix.target }}}}.zip.minisig" -t "tinfo-{name}-${{{{ matrix.target }}}}.zip"
          }}

      - name: Upload release artifact bundle
        uses: actions/upload-artifact@v4
        with:
          name: plugin-${{{{ matrix.target }}}}
          path: dist/*

  registry:
    name: Generate registry JSON
    runs-on: ubuntu-22.04
    needs: build
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install terminal-info CLI
        run: cargo install terminal-info --locked

      - name: Download artifacts
        uses: actions/download-artifact@v4
        with:
          path: dist
          pattern: plugin-*
          merge-multiple: true

      - name: Generate registry JSON
        run: tinfo plugin pack --from-dist

      - name: Upload registry JSON artifact
        uses: actions/upload-artifact@v4
        with:
          name: registry-json
          path: dist/registry/*.json

  release:
    name: Publish release
    runs-on: ubuntu-22.04
    needs:
      - build
      - registry
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4
        with:
          path: dist

      - name: Publish GitHub release
        uses: softprops/action-gh-release@v2
        with:
          files: dist/**/*
          allowUpdates: true
          generate_release_notes: true
"#
    )
}

fn trusted_plugins_path() -> Result<PathBuf, String> {
    Ok(home_dir_path()
        .join(".terminal-info")
        .join("trusted_plugins.json"))
}

fn load_trusted_plugins() -> Result<TrustedPlugins, String> {
    let path = trusted_plugins_path()?;
    if !path.exists() {
        return Ok(TrustedPlugins::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|err| format!("Failed to read trusted plugins: {err}"))?;
    serde_json::from_str(&contents).map_err(|err| format!("Failed to parse trusted plugins: {err}"))
}

fn save_trusted_plugins(value: &TrustedPlugins) -> Result<(), String> {
    let path = trusted_plugins_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create trusted plugin directory: {err}"))?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize trusted plugins: {err}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|err| format!("Failed to write trusted plugins: {err}"))
}

fn is_plugin_trusted(name: &str) -> Result<bool, String> {
    let allowlist = load_trusted_plugins()?;
    Ok(allowlist.trusted.iter().any(|entry| entry == name))
}

fn read_installed_manifest(name: &str) -> Result<toml::Value, String> {
    let path = plugin_manifest_path(name)?;
    let contents =
        fs::read_to_string(path).map_err(|err| format!("Failed to read plugin manifest: {err}"))?;
    toml::from_str(&contents).map_err(|err| format!("Failed to parse plugin manifest: {err}"))
}

fn checksum_status(
    name: &str,
    registry: Option<&PluginMetadata>,
    manifest: Option<&toml::Value>,
) -> String {
    let Some(registry) = registry else {
        return "unknown".to_string();
    };
    let Some(manifest) = manifest else {
        return "missing".to_string();
    };
    let expected = registry.checksums.get(target_triple());
    let actual = manifest
        .get("install")
        .and_then(|section| section.get("asset_checksum"))
        .and_then(|value| value.as_str());
    if expected.map(|value| value.as_str()) == actual {
        "verified".to_string()
    } else {
        let _ = name;
        "mismatch".to_string()
    }
}
