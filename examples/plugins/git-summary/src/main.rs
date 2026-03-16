use serde::Serialize;
use tinfo_plugin::{Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel};

#[derive(Serialize)]
struct Snapshot {
    repository: &'static str,
    host: String,
}

fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().section("Git");
    ctx.output().status(StatusLevel::Ok, "repository is clean");
    ctx.output().kv("Host", ctx.host.version());
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&Snapshot {
        repository: "clean",
        host: ctx.host.version(),
    })?;
    Ok(())
}

fn main() {
    Plugin::new("git-summary")
        .description("Official example git summary plugin")
        .author("Terminal Info")
        .capability(Capability::Filesystem)
        .command(
            PluginCommand::new("status")
                .description("Show a friendly status summary")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Emit JSON plugin state")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}
