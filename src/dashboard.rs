use crate::builtins::build_dashboard_snapshot;
use crate::config::Config;
use crate::output::{OutputMode, output_mode};

pub fn show_dashboard(config: &Config) -> Result<(), String> {
    let title = "Terminal Info";
    let snapshot = build_dashboard_snapshot(config);
    let location = if config.uses_auto_location() {
        "auto"
    } else {
        config.configured_location().unwrap_or("unknown")
    };

    match output_mode() {
        OutputMode::Compact => {
            println!(
                "location={} time={} weather={} net={} cpu={} mem={}",
                location,
                snapshot.time,
                snapshot.weather.as_deref().unwrap_or("unavailable"),
                snapshot.network,
                snapshot.cpu,
                snapshot.memory
            );
        }
        OutputMode::Plain => {
            println!("{title}");
            println!("Location: {location}");
            println!(
                "Weather: {}",
                snapshot.weather.as_deref().unwrap_or("unavailable")
            );
            println!("Time: {}", snapshot.time);
            println!("Network: {}", snapshot.network);
            println!("CPU: {}", snapshot.cpu);
            println!("Memory: {}", snapshot.memory);
        }
        OutputMode::Color => {
            let border = format!("+{}+", "-".repeat(title.len() + 2));
            println!("{border}");
            println!("| {title} |");
            println!("{border}");
            println!("Location: {location}");
            println!(
                "Weather: {}",
                snapshot.weather.as_deref().unwrap_or("unavailable")
            );
            println!("Time: {}", snapshot.time);
            println!("Network: {}", snapshot.network);
            println!("CPU: {}", snapshot.cpu);
            println!("Memory: {}", snapshot.memory);
        }
    }

    Ok(())
}
