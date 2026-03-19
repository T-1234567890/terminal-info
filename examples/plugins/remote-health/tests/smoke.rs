use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tinfo_plugin::{
    testing::{MockHost, TestRunner},
    Capability, CommandInput, Plugin, PluginCommand, PluginResult,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HealthSnapshot {
    url: String,
    status: String,
    bytes: usize,
}

fn build_plugin() -> Plugin {
    fn status(ctx: tinfo_plugin::Context, args: CommandInput) -> PluginResult<()> {
        let offline = args.flag("--offline");
        if offline {
            let snapshot = ctx
                .cache
                .read_json::<HealthSnapshot>("remote-health.json")?
                .expect("cached snapshot should exist for test");
            ctx.output().kv("Status", snapshot.status);
            return Ok(());
        }

        let url = ctx
            .config
            .string("plugin.remote_health.url")?
            .unwrap_or_else(|| "https://api.github.com".to_string());
        ctx.output().kv("URL", url);
        Ok(())
    }

    Plugin::new("remote-health")
        .capabilities([Capability::Config, Capability::Cache])
        .command(PluginCommand::new("status").handler(status))
}

#[test]
fn offline_status_uses_cached_snapshot() {
    let cache_dir = unique_temp_dir("remote-health-cache");
    std::fs::create_dir_all(cache_dir.join("plugin")).expect("cache dir should be created");
    std::fs::write(
        cache_dir.join("plugin").join("remote-health.json"),
        serde_json::to_string(&HealthSnapshot {
            url: "https://example.com".to_string(),
            status: "reachable".to_string(),
            bytes: 128,
        })
        .expect("snapshot should serialize"),
    )
    .expect("cache file should be written");

    let run = TestRunner::new(build_plugin())
        .host(
            MockHost::default()
                .cache_dir(cache_dir)
                .config_json(json!({ "plugin": { "remote_health": { "url": "https://example.com" }}})),
        )
        .args(["status", "--offline"])
        .run()
        .expect("plugin should run");

    assert!(run.stdout.contains("Status: reachable"));
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after unix epoch")
            .as_nanos()
    ))
}
