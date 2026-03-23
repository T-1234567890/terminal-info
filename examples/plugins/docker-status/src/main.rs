use serde::Serialize;
use tinfo_plugin::{Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel};

#[derive(Serialize)]
struct DockerSnapshot {
    running_containers: u8,
}

fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().section("Docker");
    ctx.output().status(StatusLevel::Running, "3 containers running");
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&DockerSnapshot {
        running_containers: 3,
    })?;
    Ok(())
}

fn main() {
    Plugin::new("docker-status")
        .description("Official example docker status plugin")
        .author("Terminal Info")
        .capability(Capability::Filesystem)
        .command(
            PluginCommand::new("status")
                .description("Show a status summary")
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
