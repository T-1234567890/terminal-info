use std::env;

use serde::{Deserialize, Serialize};
use tinfo_plugin::{Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel};

const CACHE_KEY: &str = "remote-health.json";
const DEFAULT_URL: &str = "https://api.github.com";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HealthSnapshot {
    url: String,
    status: String,
    bytes: usize,
}

#[derive(Serialize)]
struct WidgetView {
    title: String,
    content: String,
}

fn status(ctx: tinfo_plugin::Context, args: CommandInput) -> PluginResult<()> {
    let snapshot = fetch_snapshot(&ctx, &args)?;
    let level = if snapshot.status == "reachable" {
        StatusLevel::Ok
    } else {
        StatusLevel::Warn
    };

    ctx.output().section("Remote Health");
    ctx.output().status(level, &snapshot.status);
    ctx.output().kv("URL", &snapshot.url);
    ctx.output().kv("Bytes", snapshot.bytes);
    if args.flag("--offline") {
        ctx.output().warn("offline mode uses the cached snapshot");
    }
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&fetch_snapshot(&ctx, &args)?)?;
    Ok(())
}

fn fetch_snapshot(ctx: &tinfo_plugin::Context, args: &CommandInput) -> PluginResult<HealthSnapshot> {
    if args.flag("--offline") {
        return ctx
            .cache
            .read_json::<HealthSnapshot>(CACHE_KEY)?
            .ok_or_else(|| "no cached snapshot available; run without --offline first".into());
    }

    let url = args
        .option("--url")
        .map(str::to_string)
        .or(ctx.config.string("plugin.remote_health.url")?)
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    let body = ctx
        .network
        .get(url.clone())
        .header("User-Agent", "tinfo-example-remote-health")
        .send_text()?;
    let snapshot = HealthSnapshot {
        url,
        status: "reachable".to_string(),
        bytes: body.len(),
    };
    ctx.cache.write_json(CACHE_KEY, &snapshot)?;
    Ok(snapshot)
}

fn emit_widget() -> PluginResult<()> {
    let cache_dir = env::var("TINFO_PLUGIN_CACHE_DIR").unwrap_or_else(|_| ".".to_string());
    let path = std::path::Path::new(&cache_dir)
        .join("remote-health")
        .join(CACHE_KEY);
    let snapshot = std::fs::read_to_string(path)
        .ok()
        .and_then(|value| serde_json::from_str::<HealthSnapshot>(&value).ok());
    let content = match snapshot {
        Some(snapshot) => format!("{} ({})", snapshot.status, snapshot.url),
        None => "no cached status yet".to_string(),
    };
    println!(
        "{}",
        serde_json::to_string(&WidgetView {
            title: "Remote Health".to_string(),
            content,
        })?
    );
    Ok(())
}

fn main() {
    if env::args().nth(1).as_deref() == Some("--widget") {
        if let Err(err) = emit_widget() {
            eprintln!("Plugin error: {err}");
            std::process::exit(1);
        }
        return;
    }

    Plugin::new("remote-health")
        .description("Official example plugin for network access and widgets")
        .author("Terminal Info")
        .capabilities([Capability::Network, Capability::Cache, Capability::Config])
        .command(
            PluginCommand::new("status")
                .description("Fetch and show endpoint health")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Emit the endpoint snapshot as JSON")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}
