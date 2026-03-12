use crate::builtins::build_dashboard_snapshot;
use crate::config::Config;

pub fn show_dashboard(config: &Config) -> Result<(), String> {
    let title = "Terminal Info";
    let border = format!("+{}+", "-".repeat(title.len() + 2));
    let snapshot = build_dashboard_snapshot(config);

    println!("{border}");
    println!("| {title} |");
    println!("{border}");
    println!(
        "Location: {}",
        config.default_city.as_deref().unwrap_or("unknown")
    );
    println!(
        "Weather: {}",
        snapshot.weather.as_deref().unwrap_or("unavailable")
    );
    println!("Time: {}", snapshot.time);
    println!("Network: {}", snapshot.network);
    println!("CPU: {}", snapshot.cpu);
    println!("Memory: {}", snapshot.memory);

    Ok(())
}
