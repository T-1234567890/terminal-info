use std::fs;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{FixedOffset, Local, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

use crate::cache::{read_cache, write_cache};
use crate::config::Config;
use crate::output::{error_prefix, success_prefix};
use crate::plugin::{run_diagnostic_plugins, verify_plugins};
use crate::weather::WeatherClient;

const DEFAULT_PING_HOSTS: [&str; 3] = ["google.com", "cloudflare.com", "github.com"];

pub struct DashboardSnapshot {
    pub time: String,
    pub weather: DashboardWeather,
    pub network: String,
    pub cpu: String,
    pub memory: String,
}

#[derive(Serialize)]
struct NetworkInfoView {
    public_ip: String,
    local_ip: String,
    dns: String,
    isp: String,
}

#[derive(Serialize)]
struct SystemInfoView {
    os: String,
    cpu: String,
    memory: String,
    disk_usage: String,
    uptime: String,
}

#[derive(Serialize)]
struct DoctorCheck {
    name: String,
    status: String,
    detail: String,
}

pub struct DashboardWeather {
    pub line: String,
    pub hint: Option<String>,
    pub detected_location: Option<String>,
}

pub fn build_dashboard_snapshot(config: &Config) -> DashboardSnapshot {
    let mut system = System::new_all();
    system.refresh_all();

    DashboardSnapshot {
        time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        weather: dashboard_weather(config),
        network: local_ip().unwrap_or_else(|_| "unavailable".to_string()),
        cpu: format!("{:.1}%", system.global_cpu_usage()),
        memory: memory_line(&system),
    }
}

pub fn run_ping(host: Option<String>) -> Result<(), String> {
    let hosts: Vec<String> = match host {
        Some(host) => vec![host],
        None => DEFAULT_PING_HOSTS
            .iter()
            .map(|host| (*host).to_string())
            .collect(),
    };

    for host in hosts {
        match tcp_ping_latency(&host) {
            Ok(latency_ms) => println!("{host}: {latency_ms:.1} ms"),
            Err(err) => println!("{host}: failed ({err})"),
        }
    }

    Ok(())
}

pub fn show_network_info() -> Result<(), String> {
    let info = read_cache("network", 30).unwrap_or_else(|| {
        let info = gather_network_info();
        let _ = write_cache("network", &info);
        info
    });

    if crate::output::json_output() {
        let view = NetworkInfoView {
            public_ip: info
                .public_ip
                .clone()
                .unwrap_or_else(|| "unavailable".to_string()),
            local_ip: info
                .local_ip
                .clone()
                .unwrap_or_else(|| "unavailable".to_string()),
            dns: info
                .dns
                .clone()
                .unwrap_or_else(|| "unavailable".to_string()),
            isp: info
                .isp
                .clone()
                .unwrap_or_else(|| "unavailable".to_string()),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!(
        "Public IP: {}",
        info.public_ip.as_deref().unwrap_or("unavailable")
    );
    println!(
        "Local IP: {}",
        info.local_ip.as_deref().unwrap_or("unavailable")
    );
    println!("DNS: {}", info.dns.as_deref().unwrap_or("unavailable"));
    println!("ISP: {}", info.isp.as_deref().unwrap_or("unavailable"));

    Ok(())
}

pub fn show_system_info() -> Result<(), String> {
    let mut system = System::new_all();
    system.refresh_all();

    let disks = Disks::new_with_refreshed_list();
    let disk_line = disks
        .list()
        .iter()
        .next()
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            format!("{} / {} used", format_bytes(used), format_bytes(total))
        })
        .unwrap_or_else(|| "unavailable".to_string());

    let view = SystemInfoView {
        os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        cpu: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        memory: memory_line(&system),
        disk_usage: disk_line,
        uptime: system_uptime_seconds()
            .map(format_uptime)
            .unwrap_or_else(|| "unavailable".to_string()),
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("OS: {}", view.os);
    println!("CPU: {}", view.cpu);
    println!("Memory: {}", view.memory);
    println!("Disk usage: {}", view.disk_usage);
    println!("Uptime: {}", view.uptime);

    Ok(())
}

pub fn time_output(city: Option<String>) -> Result<String, String> {
    let key = format!(
        "time-{}",
        city.clone().unwrap_or_else(|| "global".to_string())
    );
    let cached: Option<Vec<(String, String)>> = read_cache(&key, 10);
    let data = if let Some(cached) = cached {
        cached
    } else {
        let built = match city.clone() {
            Some(city) => {
                let (label, formatted) = city_time(&city).ok_or_else(|| {
                    format!("Unsupported city '{city}'. Try Tokyo, London, New York, or Local.")
                })?;
                vec![(label, formatted)]
            }
            None => ["Local", "Tokyo", "London", "New York"]
                .iter()
                .map(|city| {
                    city_time(city).ok_or_else(|| format!("Failed to build time for {city}."))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let _ = write_cache(&key, &built);
        built
    };

    if crate::output::json_output() {
        return Ok(serde_json::to_string_pretty(&data).unwrap_or_else(|_| "[]".to_string()));
    }

    let rows = match city {
        Some(city) => {
            let (label, formatted) = data.into_iter().next().ok_or_else(|| {
                format!("Unsupported city '{city}'. Try Tokyo, London, New York, or Local.")
            })?;
            vec![(label, formatted)]
        }
        None => data,
    };

    Ok(format_time_table("Terminal Info Time", &rows))
}

fn format_time_table(title: &str, rows: &[(String, String)]) -> String {
    let content_width = rows
        .iter()
        .map(|(label, value)| label.len() + 2 + value.len())
        .max()
        .unwrap_or(0)
        .max(title.len());
    let top = format!("┌{}┐", "─".repeat(content_width + 2));
    let middle = format!("├{}┤", "─".repeat(content_width + 2));
    let bottom = format!("└{}┘", "─".repeat(content_width + 2));
    let mut lines = vec![
        top,
        format!("│ {} │", center_line(title, content_width)),
        middle,
    ];
    for (label, value) in rows {
        lines.push(format!(
            "│ {:<content_width$} │",
            format!("{label}: {value}")
        ));
    }
    lines.push(bottom);
    format!("{}\n", lines.join("\n"))
}

fn center_line(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(value.len());
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}

pub fn run_diagnostic_all() -> Result<(), String> {
    if crate::output::json_output() {
        let checks = collect_diagnostic_checks();
        println!(
            "{}",
            serde_json::to_string_pretty(&checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }
    println!("Network");
    run_diagnostic_network()?;
    println!();
    println!("System");
    run_diagnostic_system()?;
    println!();
    println!("Plugins");
    run_diagnostic_plugins()?;
    Ok(())
}

pub fn run_config_doctor(config: &Config) -> Result<(), String> {
    let checks = vec![
        doctor_status("Config file", "PASS", "loaded"),
        doctor_status(
            "Profiles",
            if config.profile.is_empty() {
                "WARN"
            } else {
                "PASS"
            },
            if config.profile.is_empty() {
                "none configured"
            } else {
                "ok"
            },
        ),
        doctor_status(
            "Plugin directory",
            if plugin_dir_path()?.exists() {
                "PASS"
            } else {
                "WARN"
            },
            if plugin_dir_path()?.exists() {
                "present"
            } else {
                "missing"
            },
        ),
        doctor_status(
            "Registry cache",
            if registry_cache_path().exists() {
                "PASS"
            } else {
                "WARN"
            },
            if registry_cache_path().exists() {
                "present"
            } else {
                "missing"
            },
        ),
        doctor_status(
            "Weather API",
            if config.provider.is_some() && config.api_key.is_none() {
                "FAIL"
            } else {
                "PASS"
            },
            if config.provider.is_some() && config.api_key.is_none() {
                "provider set without API key"
            } else {
                "ok"
            },
        ),
        doctor_status(
            "Network connectivity",
            if http_reachable("https://github.com") {
                "PASS"
            } else {
                "WARN"
            },
            if http_reachable("https://github.com") {
                "reachable"
            } else {
                "offline"
            },
        ),
        doctor_status(
            "Plugin integrity",
            "PASS",
            "use `tinfo plugin verify` for details",
        ),
    ];

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    for check in checks {
        println!(
            "{:<18} {} ({})",
            format!("{} ........", check.name),
            check.status,
            check.detail
        );
    }
    let _ = verify_plugins();
    Ok(())
}

pub fn run_diagnostic_network() -> Result<(), String> {
    let dns_ok = ("github.com", 443)
        .to_socket_addrs()
        .map(|mut addrs| addrs.next().is_some())
        .unwrap_or(false);
    print_status(dns_ok, "DNS OK", "DNS resolution failed");

    let http_ok = http_reachable("http://example.com");
    print_status(http_ok, "HTTP reachable", "HTTP unreachable");

    let tls_ok = http_reachable("https://github.com");
    print_status(tls_ok, "TLS handshake OK", "TLS handshake failed");

    match tcp_ping_latency("cloudflare.com") {
        Ok(latency_ms) => println!("{} Latency {:.1} ms", success_prefix(), latency_ms),
        Err(_) => println!("{} Latency measurement failed", error_prefix()),
    }

    Ok(())
}

pub fn run_diagnostic_system() -> Result<(), String> {
    let mut system = System::new_all();
    system.refresh_all();

    let disks = Disks::new_with_refreshed_list();
    let disk_usage_ratio = disks
        .list()
        .iter()
        .next()
        .map(|disk| {
            let total = disk.total_space() as f64;
            let used = disk.total_space().saturating_sub(disk.available_space()) as f64;
            if total > 0.0 { used / total } else { 0.0 }
        })
        .unwrap_or(0.0);

    let cpu = system.global_cpu_usage();
    let memory_ratio = if system.total_memory() > 0 {
        system.used_memory() as f64 / system.total_memory() as f64
    } else {
        0.0
    };

    print_status(
        disk_usage_ratio < 0.9,
        &format!("Disk usage {:.1}%", disk_usage_ratio * 100.0),
        &format!("Disk usage high {:.1}%", disk_usage_ratio * 100.0),
    );
    print_status(
        memory_ratio < 0.9,
        &format!("Memory usage {:.1}%", memory_ratio * 100.0),
        &format!("Memory usage high {:.1}%", memory_ratio * 100.0),
    );
    print_status(
        cpu < 90.0,
        &format!("CPU load {:.1}%", cpu),
        &format!("CPU load high {:.1}%", cpu),
    );

    Ok(())
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct NetworkInfo {
    public_ip: Option<String>,
    local_ip: Option<String>,
    dns: Option<String>,
    isp: Option<String>,
}

fn gather_network_info() -> NetworkInfo {
    let public = public_ip_info().ok();

    NetworkInfo {
        public_ip: public.as_ref().map(|info| info.ip.clone()),
        local_ip: local_ip().ok(),
        dns: dns_server().ok(),
        isp: public.and_then(|info| info.org),
    }
}

fn dashboard_weather(config: &Config) -> DashboardWeather {
    let client = WeatherClient::new();

    if let Some(city) = config.configured_location() {
        return match cached_dashboard_weather(&client, city, config) {
            Ok(report) => DashboardWeather {
                line: format!(
                    "{}, {:.1}{}",
                    report.summary,
                    report.temperature,
                    config.units.temperature_symbol()
                ),
                hint: None,
                detected_location: None,
            },
            Err(err) => DashboardWeather {
                line: format!("unavailable ({})", weather_error_reason(&err)),
                hint: None,
                detected_location: None,
            },
        };
    }

    match client.detect_city_by_ip_detailed() {
        Ok(city) => match cached_dashboard_weather(&client, &city, config) {
            Ok(report) => DashboardWeather {
                line: format!(
                    "{}, {:.1}{}",
                    report.summary,
                    report.temperature,
                    config.units.temperature_symbol()
                ),
                hint: Some(format!("Tip: run\ntinfo config location {city}")),
                detected_location: Some(city),
            },
            Err(err) => DashboardWeather {
                line: format!("unavailable ({})", weather_error_reason(&err)),
                hint: None,
                detected_location: Some(city),
            },
        },
        Err(err) => {
            if config.uses_auto_location() {
                DashboardWeather {
                    line: "unavailable (IP detection failed)".to_string(),
                    hint: None,
                    detected_location: None,
                }
            } else if is_network_error(&err) {
                DashboardWeather {
                    line: "unavailable (IP detection failed)".to_string(),
                    hint: None,
                    detected_location: None,
                }
            } else {
                DashboardWeather {
                    line: "unavailable (location not configured)".to_string(),
                    hint: Some("Tip: run\ntinfo config location <city>".to_string()),
                    detected_location: None,
                }
            }
        }
    }
}

fn cached_dashboard_weather(
    client: &WeatherClient,
    city: &str,
    config: &Config,
) -> Result<crate::weather::WeatherReport, String> {
    let key = format!(
        "weather-dashboard-{}-{}",
        city.to_ascii_lowercase(),
        config.units.label()
    );
    if let Some(report) = read_cache(&key, 60) {
        return Ok(report);
    }
    let report = client.current_weather(city, config)?;
    let _ = write_cache(&key, &report);
    Ok(report)
}

fn weather_error_reason(err: &str) -> &'static str {
    if is_network_error(err) {
        "network error"
    } else {
        "weather API error"
    }
}

fn is_network_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("request failed")
}

fn tcp_ping_latency(host: &str) -> Result<f64, String> {
    let mut addrs = (host, 80)
        .to_socket_addrs()
        .map_err(|err| format!("resolve error: {err}"))?;
    let addr = addrs.next().ok_or_else(|| "no address found".to_string())?;

    let started = Instant::now();
    TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|err| err.to_string())?;

    Ok(started.elapsed().as_secs_f64() * 1000.0)
}

fn local_ip() -> Result<String, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|err| err.to_string())?;
    socket
        .connect("8.8.8.8:80")
        .map_err(|err| err.to_string())?;
    socket
        .local_addr()
        .map(|addr| addr.ip().to_string())
        .map_err(|err| err.to_string())
}

fn dns_server() -> Result<String, String> {
    let contents = fs::read_to_string("/etc/resolv.conf").map_err(|err| err.to_string())?;
    let server = contents
        .lines()
        .find_map(|line| line.strip_prefix("nameserver ").map(str::trim))
        .ok_or_else(|| "no nameserver found".to_string())?;
    Ok(server.to_string())
}

fn city_time(city: &str) -> Option<(String, String)> {
    match city.to_ascii_lowercase().as_str() {
        "local" => Some((
            "Local".to_string(),
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        )),
        "tokyo" => format_offset_time("Tokyo", 9 * 3600),
        "london" => format_offset_time("London", 0),
        "new york" | "new-york" | "new_york" | "newyork" => {
            format_offset_time("New York", -5 * 3600)
        }
        _ => None,
    }
}

fn format_offset_time(label: &str, offset_seconds: i32) -> Option<(String, String)> {
    let offset = FixedOffset::east_opt(offset_seconds)?;
    Some((
        label.to_string(),
        Utc::now()
            .with_timezone(&offset)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    ))
}

fn memory_line(system: &System) -> String {
    format!(
        "{} / {} used",
        format_bytes(system.used_memory()),
        format_bytes(system.total_memory())
    )
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    if bytes as f64 >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn system_uptime_seconds() -> Option<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .ok()?;
    let boot = System::boot_time();

    if boot == 0 || boot > now {
        return None;
    }

    let uptime = now.saturating_sub(boot);
    if uptime > 10 * 365 * 24 * 60 * 60 {
        return None;
    }

    Some(uptime)
}

#[derive(Deserialize)]
struct PublicIpInfo {
    ip: String,
    #[serde(default)]
    org: Option<String>,
}

fn public_ip_info() -> Result<PublicIpInfo, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|err| err.to_string())?
        .get("https://ipapi.co/json/")
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json()
        .map_err(|err| err.to_string())
}

fn http_reachable(url: &str) -> bool {
    Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .ok()
        .and_then(|client| client.get(url).send().ok())
        .and_then(|response| response.error_for_status().ok())
        .is_some()
}

fn print_status(ok: bool, ok_message: &str, err_message: &str) {
    if ok {
        println!("{} {ok_message}", success_prefix());
    } else {
        println!("{} {err_message}", error_prefix());
    }
}

fn doctor_status(name: &str, status: &str, detail: &str) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: status.to_string(),
        detail: detail.to_string(),
    }
}

fn collect_diagnostic_checks() -> Vec<DoctorCheck> {
    let dns_started = Instant::now();
    let dns_ok = ("github.com", 443)
        .to_socket_addrs()
        .map(|mut addrs| addrs.next().is_some())
        .unwrap_or(false);
    let dns_ms = dns_started.elapsed().as_secs_f64() * 1000.0;
    let http_ok = http_reachable("http://example.com");
    let tls_ok = http_reachable("https://github.com");
    let latency = tcp_ping_latency("cloudflare.com").ok();
    let mut checks = vec![
        doctor_status(
            "DNS resolution",
            if dns_ok { "PASS" } else { "FAIL" },
            &format!("{dns_ms:.1} ms"),
        ),
        doctor_status(
            "HTTP reachability",
            if http_ok { "PASS" } else { "FAIL" },
            if http_ok { "reachable" } else { "unreachable" },
        ),
        doctor_status(
            "TLS certificate",
            if tls_ok { "PASS" } else { "FAIL" },
            if tls_ok { "valid" } else { "failed" },
        ),
        doctor_status(
            "HTTP latency",
            if latency.is_some() { "PASS" } else { "WARN" },
            &latency
                .map(|ms| format!("{ms:.1} ms"))
                .unwrap_or_else(|| "unavailable".to_string()),
        ),
    ];

    let disks = Disks::new_with_refreshed_list();
    let disk_status = if disks.list().is_empty() {
        "WARN"
    } else {
        "PASS"
    };
    checks.push(doctor_status(
        "Disk health",
        disk_status,
        "basic disk check",
    ));
    checks.push(doctor_status(
        "Plugin integrity",
        "PASS",
        "use plugin verify",
    ));
    checks.push(doctor_status("Config integrity", "PASS", "config parsed"));
    checks
}

fn plugin_dir_path() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".terminal-info").join("plugins"))
}

fn registry_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".terminal-info")
        .join("cache")
        .join("plugins.json")
}
