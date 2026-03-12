use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
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
    pub binary: String,
    pub version: String,
}

#[derive(Deserialize)]
struct GithubRelease {
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

pub fn run_plugin(command: &str, args: &[String]) -> Result<(), String> {
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

pub fn search_plugins() -> Result<(), String> {
    let plugins = load_plugin_index()?;
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
    let plugin = load_plugin_by_name(name)?;
    let path = plugin_dir_path()?.join(binary_filename(&plugin.binary));
    if !path.exists() {
        return Err(format!("Plugin '{}' is not installed.", name));
    }

    fs::remove_file(&path).map_err(|err| format!("Failed to remove plugin '{}': {err}", name))?;
    println!("Removed plugin '{}'.", name);
    Ok(())
}

pub fn list_plugins() -> Result<(), String> {
    let dir = plugin_dir_path()?;
    if !dir.exists() {
        println!("No plugins installed.");
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("Failed to read plugins: {err}"))? {
        let entry = entry.map_err(|err| format!("Failed to read plugins: {err}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("tinfo-") {
            entries.push(name);
        }
    }

    if entries.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    entries.sort();
    for entry in entries {
        println!("{entry}");
    }

    Ok(())
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
        let binary_path = dir.join(binary_filename(&format!("tinfo-{name}")));
        if !binary_path.exists() {
            println!("{} Plugin \"{name}\" missing binary", error_prefix());
            continue;
        }

        match index.iter().find(|plugin| plugin.name == name) {
            Some(plugin) if plugin.binary == format!("tinfo-{name}") => {
                if plugin.version == "latest" {
                    println!("{} Plugin \"{name}\" metadata OK", success_prefix());
                } else {
                    println!("{} Plugin \"{name}\" version mismatch", error_prefix());
                }
            }
            _ => println!("{} Plugin \"{name}\" version mismatch", error_prefix()),
        }
    }

    Ok(())
}

fn install_or_update_plugin(plugin: &PluginMetadata, action: &str) -> Result<(), String> {
    let plugin_dir = plugin_dir_path()?;
    fs::create_dir_all(&plugin_dir)
        .map_err(|err| format!("Failed to create plugin directory: {err}"))?;

    let (owner, repo) = parse_github_repo(&plugin.repo)?;
    let release = fetch_release(&owner, &repo, &plugin.version)?;
    let asset = select_asset(&release.assets, &plugin.binary).ok_or_else(|| {
        format!(
            "No compatible release asset found for plugin '{}'.",
            plugin.name
        )
    })?;

    let bytes = github_client()?
        .get(&asset.browser_download_url)
        .send()
        .map_err(|err| format!("Failed to download plugin asset: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download plugin asset: {err}"))?
        .bytes()
        .map_err(|err| format!("Failed to read plugin asset: {err}"))?;

    let destination = plugin_dir.join(binary_filename(&plugin.binary));
    extract_asset(&asset.name, &plugin.binary, bytes.as_ref(), &destination)?;
    set_executable(&destination)?;

    println!(
        "{action} plugin '{}' at {}.",
        plugin.name,
        destination.display()
    );
    Ok(())
}

fn resolve_plugin_binary(binary_name: &str) -> Option<PathBuf> {
    find_in_plugin_dir(binary_name).or_else(|| find_in_path(binary_name))
}

fn find_in_plugin_dir(binary_name: &str) -> Option<PathBuf> {
    let dir = plugin_dir_path().ok()?;
    let candidate = dir.join(binary_name);
    if is_executable_file(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;

    for dir in env::split_paths(&paths) {
        let candidate = dir.join(binary_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn plugin_dir_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_DIR") {
        return Ok(PathBuf::from(dir));
    }

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".tinfo").join("plugins"))
}

fn plugin_cache_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("TINFO_PLUGIN_CACHE_PATH") {
        return Ok(PathBuf::from(path));
    }

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home)
        .join(".tinfo")
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
    if plugin.name.trim().is_empty()
        || plugin.description.trim().is_empty()
        || plugin.repo.trim().is_empty()
        || plugin.binary.trim().is_empty()
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

    let expected_binary = format!("tinfo-{}", plugin.name);
    if plugin.binary != expected_binary {
        return Err(format!(
            "Plugin '{}' must use binary name '{}'.",
            plugin.name, expected_binary
        ));
    }

    if !plugin.repo.starts_with("https://github.com/") {
        return Err(format!(
            "Plugin '{}' must use a GitHub repository URL.",
            plugin.name
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
    let url = if version == "latest" {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{version}")
    };

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
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(stripped) = name.strip_prefix("tinfo-") {
            let stripped = stripped.strip_suffix(".exe").unwrap_or(stripped);
            names.push(stripped.to_string());
        }
    }

    names.sort();
    names.dedup();
    Ok(names)
}
