use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tinfo_plugin::{
    testing::{MockHost, TestRunner},
    Capability, CommandInput, Plugin, PluginCommand, PluginResult,
};

fn build_plugin() -> Plugin {
    fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
        let location = ctx
            .config
            .string("location")?
            .unwrap_or_else(|| "auto".to_string());
        ctx.output().kv("Location", location);
        Ok(())
    }

    Plugin::new("daily-brief")
        .capability(Capability::Config)
        .command(PluginCommand::new("status").handler(status))
}

#[test]
fn status_reads_location_from_typed_config() {
    let run = TestRunner::new(build_plugin())
        .host(
            MockHost::default()
                .cache_dir(unique_temp_dir("daily-brief-cache"))
                .config_json(json!({ "location": "tokyo" })),
        )
        .args(["status"])
        .run()
        .expect("plugin should run");

    assert!(run.stdout.contains("Location: tokyo"));
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
