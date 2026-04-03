use std::io;
use std::time::Instant;

use reqwest::blocking::Client;
use serde::Serialize;

use crate::builtins::format_bytes;
use crate::output::{error_prefix, success_prefix};

const SPEEDTEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=5000000";
const SPEEDTEST_LABEL: &str = "Cloudflare";

#[derive(Serialize)]
struct NetworkSpeedView {
    target: String,
    url: String,
    bytes_downloaded: u64,
    duration_ms: f64,
    estimated_mbps: f64,
    time_to_first_byte_ms: f64,
}

pub fn show_network_speed() -> Result<(), String> {
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?;

    let request_started = Instant::now();
    let mut response = client
        .get(SPEEDTEST_URL)
        .send()
        .map_err(|err| format!("Failed to start network speed test: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Network speed test request failed: {err}"))?;
    let first_byte_ms = request_started.elapsed().as_secs_f64() * 1000.0;

    let download_started = Instant::now();
    let bytes_downloaded = io::copy(&mut response, &mut io::sink())
        .map_err(|err| format!("Failed to read speed test response: {err}"))?;
    let duration_ms = download_started.elapsed().as_secs_f64() * 1000.0;
    let estimated_mbps = if duration_ms > 0.0 {
        (bytes_downloaded as f64 * 8.0) / (duration_ms / 1000.0) / 1_000_000.0
    } else {
        0.0
    };

    let view = NetworkSpeedView {
        target: SPEEDTEST_LABEL.to_string(),
        url: SPEEDTEST_URL.to_string(),
        bytes_downloaded,
        duration_ms,
        estimated_mbps,
        time_to_first_byte_ms: first_byte_ms,
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("{} Target: {}", success_prefix(), view.target);
    println!(
        "{} Downloaded: {}",
        success_prefix(),
        format_bytes(view.bytes_downloaded)
    );
    println!(
        "{} Time to first byte: {:.1} ms",
        success_prefix(),
        view.time_to_first_byte_ms
    );
    println!(
        "{} Transfer time: {:.1} ms",
        success_prefix(),
        view.duration_ms
    );
    if view.estimated_mbps > 0.0 {
        println!(
            "{} Estimated download speed: {:.2} Mbps",
            success_prefix(),
            view.estimated_mbps
        );
    } else {
        println!("{} Estimated download speed: unavailable", error_prefix());
    }

    Ok(())
}
