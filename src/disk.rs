use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use sysinfo::Disks;

use crate::output::{error_prefix, success_prefix, warn_prefix};

#[derive(Clone, Debug, Serialize)]
struct DiskHealthReport {
    disk: String,
    mount: String,
    model: String,
    disk_type: String,
    interface: String,
    total_bytes: u64,
    available_bytes: u64,
    health_score: u8,
    temperature_c: Option<f64>,
    thermal_status: String,
    smart_status: String,
    health_label: String,
    status_summary: String,
    warnings: Vec<String>,
    reallocated_sectors: Option<u64>,
    media_errors: Option<u64>,
    wear_level: Option<u64>,
    power_on_hours: Option<u64>,
    disk_errors: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct SmartDetails {
    model: Option<String>,
    disk_type: Option<String>,
    interface: Option<String>,
    smart_status: Option<String>,
    temperature_c: Option<f64>,
    reallocated_sectors: Option<u64>,
    media_errors: Option<u64>,
    wear_level: Option<u64>,
    power_on_hours: Option<u64>,
    disk_errors: Option<u64>,
}

pub fn show_disk_overview() -> Result<(), String> {
    let reports = collect_disk_reports();
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    if reports.is_empty() {
        println!("No disks detected.");
        return Ok(());
    }

    for report in reports {
        print_summary(&report);
        println!();
    }
    Ok(())
}

pub fn show_disk_health() -> Result<(), String> {
    show_disk_overview()
}

pub fn show_disk_smart() -> Result<(), String> {
    let reports = collect_disk_reports();
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    if reports.is_empty() {
        println!("No disks detected.");
        return Ok(());
    }

    for report in reports {
        println!("Disk: {}", report.disk);
        println!("Model: {}", report.model);
        println!(
            "Type: {}",
            format_disk_identity(&report.disk_type, &report.interface)
        );
        println!("Capacity: {}", format_bytes(report.total_bytes));
        println!("Mount: {}", report.mount);
        println!("SMART status: {}", report.smart_status);
        println!(
            "Temperature: {}",
            report
                .temperature_c
                .map(|value| format!("{value:.1}°C"))
                .unwrap_or_else(|| "unavailable".to_string())
        );
        println!(
            "Reallocated sectors: {}",
            optional_u64(report.reallocated_sectors)
        );
        println!("Media errors: {}", optional_u64(report.media_errors));
        println!("Wear level: {}", optional_percent(report.wear_level));
        println!("Power-on hours: {}", optional_u64(report.power_on_hours));
        println!("Disk errors: {}", optional_u64(report.disk_errors));
        println!();
    }
    Ok(())
}

pub fn show_disk_temperature() -> Result<(), String> {
    let reports = collect_disk_reports();
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    if reports.is_empty() {
        println!("No disks detected.");
        return Ok(());
    }

    for report in reports {
        let prefix = match report.thermal_status.as_str() {
            "normal" => success_prefix(),
            "warm" => warn_prefix(),
            "hot" => error_prefix(),
            _ => warn_prefix(),
        };
        println!(
            "{} {}: {} ({})",
            prefix,
            report.disk,
            report
                .temperature_c
                .map(|value| format!("{value:.1}°C"))
                .unwrap_or_else(|| "unavailable".to_string()),
            report.thermal_status
        );
    }
    Ok(())
}

pub fn show_disk_reliability() -> Result<(), String> {
    let reports = collect_disk_reports();
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    if reports.is_empty() {
        println!("No disks detected.");
        return Ok(());
    }

    for report in reports {
        println!("Disk: {}", report.disk);
        println!("Model: {}", report.model);
        println!(
            "Type: {}",
            format_disk_identity(&report.disk_type, &report.interface)
        );
        println!("Capacity: {}", format_bytes(report.total_bytes));
        println!("Health score: {}%", report.health_score);
        println!("Health: {}", report.health_label);
        println!("Status: {}", report.status_summary);
        println!("SMART status: {}", report.smart_status);
        println!(
            "Reallocated sectors: {}",
            optional_u64(report.reallocated_sectors)
        );
        println!("Media errors: {}", optional_u64(report.media_errors));
        println!("Wear level: {}", optional_percent(report.wear_level));
        println!("Power-on hours: {}", optional_u64(report.power_on_hours));
        println!("Temperature: {}", report.thermal_status);
        println!(
            "Warnings: {}",
            if report.warnings.is_empty() {
                "none".to_string()
            } else {
                report.warnings.join(", ")
            }
        );
        println!();
    }
    Ok(())
}

fn collect_disk_reports() -> Vec<DiskHealthReport> {
    let disks = Disks::new_with_refreshed_list();
    let mut seen = HashSet::new();
    disks
        .list()
        .iter()
        .filter(|disk| !should_skip_mount(disk.mount_point()))
        .filter(|disk| {
            let name = disk.name().to_string_lossy();
            let key = format!(
                "{}:{}",
                if name.is_empty() {
                    disk.mount_point().display().to_string()
                } else {
                    name.to_string()
                },
                disk.total_space()
            );
            seen.insert(key)
        })
        .map(|disk| {
            let mount = disk.mount_point().display().to_string();
            let disk_name = if disk.name().is_empty() {
                mount.clone()
            } else {
                disk.name().to_string_lossy().to_string()
            };
            let smart = smart_details_for_mount(disk.mount_point());
            let warnings = reliability_warnings(&smart);
            let health_score = compute_health_score(&smart);
            DiskHealthReport {
                disk: disk_name,
                mount,
                model: smart.model.clone().unwrap_or_else(|| "unknown".to_string()),
                disk_type: smart
                    .disk_type
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                interface: smart
                    .interface
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
                health_score,
                temperature_c: smart.temperature_c,
                thermal_status: thermal_status(smart.temperature_c),
                smart_status: smart
                    .smart_status
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
                health_label: health_label(health_score).to_string(),
                status_summary: reliability_summary(&warnings, &smart),
                warnings,
                reallocated_sectors: smart.reallocated_sectors,
                media_errors: smart.media_errors,
                wear_level: smart.wear_level,
                power_on_hours: smart.power_on_hours,
                disk_errors: smart.disk_errors,
            }
        })
        .collect()
}

fn should_skip_mount(mount: &Path) -> bool {
    if cfg!(target_os = "macos") {
        let text = mount.display().to_string();
        return text.starts_with("/System/Volumes/")
            || text.starts_with("/private/var/vm")
            || text.starts_with("/private/var/run");
    }
    false
}

fn print_summary(report: &DiskHealthReport) {
    println!("Disk: {}", report.disk);
    println!("Model: {}", report.model);
    println!(
        "Type: {}",
        format_disk_identity(&report.disk_type, &report.interface)
    );
    println!("Capacity: {}", format_bytes(report.total_bytes));
    println!(
        "Temperature: {}",
        report
            .temperature_c
            .map(|value| format!("{value:.1}°C"))
            .unwrap_or_else(|| "unavailable".to_string())
    );
    println!();
    println!("SMART summary");
    println!("Health score: {}%", report.health_score);
    println!("SMART status: {}", report.smart_status);
    println!("Power-on hours: {}", optional_hours(report.power_on_hours));
    println!("Wear level: {}", optional_percent(report.wear_level));
    println!("Media errors: {}", optional_u64(report.media_errors));
    println!(
        "Reallocated sectors: {}",
        optional_u64(report.reallocated_sectors)
    );
    println!();
    println!("Health interpretation");
    println!("Health: {}", report.health_label);
    println!("Status: {}", report.status_summary);
    println!(
        "Warnings: {}",
        if report.warnings.is_empty() {
            "none".to_string()
        } else {
            report.warnings.join(", ")
        }
    );
}

fn optional_hours(value: Option<u64>) -> String {
    value
        .map(|value| format!("{value}h"))
        .unwrap_or_else(|| "unavailable".to_string())
}

fn optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn optional_percent(value: Option<u64>) -> String {
    value
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "unavailable".to_string())
}

fn smart_details_for_mount(mount: &Path) -> SmartDetails {
    #[cfg(target_os = "macos")]
    {
        smart_details_macos(mount)
    }
    #[cfg(target_os = "linux")]
    {
        smart_details_linux(mount)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = mount;
        SmartDetails::default()
    }
}

#[cfg(target_os = "macos")]
fn smart_details_macos(mount: &Path) -> SmartDetails {
    let mut details = SmartDetails::default();
    let output = Command::new("diskutil").arg("info").arg(mount).output();
    let Ok(output) = output else {
        return details;
    };
    if !output.status.success() {
        return details;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    details.model = text
        .lines()
        .find_map(|line| {
            line.split_once("Device / Media Name:")
                .map(|(_, value)| value.trim().to_string())
        })
        .or_else(|| {
            text.lines().find_map(|line| {
                line.split_once("Volume Name:")
                    .map(|(_, value)| value.trim().to_string())
            })
        });
    details.disk_type = text
        .lines()
        .find_map(|line| {
            line.split_once("Solid State:")
                .map(|(_, value)| value.trim().to_string())
        })
        .map(|value| {
            if value.eq_ignore_ascii_case("yes") {
                "SSD".to_string()
            } else {
                "HDD".to_string()
            }
        });
    details.interface = text
        .lines()
        .find_map(|line| {
            line.split_once("Protocol:")
                .map(|(_, value)| value.trim().to_string())
        })
        .or_else(|| {
            text.lines().find_map(|line| {
                line.split_once("Bus Protocol:")
                    .map(|(_, value)| value.trim().to_string())
            })
        });
    let device = text.lines().find_map(|line| {
        line.split_once("Device Node:")
            .map(|(_, value)| value.trim().to_string())
    });
    details.smart_status = text.lines().find_map(|line| {
        line.split_once("SMART Status:")
            .map(|(_, value)| value.trim().to_string())
    });

    if let Some(device) = device.as_deref().and_then(base_device_name) {
        let smart = smartctl_details(device);
        merge_smart_details(&mut details, smart);
    }
    details
}

#[cfg(target_os = "linux")]
fn smart_details_linux(mount: &Path) -> SmartDetails {
    let source = Command::new("findmnt")
        .args(["-no", "SOURCE", "--target"])
        .arg(mount)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    let Some(source) = source else {
        return SmartDetails::default();
    };
    let Some(device) = base_device_name(&source) else {
        return SmartDetails::default();
    };
    let mut details = smartctl_details(device);
    let lsblk = Command::new("lsblk")
        .args(["-ndo", "MODEL,ROTA,TRAN", device])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    if let Some(line) = lsblk {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if !parts.is_empty() {
            details.model = Some(parts[0..parts.len().saturating_sub(2)].join(" "));
        }
        if let Some(rota) = parts.get(parts.len().saturating_sub(2)) {
            details.disk_type = Some(if *rota == "0" { "SSD" } else { "HDD" }.to_string());
        }
        if let Some(transport) = parts.last() {
            details.interface = Some((*transport).to_string());
        }
    }
    details
}

fn merge_smart_details(target: &mut SmartDetails, from: SmartDetails) {
    if target.model.is_none() {
        target.model = from.model;
    }
    if target.disk_type.is_none() {
        target.disk_type = from.disk_type;
    }
    if target.interface.is_none() {
        target.interface = from.interface;
    }
    if target.smart_status.is_none() {
        target.smart_status = from.smart_status;
    }
    if target.temperature_c.is_none() {
        target.temperature_c = from.temperature_c;
    }
    if target.reallocated_sectors.is_none() {
        target.reallocated_sectors = from.reallocated_sectors;
    }
    if target.media_errors.is_none() {
        target.media_errors = from.media_errors;
    }
    if target.wear_level.is_none() {
        target.wear_level = from.wear_level;
    }
    if target.power_on_hours.is_none() {
        target.power_on_hours = from.power_on_hours;
    }
    if target.disk_errors.is_none() {
        target.disk_errors = from.disk_errors;
    }
}

fn smartctl_details(device: &str) -> SmartDetails {
    let output = Command::new("smartctl").args(["-H", "-A", device]).output();
    let Ok(output) = output else {
        return SmartDetails::default();
    };
    if !output.status.success() {
        return SmartDetails::default();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut details = SmartDetails::default();
    details.model = text.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(label, _)| {
                label.contains("Device Model")
                    || label.contains("Model Number")
                    || label.contains("Product")
            })
            .map(|(_, value)| value.trim().to_string())
    });
    details.interface = text.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(label, _)| {
                label.contains("Transport protocol") || label.contains("SATA Version")
            })
            .map(|(_, value)| value.trim().to_string())
    });
    details.smart_status = text
        .lines()
        .find_map(parse_smart_status)
        .map(str::to_string);
    details.temperature_c = text.lines().find_map(parse_temperature);
    details.reallocated_sectors = text
        .lines()
        .find_map(|line| parse_named_u64(line, &["Reallocated_Sector_Ct"]));
    details.media_errors = text.lines().find_map(|line| {
        parse_named_u64(
            line,
            &[
                "Media and Data Integrity Errors",
                "Reported_Uncorrect",
                "Offline_Uncorrectable",
            ],
        )
    });
    details.wear_level = text.lines().find_map(|line| {
        parse_named_u64(
            line,
            &[
                "Percentage Used",
                "Wear_Leveling_Count",
                "Percent_Lifetime_Remain",
            ],
        )
    });
    details.power_on_hours = text
        .lines()
        .find_map(|line| parse_named_u64(line, &["Power_On_Hours"]));
    details.disk_errors = text.lines().find_map(|line| {
        parse_named_u64(
            line,
            &[
                "Error Information Log Entries",
                "ATA Error Count",
                "CRC_Error_Count",
            ],
        )
    });
    details.disk_type = Some(infer_disk_type(&text, device).to_string());
    details
}

fn parse_smart_status(line: &str) -> Option<&str> {
    if line.contains("SMART overall-health self-assessment test result:")
        || line.contains("SMART Health Status:")
        || line.contains("SMART overall-health")
    {
        return line.split(':').next_back().map(str::trim);
    }
    None
}

fn parse_temperature(line: &str) -> Option<f64> {
    if !line.contains("Temperature") && !line.contains("Airflow_Temperature_Cel") {
        return None;
    }
    line.split_whitespace().rev().find_map(|part| {
        part.trim_matches(|ch: char| !ch.is_ascii_digit())
            .parse::<f64>()
            .ok()
    })
}

fn parse_named_u64(line: &str, labels: &[&str]) -> Option<u64> {
    if !labels.iter().any(|label| line.contains(label)) {
        return None;
    }
    line.split_whitespace()
        .rev()
        .find_map(|part| part.parse::<u64>().ok())
}

fn base_device_name(value: &str) -> Option<&str> {
    #[cfg(target_os = "macos")]
    {
        let trimmed = value.trim();
        let disk_index = trimmed.rfind("disk")?;
        let suffix = &trimmed[disk_index..];
        let mut cut = suffix.len();
        if let Some(s_index) = suffix.rfind('s') {
            if suffix[s_index + 1..].chars().all(|ch| ch.is_ascii_digit()) {
                cut = s_index;
            }
        }
        return Some(&trimmed[..disk_index + cut]);
    }
    #[cfg(target_os = "linux")]
    {
        let trimmed = value.trim();
        if let Some(rest) = trimmed.strip_prefix("/dev/") {
            if rest.starts_with("nvme") {
                let bytes = trimmed.as_bytes();
                for index in (0..bytes.len()).rev() {
                    if bytes[index] == b'p'
                        && trimmed[index + 1..].chars().all(|ch| ch.is_ascii_digit())
                    {
                        return Some(&trimmed[..index]);
                    }
                }
            }
            let cut = trimmed.trim_end_matches(|ch: char| ch.is_ascii_digit());
            return Some(cut);
        }
        Some(trimmed)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = value;
        None
    }
}

fn compute_health_score(details: &SmartDetails) -> u8 {
    let mut score = 100_i32;
    if let Some(status) = &details.smart_status {
        let lowered = status.to_ascii_lowercase();
        if lowered.contains("fail") {
            score -= 45;
        } else if lowered.contains("warn") {
            score -= 20;
        }
    } else {
        score -= 5;
    }
    if let Some(temp) = details.temperature_c {
        if temp >= 60.0 {
            score -= 25;
        } else if temp >= 50.0 {
            score -= 12;
        }
    }
    if details.reallocated_sectors.unwrap_or(0) > 0 {
        score -= 20;
    }
    if details.media_errors.unwrap_or(0) > 0 {
        score -= 20;
    }
    if let Some(wear) = details.wear_level {
        if wear >= 90 {
            score -= 20;
        } else if wear >= 75 {
            score -= 10;
        }
    }
    score.clamp(0, 100) as u8
}

fn health_label(score: u8) -> &'static str {
    match score {
        95..=100 => "Excellent",
        85..=94 => "Good",
        70..=84 => "Fair",
        50..=69 => "Watch",
        _ => "Poor",
    }
}

fn reliability_summary(warnings: &[String], details: &SmartDetails) -> String {
    if warnings.is_empty() {
        return "No reliability indicators detected".to_string();
    }
    if details.media_errors.unwrap_or(0) > 0 || details.reallocated_sectors.unwrap_or(0) > 0 {
        return "Drive has reliability indicators that should be monitored".to_string();
    }
    if details.temperature_c.unwrap_or(0.0) >= 50.0 {
        return "Drive is running warmer than expected".to_string();
    }
    "Drive has warning indicators that deserve attention".to_string()
}

fn thermal_status(temp: Option<f64>) -> String {
    match temp {
        Some(value) if value >= 60.0 => "hot".to_string(),
        Some(value) if value >= 45.0 => "warm".to_string(),
        Some(_) => "normal".to_string(),
        None => "unknown".to_string(),
    }
}

fn reliability_warnings(details: &SmartDetails) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some(status) = &details.smart_status {
        let lowered = status.to_ascii_lowercase();
        if lowered.contains("fail") {
            warnings.push("SMART reported a failure".to_string());
        } else if lowered.contains("warn") {
            warnings.push("SMART reported a warning".to_string());
        }
    }
    if details.reallocated_sectors.unwrap_or(0) > 0 {
        warnings.push("reallocated sectors detected".to_string());
    }
    if details.media_errors.unwrap_or(0) > 0 {
        warnings.push("media errors detected".to_string());
    }
    if let Some(temp) = details.temperature_c {
        if temp >= 60.0 {
            warnings.push("temperature is very high".to_string());
        } else if temp >= 50.0 {
            warnings.push("temperature is elevated".to_string());
        }
    }
    if let Some(wear) = details.wear_level {
        if wear >= 90 {
            warnings.push("wear level is very high".to_string());
        }
    }
    warnings
}

fn infer_disk_type(text: &str, device: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains("nvme") || device.contains("nvme") {
        "SSD"
    } else if lower.contains("solid state") || lower.contains("ssd") {
        "SSD"
    } else if lower.contains("rotation rate") || lower.contains("rpm") {
        "HDD"
    } else {
        "Disk"
    }
}

fn format_disk_identity(disk_type: &str, interface: &str) -> String {
    match (
        disk_type.eq_ignore_ascii_case("unknown"),
        interface.eq_ignore_ascii_case("unknown"),
    ) {
        (true, true) => "unknown".to_string(),
        (false, true) => disk_type.to_string(),
        (true, false) => interface.to_string(),
        (false, false) => format!("{interface} {disk_type}"),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_score_penalizes_bad_signals() {
        let details = SmartDetails {
            model: None,
            disk_type: None,
            interface: None,
            smart_status: Some("FAILED".to_string()),
            temperature_c: Some(65.0),
            reallocated_sectors: Some(12),
            media_errors: Some(1),
            wear_level: Some(95),
            power_on_hours: None,
            disk_errors: None,
        };
        assert!(compute_health_score(&details) < 40);
    }
}
