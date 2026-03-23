use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sysinfo::Disks;

#[derive(Clone, Debug, Serialize)]
struct FilesystemUsage {
    filesystem: String,
    filesystem_type: String,
    mount: String,
    used_percent: f64,
    used_bytes: u64,
    free_bytes: u64,
    available_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct LargestEntry {
    path: String,
    size_bytes: u64,
    kind: String,
}

#[derive(Clone, Debug, Serialize)]
struct StorageSuggestion {
    summary: String,
    path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct StorageAnalyzeView {
    root: String,
    filesystems: Vec<FilesystemUsage>,
    top_directories: Vec<LargestEntry>,
    largest_files: Vec<LargestEntry>,
    suggestions: Vec<StorageSuggestion>,
}

pub fn show_storage_overview() -> Result<(), String> {
    let filesystems = filesystem_usage();
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&filesystems).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }
    if filesystems.is_empty() {
        println!("No filesystems detected.");
        return Ok(());
    }
    for fs in filesystems {
        println!(
            "Filesystem: {}",
            if fs.filesystem.is_empty() {
                fs.mount.clone()
            } else {
                fs.filesystem.clone()
            }
        );
        println!("Type: {}", fs.filesystem_type);
        println!("Mount: {}", fs.mount);
        println!(
            "Used: {:.0}% ({} / {})",
            fs.used_percent,
            format_bytes(fs.used_bytes),
            format_bytes(fs.total_bytes)
        );
        println!("Free: {}", format_bytes(fs.free_bytes));
        println!();
    }
    Ok(())
}

pub fn show_storage_usage() -> Result<(), String> {
    show_storage_overview()
}

pub fn show_storage_largest() -> Result<(), String> {
    let root = default_analysis_root()?;
    let entries = largest_entries(&root, 8);
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    println!("Root: {}", root.display());
    println!("Top entries:");
    for entry in entries {
        println!("{} {:<4} {}", pad_size(entry.size_bytes), entry.kind, entry.path);
    }
    Ok(())
}

pub fn show_storage_analyze() -> Result<(), String> {
    let root = default_analysis_root()?;
    let filesystems = filesystem_usage();
    let top_directories = largest_entries(&root, 8);
    let largest_files = largest_files(&root, 8, 5);
    let suggestions = storage_suggestions(&root, &top_directories, &largest_files);
    let view = StorageAnalyzeView {
        root: root.display().to_string(),
        filesystems,
        top_directories,
        largest_files,
        suggestions,
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("Analysis root: {}", view.root);
    println!();
    for fs in &view.filesystems {
        println!(
            "Filesystem: {}",
            if fs.filesystem.is_empty() {
                fs.mount.clone()
            } else {
                fs.filesystem.clone()
            }
        );
        println!("Type: {}", fs.filesystem_type);
        println!("Mount: {}", fs.mount);
        println!(
            "Used: {:.0}% ({} / {})",
            fs.used_percent,
            format_bytes(fs.used_bytes),
            format_bytes(fs.total_bytes)
        );
        println!(
            "Free: {} (available {})",
            format_bytes(fs.free_bytes),
            format_bytes(fs.available_bytes)
        );
        println!();
    }
    println!("Top storage consumers:");
    for entry in &view.top_directories {
        println!("{} {}", pad_size(entry.size_bytes), entry.path);
    }
    println!();
    println!("Largest files:");
    for entry in &view.largest_files {
        println!("{} {}", pad_size(entry.size_bytes), entry.path);
    }
    println!();
    println!("Suggestions:");
    if view.suggestions.is_empty() {
        println!("- no obvious cleanup targets detected");
    } else {
        for suggestion in &view.suggestions {
            if let Some(path) = &suggestion.path {
                println!("- {} ({})", suggestion.summary, path);
            } else {
                println!("- {}", suggestion.summary);
            }
        }
    }
    Ok(())
}

pub fn show_storage_optimize() -> Result<(), String> {
    let root = default_analysis_root()?;
    let entries = largest_entries(&root, 8);
    let files = largest_files(&root, 8, 5);
    let suggestions = storage_suggestions(&root, &entries, &files);
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&suggestions).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    println!("Suggestions:");
    if suggestions.is_empty() {
        println!("- no obvious cleanup targets detected");
        return Ok(());
    }

    for suggestion in suggestions {
        if let Some(path) = suggestion.path {
            println!("- {} ({})", suggestion.summary, path);
        } else {
            println!("- {}", suggestion.summary);
        }
    }
    Ok(())
}

fn filesystem_usage() -> Vec<FilesystemUsage> {
    let disks = Disks::new_with_refreshed_list();
    let mut filesystems = disks
        .list()
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let used_percent = if total > 0 {
                used as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            FilesystemUsage {
                filesystem: disk.name().to_string_lossy().to_string(),
                filesystem_type: disk.file_system().to_string_lossy().to_string(),
                mount: disk.mount_point().display().to_string(),
                used_percent,
                used_bytes: used,
                free_bytes: available,
                available_bytes: available,
                total_bytes: total,
            }
        })
        .collect::<Vec<_>>();
    filesystems.sort_by(|a, b| a.mount.cmp(&b.mount));
    dedupe_filesystems(filesystems)
}

fn default_analysis_root() -> Result<PathBuf, String> {
    if let Ok(home) = env::var("HOME") {
        return Ok(PathBuf::from(home));
    }
    env::current_dir().map_err(|err| format!("Failed to determine analysis root: {err}"))
}

fn largest_entries(root: &Path, limit: usize) -> Vec<LargestEntry> {
    let mut entries = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|iter| iter.flatten())
        .map(|entry| {
            let path = entry.path();
            LargestEntry {
                path: path.display().to_string(),
                size_bytes: path_size(&path, 6),
                kind: "dir".to_string(),
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    entries.truncate(limit);
    entries
}

fn largest_files(root: &Path, limit: usize, depth: usize) -> Vec<LargestEntry> {
    let mut files = Vec::new();
    collect_files(root, depth, &mut files);
    files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    files.truncate(limit);
    files
}

fn collect_files(root: &Path, depth: usize, files: &mut Vec<LargestEntry>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            files.push(LargestEntry {
                path: path.display().to_string(),
                size_bytes: metadata.len(),
                kind: "file".to_string(),
            });
        } else if metadata.is_dir() {
            collect_files(&path, depth.saturating_sub(1), files);
        }
    }
}

fn dedupe_filesystems(filesystems: Vec<FilesystemUsage>) -> Vec<FilesystemUsage> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for fs in filesystems {
        if should_skip_mount(&fs.mount) {
            continue;
        }
        let key = format!("{}:{}:{}", fs.filesystem, fs.total_bytes, fs.filesystem_type);
        if seen.insert(key) {
            deduped.push(fs);
        }
    }
    deduped
}

fn should_skip_mount(mount: &str) -> bool {
    if cfg!(target_os = "macos") {
        return mount.starts_with("/System/Volumes/")
            || mount.starts_with("/private/var/vm")
            || mount.starts_with("/private/var/run");
    }
    false
}

fn path_size(path: &Path, depth: usize) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() {
        return 0;
    }
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() || depth == 0 {
        return 0;
    }
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|iter| iter.flatten())
        .map(|entry| path_size(&entry.path(), depth.saturating_sub(1)))
        .sum()
}

fn storage_suggestions(
    root: &Path,
    top_entries: &[LargestEntry],
    largest_files: &[LargestEntry],
) -> Vec<StorageSuggestion> {
    let mut suggestions = Vec::new();

    for entry in top_entries {
        let path = entry.path.to_ascii_lowercase();
        if path.contains("deriveddata") && entry.size_bytes > gib(2) {
            suggestions.push(StorageSuggestion {
                summary: "Large Xcode DerivedData folder detected".to_string(),
                path: Some(entry.path.clone()),
            });
        }
        if path.ends_with("/target") && entry.size_bytes > gib(1) {
            suggestions.push(StorageSuggestion {
                summary: "Large Rust build artifacts detected".to_string(),
                path: Some(entry.path.clone()),
            });
        }
        if path.ends_with("/node_modules") && entry.size_bytes > gib(1) {
            suggestions.push(StorageSuggestion {
                summary: "Large node_modules directory detected".to_string(),
                path: Some(entry.path.clone()),
            });
        }
        if path.contains("/library/caches") || path.ends_with("/.cache") {
            if entry.size_bytes > gib(1) {
                suggestions.push(StorageSuggestion {
                    summary: "Large cache directory detected".to_string(),
                    path: Some(entry.path.clone()),
                });
            }
        }
        if path.contains("/log") && entry.size_bytes > mib(256) {
            suggestions.push(StorageSuggestion {
                summary: "Large log directory detected".to_string(),
                path: Some(entry.path.clone()),
            });
        }
    }

    for entry in largest_files {
        let path = entry.path.to_ascii_lowercase();
        if path.contains("/tmp") || path.contains("/temp") {
            if entry.size_bytes > mib(512) {
                suggestions.push(StorageSuggestion {
                    summary: "Large temporary file detected".to_string(),
                    path: Some(entry.path.clone()),
                });
            }
        }
        if path.ends_with(".log") && entry.size_bytes > mib(128) {
            suggestions.push(StorageSuggestion {
                summary: "Large log file detected".to_string(),
                path: Some(entry.path.clone()),
            });
        }
    }

    let common = [
        (
            root.join("Library").join("Developer").join("Xcode").join("DerivedData"),
            "Large Xcode DerivedData folder detected",
            gib(1),
        ),
        (root.join("Library").join("Caches"), "Large cache directory detected", gib(1)),
        (root.join(".cache"), "Large cache directory detected", gib(1)),
        (root.join("target"), "Large Rust build artifacts detected", gib(1)),
        (root.join("node_modules"), "Large node_modules directory detected", gib(1)),
        (root.join(".npm"), "npm cache is using significant storage", mib(512)),
    ];
    for (path, summary, threshold) in common {
        if path.exists() {
            let size = path_size(&path, 6);
            if size > threshold {
                suggestions.push(StorageSuggestion {
                    summary: summary.to_string(),
                    path: Some(path.display().to_string()),
                });
            }
        }
    }

    suggestions.sort_by(|a, b| a.summary.cmp(&b.summary).then(a.path.cmp(&b.path)));
    suggestions.dedup_by(|a, b| a.summary == b.summary && a.path == b.path);
    suggestions
}

fn pad_size(bytes: u64) -> String {
    format!("{:>8}", format_bytes(bytes))
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn gib(value: u64) -> u64 {
    value * 1024 * 1024 * 1024
}

fn mib(value: u64) -> u64 {
    value * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_suggestions_detect_cache_paths() {
        let entries = vec![LargestEntry {
            path: "/Users/test/Library/Caches".to_string(),
            size_bytes: gib(2),
            kind: "dir".to_string(),
        }];
        let suggestions = storage_suggestions(Path::new("/Users/test"), &entries, &[]);
        assert!(suggestions.iter().any(|item| item.summary.contains("cache")));
    }

    #[test]
    fn storage_suggestions_detect_large_temp_files() {
        let directories = Vec::new();
        let files = vec![LargestEntry {
            path: "/Users/test/tmp/archive.tmp".to_string(),
            size_bytes: mib(700),
            kind: "file".to_string(),
        }];
        let suggestions = storage_suggestions(Path::new("/Users/test"), &directories, &files);
        assert!(suggestions.iter().any(|item| item.summary.contains("temporary")));
    }
}
