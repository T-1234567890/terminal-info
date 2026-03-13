use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, Cursor, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dialoguer::{Input, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use minisign_verify::{PublicKey, Signature};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive;
use zip::ZipArchive;

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
const CACHE_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub repo: String,
    #[serde(default)]
    pub binary: String,
    pub version: String,
    pub checksums: std::collections::BTreeMap<String, String>,
    pub pubkey: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubContentItem {
    name: String,
    download_url: Option<String>,
    #[serde(rename = "type")]
    item_type: String,
}

#[derive(Serialize, Deserialize)]
struct PluginCache {
    fetched_at: u64,
    plugins: Vec<PluginMetadata>,
}

#[derive(Serialize, Deserialize)]
struct PluginManifest {
    plugin: PluginSection,
    command: CommandSection,
    compatibility: CompatibilitySection,
    #[serde(skip_serializing_if = "Option::is_none")]
    install: Option<InstallSection>,
}

#[derive(Serialize, Deserialize)]
struct PluginSection {
    name: String,
    version: String,
    description: String,
}

#[derive(Serialize, Deserialize)]
struct CommandSection {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct CompatibilitySection {
    terminal_info: String,
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
    pub content: String,
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

    let status = Command::new(&binary_path)
        .args(args)
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
            .map(|plugin| plugin.repo.clone())
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

pub fn dashboard_widgets() -> Vec<PluginWidget> {
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
        let output = match Command::new(binary).arg("--widget").output() {
            Ok(output) if output.status.success() => output,
            _ => continue,
        };
        let text = String::from_utf8_lossy(&output.stdout);
        if let Ok(widget) = serde_json::from_str::<PluginWidget>(&text) {
            widgets.push(widget);
        }
    }
    widgets
}

pub fn search_plugins() -> Result<(), String> {
    let plugins = load_plugin_index()?;
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&plugins).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }
    if plugins.is_empty() {
        println!("No plugins available.");
        return Ok(());
    }

    for plugin in plugins {
        println!("{} - {}", plugin.name, plugin.description);
    }

    Ok(())
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

    fs::write(
        directory.join("plugin.toml"),
        plugin_manifest_template(&plugin_name, &description),
    )
    .map_err(|err| format!("Failed to write plugin.toml: {err}"))?;
    fs::write(directory.join("Cargo.toml"), cargo_template(&plugin_name))
        .map_err(|err| format!("Failed to write Cargo.toml: {err}"))?;
    fs::write(directory.join("src").join("main.rs"), main_template())
        .map_err(|err| format!("Failed to write src/main.rs: {err}"))?;
    fs::write(
        directory.join("README.md"),
        readme_template(&plugin_name, &description),
    )
    .map_err(|err| format!("Failed to write README.md: {err}"))?;
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

    let index = load_plugin_index().unwrap_or_default();
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

        match index.iter().find(|plugin| plugin.name == name) {
            Some(plugin) if plugin_binary_name(plugin) == format!("tinfo-{name}") => {
                println!("{} Plugin \"{name}\" metadata OK", success_prefix());
            }
            _ => println!("{} Plugin \"{name}\" version mismatch", error_prefix()),
        }
    }

    Ok(())
}

fn install_or_update_plugin(plugin: &PluginMetadata, action: &str) -> Result<(), String> {
    let plugin_home = plugin_home_path(&plugin.name)?;
    fs::create_dir_all(&plugin_home)
        .map_err(|err| format!("Failed to create plugin directory: {err}"))?;

    let (owner, repo) = parse_github_repo(&plugin.repo)?;
    let release = fetch_release(&owner, &repo, &plugin.version)?;
    if release.tag_name != plugin.version {
        return Err(format!(
            "Registry version mismatch for plugin '{}': expected {}, got {}.",
            plugin.name, plugin.version, release.tag_name
        ));
    }

    let binary = plugin_binary_name(plugin);
    let asset = select_asset(&release.assets, &binary).ok_or_else(|| {
        format!(
            "No compatible release asset found for plugin '{}'.",
            plugin.name
        )
    })?;
    let signature_asset = select_signature_asset(&release.assets, &asset.name)
        .ok_or_else(|| format!("No minisign signature found for plugin '{}'.", plugin.name))?;

    let bytes = github_client()?
        .get(&asset.browser_download_url)
        .send()
        .map_err(|err| format!("Failed to download plugin asset: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download plugin asset: {err}"))?
        .bytes()
        .map_err(|err| format!("Failed to read plugin asset: {err}"))?;
    let signature = github_client()?
        .get(&signature_asset.browser_download_url)
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
    extract_asset(&asset.name, &binary, bytes.as_ref(), &destination)?;
    set_executable(&destination)?;
    write_plugin_manifest(plugin, &release.tag_name, &sha256_hex(bytes.as_ref()))?;

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

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".terminal-info").join("plugins"))
}

fn plugin_cache_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("TINFO_PLUGIN_CACHE_PATH") {
        return Ok(PathBuf::from(path));
    }

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home)
        .join(".terminal-info")
        .join("cache")
        .join("plugins.json"))
}

fn plugin_index_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_INDEX_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Err("No local plugin index override configured. Falling back to GitHub registry.".to_string())
}

fn load_plugin_index() -> Result<Vec<PluginMetadata>, String> {
    if let Ok(dir) = plugin_index_dir() {
        return load_plugin_index_from_local_dir(&dir);
    }

    load_plugin_index_cached()
}

fn load_plugin_index_from_local_dir(dir: &Path) -> Result<Vec<PluginMetadata>, String> {
    let mut plugins = Vec::new();
    let mut seen = HashSet::new();

    for entry in fs::read_dir(dir).map_err(|err| format!("Failed to read plugin index: {err}"))? {
        let entry = entry.map_err(|err| format!("Failed to read plugin index: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
        let plugin: PluginMetadata = serde_json::from_str(&contents)
            .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?;
        validate_plugin_metadata(&plugin)?;

        if !seen.insert(plugin.name.clone()) {
            return Err(format!(
                "Duplicate plugin name '{}' in plugin index.",
                plugin.name
            ));
        }

        plugins.push(plugin);
    }

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

fn load_plugin_index_cached() -> Result<Vec<PluginMetadata>, String> {
    let cache_path = plugin_cache_path()?;
    let cache = read_plugin_cache(&cache_path).ok();

    if let Some(cache) = cache.as_ref() {
        if !cache_is_expired(cache.fetched_at) {
            return Ok(cache.plugins.clone());
        }
    }

    match fetch_plugin_index_from_registry() {
        Ok(plugins) => {
            write_plugin_cache(&cache_path, &plugins)?;
            Ok(plugins)
        }
        Err(err) => {
            if let Some(cache) = cache {
                Ok(cache.plugins)
            } else {
                Err(err)
            }
        }
    }
}

fn fetch_plugin_index_from_registry() -> Result<Vec<PluginMetadata>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/plugins",
        REGISTRY_OWNER, REGISTRY_REPO
    );
    let items: Vec<GithubContentItem> = github_client()?
        .get(url)
        .send()
        .map_err(|err| format!("Failed to fetch plugin registry: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to fetch plugin registry: {err}"))?
        .json()
        .map_err(|err| format!("Failed to parse plugin registry: {err}"))?;

    let mut plugins = Vec::new();
    let mut seen = HashSet::new();

    for item in items {
        if item.item_type != "file" || !item.name.ends_with(".json") {
            continue;
        }

        let download_url = item
            .download_url
            .ok_or_else(|| format!("Registry entry '{}' has no download URL.", item.name))?;
        let contents = github_client()?
            .get(download_url)
            .send()
            .map_err(|err| format!("Failed to download plugin metadata: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Failed to download plugin metadata: {err}"))?
            .text()
            .map_err(|err| format!("Failed to read plugin metadata: {err}"))?;
        let plugin: PluginMetadata = serde_json::from_str(&contents)
            .map_err(|err| format!("Failed to parse plugin metadata '{}': {err}", item.name))?;
        validate_plugin_metadata(&plugin)?;

        if !seen.insert(plugin.name.clone()) {
            return Err(format!(
                "Duplicate plugin name '{}' in plugin index.",
                plugin.name
            ));
        }

        plugins.push(plugin);
    }

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

fn load_plugin_by_name(name: &str) -> Result<PluginMetadata, String> {
    load_plugin_index()?
        .into_iter()
        .find(|plugin| plugin.name == name)
        .ok_or_else(|| format!("Plugin '{}' not found in plugin index.", name))
}

fn read_plugin_cache(path: &Path) -> Result<PluginCache, String> {
    let contents =
        fs::read_to_string(path).map_err(|err| format!("Failed to read plugin cache: {err}"))?;
    serde_json::from_str(&contents).map_err(|err| format!("Failed to parse plugin cache: {err}"))
}

fn write_plugin_cache(path: &Path, plugins: &[PluginMetadata]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create plugin cache directory: {err}"))?;
    }

    let payload = PluginCache {
        fetched_at: now_unix(),
        plugins: plugins.to_vec(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Failed to serialize plugin cache: {err}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|err| format!("Failed to write plugin cache: {err}"))
}

fn cache_is_expired(fetched_at: u64) -> bool {
    now_unix().saturating_sub(fetched_at) > CACHE_TTL_SECS
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
        || plugin.description.trim().is_empty()
        || plugin.repo.trim().is_empty()
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

    if !plugin.repo.starts_with("https://github.com/") {
        return Err(format!(
            "Plugin '{}' must use a GitHub repository URL.",
            plugin.name
        ));
    }

    if plugin.version == "latest" {
        return Err(format!(
            "Plugin '{}' must pin an exact reviewed version, not 'latest'.",
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
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(format!(
            "Plugin '{}' must use lowercase ASCII letters, digits, or '-'.",
            name
        ));
    }

    Ok(())
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

fn fetch_release(owner: &str, repo: &str, version: &str) -> Result<GithubRelease, String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{version}");

    github_client()?
        .get(url)
        .send()
        .map_err(|err| format!("Failed to fetch GitHub release metadata: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to fetch GitHub release metadata: {err}"))?
        .json()
        .map_err(|err| format!("Failed to parse GitHub release metadata: {err}"))
}

fn github_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))
}

fn select_asset<'a>(assets: &'a [GithubAsset], binary: &str) -> Option<&'a GithubAsset> {
    let raw = binary_filename(binary);
    let target = target_triple();

    assets
        .iter()
        .find(|asset| asset.name == raw)
        .or_else(|| {
            assets.iter().find(|asset| {
                asset.name.contains(binary)
                    && asset.name.contains(target)
                    && (asset.name.ends_with(".tar.gz") || asset.name.ends_with(".zip"))
            })
        })
        .or_else(|| {
            assets.iter().find(|asset| {
                asset.name.contains(binary)
                    && (asset.name.ends_with(".tar.gz") || asset.name.ends_with(".zip"))
            })
        })
}

fn select_signature_asset<'a>(
    assets: &'a [GithubAsset],
    asset_name: &str,
) -> Option<&'a GithubAsset> {
    let signature_name = format!("{asset_name}.minisig");
    assets.iter().find(|asset| asset.name == signature_name)
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
        },
        command: CommandSection {
            name: plugin.name.clone(),
        },
        compatibility: CompatibilitySection {
            terminal_info: format!(">={}", env!("CARGO_PKG_VERSION")),
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

[command]
name = "{name}"

[compatibility]
terminal_info = ">={version}"
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
"#
    )
}

fn main_template() -> &'static str {
    r#"fn main() {
    println!("Hello from Terminal Info plugin!");
}
"#
}

fn readme_template(name: &str, description: &str) -> String {
    format!(
        r#"# tinfo-{name}

{description}

## Build

```bash
cargo build --release
```

## Run With Terminal Info

```bash
tinfo {name}
```

Terminal Info will route `tinfo {name}` to the `tinfo-{name}` executable.

## Submit To The Plugin Registry

1. Publish a GitHub release for this plugin
2. Add or update `plugins/{name}.json` in the Terminal Info repository
3. Open a pull request for registry review
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
          choco install minisign -y

      - name: Package asset (Unix)
        if: runner.os != 'Windows'
        run: |
          mkdir -p dist
          cp target/${{{{ matrix.target }}}}/release/${{{{ matrix.binary_name }}}} dist/${{{{ matrix.binary_name }}}}
          mv dist/${{{{ matrix.binary_name }}}} dist/tinfo-{name}-${{{{ matrix.target }}}}
          shasum -a 256 dist/tinfo-{name}-${{{{ matrix.target }}}} > dist/tinfo-{name}-${{{{ matrix.target }}}}.sha256
          printf '%s' "$MINISIGN_SECRET_KEY" > minisign.key
          chmod 600 minisign.key
          minisign -S -s minisign.key -m dist/tinfo-{name}-${{{{ matrix.target }}}} -x dist/tinfo-{name}-${{{{ matrix.target }}}}.minisig -t "tinfo-{name}-${{{{ matrix.target }}}}"

      - name: Package asset (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          New-Item -ItemType Directory -Force -Path dist | Out-Null
          Copy-Item "target/${{{{ matrix.target }}}}/release/${{{{ matrix.binary_name }}}}" "dist/tinfo-{name}-${{{{ matrix.target }}}}.exe"
          $hash = (Get-FileHash "dist/tinfo-{name}-${{{{ matrix.target }}}}.exe" -Algorithm SHA256).Hash.ToLower()
          Set-Content -Path "dist/tinfo-{name}-${{{{ matrix.target }}}}.exe.sha256" -Value "$hash  tinfo-{name}-${{{{ matrix.target }}}}.exe"
          if ($env:MINISIGN_SECRET_KEY) {{
            [System.IO.File]::WriteAllText("minisign.key", $env:MINISIGN_SECRET_KEY)
            minisign -S -s minisign.key -m "dist/tinfo-{name}-${{{{ matrix.target }}}}.exe" -x "dist/tinfo-{name}-${{{{ matrix.target }}}}.exe.minisig" -t "tinfo-{name}-${{{{ matrix.target }}}}.exe"
          }}

      - name: Upload release asset (Unix)
        if: runner.os != 'Windows'
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/tinfo-{name}-${{{{ matrix.target }}}}
            dist/tinfo-{name}-${{{{ matrix.target }}}}.sha256
            dist/tinfo-{name}-${{{{ matrix.target }}}}.minisig
          generate_release_notes: true

      - name: Upload release asset (signed Windows)
        if: runner.os == 'Windows'
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/tinfo-{name}-${{{{ matrix.target }}}}.exe
            dist/tinfo-{name}-${{{{ matrix.target }}}}.exe.sha256
            dist/tinfo-{name}-${{{{ matrix.target }}}}.exe.minisig
          generate_release_notes: true
"#
    )
}

fn trusted_plugins_path() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home)
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
