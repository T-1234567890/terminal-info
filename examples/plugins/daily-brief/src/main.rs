use serde::{Deserialize, Serialize};
use tinfo_plugin::{Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel};

const CACHE_KEY: &str = "daily-brief.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BriefSnapshot {
    location: String,
    previous_location: Option<String>,
    run_count: u64,
}

fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    let snapshot = load_snapshot(&ctx)?;

    ctx.output().section("Daily Brief");
    ctx.output().status(StatusLevel::Ok, "brief ready");
    ctx.output().kv("Location", &snapshot.location);
    if let Some(previous) = &snapshot.previous_location {
        ctx.output().kv("Previous location", previous);
    }
    ctx.output().kv("Run count", snapshot.run_count);
    ctx.output().list([
        "Uses typed config access via ctx.config",
        "Persists state with ctx.cache",
    ]);
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&load_snapshot(&ctx)?)?;
    Ok(())
}

fn load_snapshot(ctx: &tinfo_plugin::Context) -> PluginResult<BriefSnapshot> {
    let location = ctx
        .config
        .string("location")?
        .unwrap_or_else(|| "auto".to_string());
    let cached = ctx.cache.read_json::<BriefSnapshot>(CACHE_KEY)?;
    let snapshot = BriefSnapshot {
        previous_location: cached.as_ref().map(|item| item.location.clone()),
        run_count: cached.map(|item| item.run_count + 1).unwrap_or(1),
        location,
    };
    ctx.cache.write_json(CACHE_KEY, &snapshot)?;
    Ok(snapshot)
}

fn main() {
    Plugin::new("daily-brief")
        .description("Official example plugin for config, cache, and testing")
        .author("Terminal Info")
        .capabilities([Capability::Config, Capability::Cache])
        .command(
            PluginCommand::new("status")
                .description("Show the current brief")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Emit the cached snapshot as JSON")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}
