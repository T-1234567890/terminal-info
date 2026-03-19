use std::collections::BTreeMap;

use serde::Serialize;

use crate::plugin::{PluginSearchEntry, installed_plugin_search_entries, registry_plugin_search_entries};

#[derive(Clone)]
struct BuiltinEntry {
    command: &'static str,
    description: &'static str,
    category: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchResultKind {
    Builtin,
    Plugin,
}

#[derive(Clone, Serialize)]
pub struct SearchResult {
    pub kind: SearchResultKind,
    pub command: String,
    pub title: String,
    pub description: String,
    pub source: String,
    pub category: String,
    pub installed: bool,
    pub trusted: bool,
    pub score: i32,
}

#[derive(Serialize)]
struct SearchOutput {
    query: String,
    results: Vec<SearchResult>,
}

const BUILTIN_COMMANDS: &[BuiltinEntry] = &[
    BuiltinEntry { command: "weather", description: "Weather tools and location-aware forecasts", category: "weather" },
    BuiltinEntry { command: "weather now", description: "Show current weather for the configured location or a city", category: "weather" },
    BuiltinEntry { command: "weather forecast", description: "Show a short forecast", category: "weather" },
    BuiltinEntry { command: "weather hourly", description: "Show hourly weather", category: "weather" },
    BuiltinEntry { command: "weather alerts", description: "Show active weather alerts", category: "weather" },
    BuiltinEntry { command: "weather location", description: "Show or set the default weather location", category: "weather" },
    BuiltinEntry { command: "ping", description: "Test network latency to a host", category: "network" },
    BuiltinEntry { command: "latency", description: "Run the latency probes used by ping", category: "network" },
    BuiltinEntry { command: "network", description: "Show local network information", category: "network" },
    BuiltinEntry { command: "network speed", description: "Measure network download speed", category: "network" },
    BuiltinEntry { command: "system", description: "Show system information", category: "system" },
    BuiltinEntry { command: "system hardware", description: "Show detailed hardware inventory", category: "system" },
    BuiltinEntry { command: "disk", description: "Inspect disk health and reliability", category: "storage" },
    BuiltinEntry { command: "storage", description: "Analyze filesystem usage and cleanup opportunities", category: "storage" },
    BuiltinEntry { command: "ps", description: "Inspect running processes", category: "process" },
    BuiltinEntry { command: "top", description: "Alias for ps sorted process inspection", category: "process" },
    BuiltinEntry { command: "time", description: "Show local or global times", category: "time" },
    BuiltinEntry { command: "diagnostic", description: "Run system, network, and plugin diagnostics", category: "diagnostic" },
    BuiltinEntry { command: "config", description: "Manage configuration and first-run setup", category: "config" },
    BuiltinEntry { command: "profile", description: "Manage configuration profiles", category: "config" },
    BuiltinEntry { command: "completion", description: "Generate or install shell completions", category: "shell" },
    BuiltinEntry { command: "dashboard", description: "Inspect or reset dashboard settings", category: "dashboard" },
    BuiltinEntry { command: "plugin", description: "Manage plugins and plugin development workflows", category: "plugin" },
    BuiltinEntry { command: "search", description: "Search built-ins and plugins", category: "search" },
    BuiltinEntry { command: "update", description: "Install the latest released version of tinfo", category: "maintenance" },
    BuiltinEntry { command: "self-repair", description: "Repair the current installation", category: "maintenance" },
    BuiltinEntry { command: "reinstall", description: "Reinstall the latest release", category: "maintenance" },
    BuiltinEntry { command: "uninstall", description: "Remove the tinfo binary and optionally local data", category: "maintenance" },
];

pub fn run_search(query_parts: &[String]) -> Result<(), String> {
    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        return Err("Search query cannot be empty.".to_string());
    }

    let results = collect_results(&query)?;
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&SearchOutput { query, results })
                .unwrap_or_else(|_| "{\"query\":\"\",\"results\":[]}".to_string())
        );
        return Ok(());
    }

    print_terminal_results(&query, &results);
    Ok(())
}

fn collect_results(query: &str) -> Result<Vec<SearchResult>, String> {
    let mut results = builtin_results(query);
    results.extend(plugin_results(query)?);
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.kind_rank().cmp(&b.kind_rank()))
            .then_with(|| a.command.cmp(&b.command))
    });
    results.truncate(12);
    Ok(results)
}

fn builtin_results(query: &str) -> Vec<SearchResult> {
    BUILTIN_COMMANDS
        .iter()
        .filter_map(|entry| {
            let score = score_match(query, entry.command, entry.description);
            (score > 0).then(|| SearchResult {
                kind: SearchResultKind::Builtin,
                command: entry.command.to_string(),
                title: entry.command.to_string(),
                description: entry.description.to_string(),
                source: "built-in".to_string(),
                category: entry.category.to_string(),
                installed: true,
                trusted: true,
                score: score + 8,
            })
        })
        .collect()
}

fn plugin_results(query: &str) -> Result<Vec<SearchResult>, String> {
    let mut merged: BTreeMap<String, SearchResult> = BTreeMap::new();

    for plugin in registry_plugin_search_entries()? {
        let score = score_match(query, &plugin.name, &plugin.description);
        if score <= 0 {
            continue;
        }
        merged.insert(
            plugin.name.clone(),
            SearchResult {
                kind: SearchResultKind::Plugin,
                command: plugin.name.clone(),
                title: plugin.name.clone(),
                description: plugin.description.clone(),
                source: "registry".to_string(),
                category: "plugin".to_string(),
                installed: false,
                trusted: false,
                score,
            },
        );
    }

    for plugin in installed_plugin_search_entries()? {
        let score = score_match(query, &plugin.name, &plugin.description);
        if score <= 0 {
            continue;
        }
        merge_plugin_result(&mut merged, plugin, score);
    }

    Ok(merged.into_values().collect())
}

fn merge_plugin_result(
    merged: &mut BTreeMap<String, SearchResult>,
    plugin: PluginSearchEntry,
    score: i32,
) {
    let mut combined_score = score + 18;
    if plugin.trusted {
        combined_score += 4;
    }

    if let Some(existing) = merged.get_mut(&plugin.name) {
        existing.installed = true;
        existing.trusted = plugin.trusted;
        existing.source = "installed, registry".to_string();
        existing.score = existing.score.max(combined_score);
        if !plugin.description.trim().is_empty() {
            existing.description = plugin.description;
        }
        return;
    }

    merged.insert(
        plugin.name.clone(),
        SearchResult {
            kind: SearchResultKind::Plugin,
            command: plugin.name.clone(),
            title: plugin.name,
            description: plugin.description,
            source: "installed".to_string(),
            category: "plugin".to_string(),
            installed: true,
            trusted: plugin.trusted,
            score: combined_score,
        },
    );
}

fn score_match(query: &str, primary: &str, description: &str) -> i32 {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return 0;
    }

    let primary = primary.to_ascii_lowercase();
    let description = description.to_ascii_lowercase();
    let tokens = query.split_whitespace().collect::<Vec<_>>();

    let mut score = 0;
    if primary == query {
        score += 120;
    } else if primary.starts_with(&query) {
        score += 95;
    } else if primary.contains(&query) {
        score += 70;
    } else if description.contains(&query) {
        score += 30;
    }

    for token in tokens {
        if primary == token {
            score += 50;
        } else if primary.starts_with(token) {
            score += 30;
        } else if primary.contains(token) {
            score += 18;
        } else if description.contains(token) {
            score += 8;
        }
    }

    score
}

fn print_terminal_results(query: &str, results: &[SearchResult]) {
    if results.is_empty() {
        println!("No matches for \"{query}\".");
        println!("Try a broader term such as `network`, `disk`, or `plugin`.");
        return;
    }

    println!("Search results for \"{query}\"");
    println!();

    let builtin = results
        .iter()
        .filter(|item| matches!(item.kind, SearchResultKind::Builtin))
        .collect::<Vec<_>>();
    let plugins = results
        .iter()
        .filter(|item| matches!(item.kind, SearchResultKind::Plugin))
        .collect::<Vec<_>>();

    if !builtin.is_empty() {
        println!("Built-in commands");
        for item in builtin {
            println!("  {:<20} {}", item.command, item.description);
        }
        println!();
    }

    if !plugins.is_empty() {
        println!("Plugins");
        for item in plugins {
            let mut flags = vec![item.source.clone()];
            if item.installed {
                flags.push("installed".to_string());
            }
            if item.trusted {
                flags.push("trusted".to_string());
            }
            println!(
                "  {:<20} {} [{}]",
                item.command,
                item.description,
                flags.join(", ")
            );
        }
    }
}

impl SearchResult {
    fn kind_rank(&self) -> i32 {
        match self.kind {
            SearchResultKind::Builtin => 0,
            SearchResultKind::Plugin => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::score_match;

    #[test]
    fn exact_match_scores_above_description_match() {
        let exact = score_match("network", "network", "Show local network information");
        let description = score_match("network", "diagnostic", "Run network diagnostics");
        assert!(exact > description);
    }
}
