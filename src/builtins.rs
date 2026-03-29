use std::env;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{FixedOffset, Local, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

use crate::cache::{read_cache, write_cache};
use crate::config::{Config, config_path, home_dir_path};
use crate::migration::inspect_migration_status;
use crate::output::{error_prefix, success_prefix, warn_prefix};
use crate::plugin::{plugin_diagnostic_summary, run_diagnostic_plugins, verify_plugins};
use crate::theme::format_box_table;
use crate::weather::WeatherClient;

const DEFAULT_PING_HOSTS: [&str; 3] = ["google.com", "cloudflare.com", "github.com"];
const FULL_PING_HOSTS: [&str; 10] = [
    "google.com",
    "cloudflare.com",
    "github.com",
    "1.1.1.1",
    "8.8.8.8",
    "dns.google",
    "fastly.com",
    "amazon.com",
    "microsoft.com",
    "apple.com",
];
const SERVER_FULL_PING_HOSTS: [&str; 15] = [
    "google.com",
    "cloudflare.com",
    "github.com",
    "1.1.1.1",
    "8.8.8.8",
    "dns.google",
    "quad9.net",
    "opendns.com",
    "fastly.com",
    "akamai.com",
    "amazon.com",
    "azure.com",
    "microsoft.com",
    "apple.com",
    "oracle.com",
];
const SERVER_API_ENDPOINTS: [(&str, &str); 6] = [
    ("GitHub API", "https://api.github.com"),
    ("Weather API", "https://api.open-meteo.com/v1/forecast"),
    ("IP geolocation API", "https://ipapi.co/json/"),
    ("Cloudflare DoH", "https://cloudflare-dns.com/dns-query"),
    ("Google DoH", "https://dns.google/resolve"),
    (
        "Plugin registry",
        "https://raw.githubusercontent.com/T-1234567890/terminal-info/main/plugins/index.json",
    ),
];
const NORMAL_API_ENDPOINTS: [(&str, &str); 2] = [
    ("GitHub API", "https://api.github.com"),
    ("Weather API", "https://api.open-meteo.com/v1/forecast"),
];

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
    severity: String,
    detail: String,
    fix: String,
}

#[derive(Serialize)]
struct LatencyCheck {
    endpoint: String,
    latency_ms: Option<f64>,
    status: String,
    error: Option<String>,
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

pub fn run_ping(host: Option<String>, server_mode: bool) -> Result<(), String> {
    let checks = collect_latency_checks(host.as_deref(), server_mode);
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    for check in &checks {
        match (check.latency_ms, check.error.as_deref()) {
            (Some(latency_ms), _) => {
                println!(
                    "{} {}: {latency_ms:.1} ms",
                    success_prefix(),
                    check.endpoint
                )
            }
            (None, Some(err)) => {
                println!("{} {}: failed ({err})", error_prefix(), check.endpoint)
            }
            (None, None) => println!("{} {}: failed", error_prefix(), check.endpoint),
        }
    }

    if matches!(host.as_deref(), Some(mode) if mode.eq_ignore_ascii_case("full")) {
        let total = checks.len();
        let success = checks
            .iter()
            .filter(|check| check.latency_ms.is_some())
            .count();
        let packet_loss = if total > 0 {
            (total - success) as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let average = {
            let latencies: Vec<f64> = checks.iter().filter_map(|check| check.latency_ms).collect();
            if latencies.is_empty() {
                None
            } else {
                Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
            }
        };
        println!();
        if let Some(avg) = average {
            println!("{} Average latency: {avg:.1} ms", success_prefix());
        } else {
            println!("{} Average latency: unavailable", error_prefix());
        }
        println!(
            "{} Packet loss: {:.1}%",
            if packet_loss < 50.0 {
                success_prefix()
            } else {
                warn_prefix()
            },
            packet_loss
        );
    }

    Ok(())
}

pub fn show_network_info() -> Result<(), String> {
    let config = Config::load_or_create().unwrap_or_default();
    let info = read_cache("network", config.cache.network_ttl_secs).unwrap_or_else(|| {
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
    let cached: Option<Vec<(String, String)>> = read_cache(&key, 1);
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
    format_box_table(title, rows)
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
    run_diagnostic_network(false)?;
    println!();
    println!("System");
    run_diagnostic_system(false)?;
    println!();
    println!("Plugins");
    run_diagnostic_plugins()?;
    Ok(())
}

pub fn run_diagnostic_full(config: &Config, server_mode: bool) -> Result<(), String> {
    let checks = collect_full_diagnostic_checks(config, server_mode);
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    println!("Running full diagnostic. This may take longer.");
    for check in checks {
        let prefix = match check.status.as_str() {
            "PASS" => success_prefix(),
            "WARN" => warn_prefix(),
            _ => error_prefix(),
        };
        println!(
            "{} {} [{}] ({})",
            prefix, check.name, check.severity, check.detail
        );
        if check.fix != "none" {
            println!("FIX: {}", check.fix);
        }
    }
    if server_mode {
        print_server_mode_footer();
    }
    Ok(())
}

pub fn write_diagnostic_markdown(path: &Path, config: &Config, full: bool) -> Result<(), String> {
    let checks = if full {
        collect_full_diagnostic_checks(config, config.server_mode)
    } else {
        collect_diagnostic_checks()
    };
    let title = if full {
        "Terminal Info Full Diagnostic"
    } else {
        "Terminal Info Diagnostic"
    };
    let markdown = render_diagnostic_markdown(title, &checks, full, config.server_mode);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create diagnostic export directory {}: {err}",
                    parent.display()
                )
            })?;
        }
    }

    fs::write(path, markdown)
        .map_err(|err| format!("Failed to write diagnostic markdown {}: {err}", path.display()))?;
    println!("Diagnostic markdown written to {}", path.display());
    Ok(())
}

pub fn run_config_doctor(config: &Config) -> Result<(), String> {
    let migration = inspect_migration_status()?;
    let checks = vec![
        doctor_status("Config file", "PASS", "info", "loaded", "none"),
        doctor_status(
            "Profiles",
            if config.profile.is_empty() {
                "WARN"
            } else {
                "PASS"
            },
            if config.profile.is_empty() {
                "warning"
            } else {
                "info"
            },
            if config.profile.is_empty() {
                "none configured"
            } else {
                "ok"
            },
            if config.profile.is_empty() {
                "add profiles under [profile.<name>] if needed"
            } else {
                "none"
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
                "info"
            } else {
                "warning"
            },
            if plugin_dir_path()?.exists() {
                "present"
            } else {
                "missing"
            },
            "run a plugin install to create it automatically",
        ),
        doctor_status(
            "Registry cache",
            if registry_cache_path().exists() {
                "PASS"
            } else {
                "WARN"
            },
            if registry_cache_path().exists() {
                "info"
            } else {
                "warning"
            },
            if registry_cache_path().exists() {
                "present"
            } else {
                "missing"
            },
            "run `tinfo plugin search` to refresh the registry cache",
        ),
        doctor_status(
            "Weather API",
            if config.provider.is_some() && config.api_key.is_none() {
                "FAIL"
            } else {
                "PASS"
            },
            if config.provider.is_some() && config.api_key.is_none() {
                "error"
            } else {
                "info"
            },
            if config.provider.is_some() && config.api_key.is_none() {
                "provider set without API key"
            } else {
                "ok"
            },
            "set an API key with `tinfo config api set openweather <key>` or clear the provider",
        ),
        doctor_status(
            "Network connectivity",
            if http_reachable("https://github.com") {
                "PASS"
            } else {
                "WARN"
            },
            if http_reachable("https://github.com") {
                "info"
            } else {
                "warning"
            },
            if http_reachable("https://github.com") {
                "reachable"
            } else {
                "offline"
            },
            "check your network connection or rely on cached data",
        ),
        doctor_status(
            "Plugin integrity",
            "PASS",
            "info",
            "use `tinfo plugin verify` for details",
            "run `tinfo plugin verify` or `tinfo plugin doctor`",
        ),
        doctor_status(
            "Migration status",
            if migration.status == "up-to-date" {
                "PASS"
            } else {
                "WARN"
            },
            if migration.status == "up-to-date" {
                "info"
            } else {
                "warning"
            },
            &migration.status,
            if migration.status == "up-to-date" {
                "none"
            } else {
                "restart tinfo to run startup migration automatically"
            },
        ),
        doctor_status(
            "Server mode",
            if config.server_mode { "PASS" } else { "WARN" },
            if config.server_mode {
                "info"
            } else {
                "warning"
            },
            if config.server_mode {
                "enabled"
            } else {
                "disabled"
            },
            if config.server_mode {
                "none"
            } else {
                "enable it with `tinfo config server enable` for server diagnostics"
            },
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
        let prefix = match check.status.as_str() {
            "PASS" => success_prefix(),
            "WARN" => warn_prefix(),
            _ => error_prefix(),
        };
        println!(
            "{} {:<18} [{}] ({})",
            prefix,
            format!("{} ........", check.name),
            check.severity,
            check.detail
        );
        if check.fix != "none" {
            println!("  FIX: {}", check.fix);
        }
    }
    let _ = verify_plugins();
    Ok(())
}

pub fn run_diagnostic_network(server_mode: bool) -> Result<(), String> {
    let dns_ok = ("github.com", 443)
        .to_socket_addrs()
        .map(|mut addrs| addrs.next().is_some())
        .unwrap_or(false);
    let http_ok = http_reachable("http://example.com");
    let google_ping = tcp_ping_latency("google.com").ok();
    let cloudflare_ping = tcp_ping_latency("cloudflare.com").ok();
    let network = gather_network_info();

    if crate::output::json_output() {
        let mut checks = vec![
            doctor_status(
                "DNS resolution",
                if dns_ok { "PASS" } else { "FAIL" },
                if dns_ok { "info" } else { "error" },
                if dns_ok { "ok" } else { "failed" },
                if dns_ok {
                    "none"
                } else {
                    "check your DNS settings or network connection"
                },
            ),
            doctor_status(
                "External ping",
                if google_ping.is_some() { "PASS" } else { "FAIL" },
                if google_ping.is_some() { "info" } else { "error" },
                &google_ping
                    .map(|ms| format!("google.com {ms:.0}ms"))
                    .unwrap_or_else(|| "failed".to_string()),
                if google_ping.is_some() {
                    "none"
                } else {
                    "verify outbound network connectivity"
                },
            ),
            doctor_status(
                "HTTP reachability",
                if http_ok { "PASS" } else { "FAIL" },
                if http_ok { "info" } else { "error" },
                if http_ok { "reachable" } else { "unreachable" },
                if http_ok {
                    "none"
                } else {
                    "verify outbound HTTP access"
                },
            ),
            doctor_status(
                "Cloudflare ping",
                if cloudflare_ping.is_some() {
                    "PASS"
                } else {
                    "FAIL"
                },
                if cloudflare_ping.is_some() {
                    "info"
                } else {
                    "error"
                },
                &cloudflare_ping
                    .map(|ms| format!("{ms:.0}ms"))
                    .unwrap_or_else(|| "failed".to_string()),
                if cloudflare_ping.is_some() {
                    "none"
                } else {
                    "verify external network routing"
                },
            ),
            doctor_status(
                "Public IP",
                if network.public_ip.is_some() { "PASS" } else { "WARN" },
                if network.public_ip.is_some() {
                    "info"
                } else {
                    "warning"
                },
                network.public_ip.as_deref().unwrap_or("unavailable"),
                "none",
            ),
            doctor_status(
                "Local IP",
                if network.local_ip.is_some() { "PASS" } else { "WARN" },
                if network.local_ip.is_some() {
                    "info"
                } else {
                    "warning"
                },
                network.local_ip.as_deref().unwrap_or("unavailable"),
                "none",
            ),
            doctor_status(
                "ISP",
                if network.isp.is_some() { "PASS" } else { "WARN" },
                if network.isp.is_some() {
                    "info"
                } else {
                    "warning"
                },
                network.isp.as_deref().unwrap_or("unavailable"),
                "none",
            ),
            api_reachability_check(),
            proxy_detection_check(),
        ];
        if server_mode {
            for (label, url) in SERVER_API_ENDPOINTS {
                checks.push(endpoint_doctor_check(
                    label,
                    url,
                    "check outbound HTTPS access, DNS, or firewall rules",
                ));
            }
            match dns_server() {
                Ok(server) => checks.push(doctor_status("DNS server", "PASS", "info", &server, "none")),
                Err(_) => checks.push(doctor_status(
                    "DNS server",
                    "WARN",
                    "warning",
                    "unavailable",
                    "check local DNS resolver configuration",
                )),
            }
        }
        print_doctor_checks(&checks)?;
        if server_mode {
            print_server_mode_footer();
        }
        return Ok(());
    }

    let width = 18usize;
    print_network_line(
        "DNS resolution",
        dns_ok,
        if dns_ok {
            Some("✓ ok".to_string())
        } else {
            Some("✗ failed".to_string())
        },
        width,
    );
    print_network_line(
        "External ping",
        google_ping.is_some(),
        Some(
            google_ping
                .map(|ms| format!("✓ ok (google.com {:.0}ms)", ms))
                .unwrap_or_else(|| "✗ failed".to_string()),
        ),
        width,
    );
    print_network_line(
        "HTTP reachability",
        http_ok,
        if http_ok {
            Some("✓ ok".to_string())
        } else {
            Some("✗ failed".to_string())
        },
        width,
    );
    print_network_line(
        "Cloudflare ping",
        cloudflare_ping.is_some(),
        Some(
            cloudflare_ping
                .map(|ms| format!("✓ ok ({:.0}ms)", ms))
                .unwrap_or_else(|| "✗ failed".to_string()),
        ),
        width,
    );
    println!(
        "  {:<width$} {}",
        "Public IP",
        network.public_ip.as_deref().unwrap_or("unavailable"),
        width = width
    );
    println!(
        "  {:<width$} {}",
        "Local IP",
        network.local_ip.as_deref().unwrap_or("unavailable"),
        width = width
    );
    println!(
        "  {:<width$} {}",
        "ISP",
        network.isp.as_deref().unwrap_or("unavailable"),
        width = width
    );

    if dns_ok && google_ping.is_some() && http_ok && cloudflare_ping.is_some() {
        println!("  All network checks passed.");
    } else {
        println!("  Some network checks failed.");
    }

    if server_mode {
        println!();
        for (label, url) in SERVER_API_ENDPOINTS {
            let ok = http_reachable(url);
            println!(
                "  {:<width$} {} {}",
                label,
                if ok { "✓" } else { "✗" },
                if ok { "ok" } else { "failed" },
                width = width
            );
        }
        print_server_mode_footer();
    }

    Ok(())
}

fn print_network_line(name: &str, ok: bool, detail: Option<String>, width: usize) {
    match detail {
        Some(detail) => {
            println!("  {:<width$} {}", name, detail, width = width);
        }
        None => {
            println!(
                "  {:<width$} {}",
                name,
                if ok { "✓ ok" } else { "✗ failed" },
                width = width
            );
        }
    }
}

pub fn run_diagnostic_system(server_mode: bool) -> Result<(), String> {
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

    let mut checks = vec![
        doctor_status(
            "OS version",
            "PASS",
            "info",
            &System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
            "none",
        ),
        doctor_status(
            "Architecture",
            "PASS",
            "info",
            env::consts::ARCH,
            "none",
        ),
        doctor_status(
            "Shell",
            if current_shell().is_empty() { "WARN" } else { "PASS" },
            if current_shell().is_empty() {
                "warning"
            } else {
                "info"
            },
            &if current_shell().is_empty() {
                "unknown".to_string()
            } else {
                current_shell()
            },
            if current_shell().is_empty() {
                "set SHELL or COMSPEC if you rely on shell-specific behavior"
            } else {
                "none"
            },
        ),
        doctor_status(
            "tinfo version",
            "PASS",
            "info",
            env!("CARGO_PKG_VERSION"),
            "none",
        ),
        doctor_status(
            "Disk usage",
            if disk_usage_ratio < 0.9 { "PASS" } else { "WARN" },
            if disk_usage_ratio < 0.9 {
                "info"
            } else {
                "warning"
            },
            &format!("{:.1}%", disk_usage_ratio * 100.0),
            if disk_usage_ratio < 0.9 {
                "none"
            } else {
                "free disk space before usage becomes critical"
            },
        ),
        doctor_status(
            "Memory usage",
            if memory_ratio < 0.9 { "PASS" } else { "WARN" },
            if memory_ratio < 0.9 {
                "info"
            } else {
                "warning"
            },
            &format!("{:.1}%", memory_ratio * 100.0),
            if memory_ratio < 0.9 {
                "none"
            } else {
                "reduce memory pressure or add capacity"
            },
        ),
        doctor_status(
            "CPU load",
            if cpu < 90.0 { "PASS" } else { "WARN" },
            if cpu < 90.0 { "info" } else { "warning" },
            &format!("{cpu:.1}%"),
            if cpu < 90.0 {
                "none"
            } else {
                "investigate sustained CPU-heavy processes"
            },
        ),
        smart_status_check(),
        disk_errors_check(),
    ];
    checks.extend(battery_checks());
    if server_mode {
        let uptime = system_uptime_seconds()
            .map(format_uptime)
            .unwrap_or_else(|| "unavailable".to_string());
        let swap_ratio = if system.total_swap() > 0 {
            system.used_swap() as f64 / system.total_swap() as f64
        } else {
            0.0
        };
        checks.push(doctor_status(
            "System uptime",
            if system_uptime_seconds().is_some() {
                "PASS"
            } else {
                "WARN"
            },
            if system_uptime_seconds().is_some() {
                "info"
            } else {
                "warning"
            },
            &uptime,
            if system_uptime_seconds().is_some() {
                "none"
            } else {
                "verify the host exposes uptime information"
            },
        ));
        checks.push(doctor_status(
            "Swap usage",
            if system.total_swap() == 0 || swap_ratio < 0.9 {
                "PASS"
            } else {
                "WARN"
            },
            if system.total_swap() == 0 || swap_ratio < 0.9 {
                "info"
            } else {
                "warning"
            },
            &format!("{:.1}%", swap_ratio * 100.0),
            if system.total_swap() == 0 || swap_ratio < 0.9 {
                "none"
            } else {
                "high swap usage usually indicates memory pressure"
            },
        ));
        let load = System::load_average();
        checks.push(doctor_status(
            "Load average",
            "PASS",
            "info",
            &format!("{:.2} {:.2} {:.2}", load.one, load.five, load.fifteen),
            "none",
        ));
        checks.push(doctor_status(
            "Process count",
            "PASS",
            "info",
            &system.processes().len().to_string(),
            "none",
        ));
    }
    print_doctor_checks(&checks)?;
    if server_mode {
        print_server_mode_footer();
    }

    Ok(())
}

pub fn run_diagnostic_performance(server_mode: bool) -> Result<(), String> {
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

    let uptime_seconds = system_uptime_seconds();
    let cpu_usage = system.global_cpu_usage();
    let memory_usage = if system.total_memory() > 0 {
        system.used_memory() as f64 / system.total_memory() as f64 * 100.0
    } else {
        0.0
    };
    let swap_usage = if system.total_swap() > 0 {
        system.used_swap() as f64 / system.total_swap() as f64 * 100.0
    } else {
        0.0
    };
    let load = System::load_average();

    let checks = vec![
        doctor_status(
            "CPU load",
            if cpu_usage < 90.0 { "PASS" } else { "WARN" },
            if cpu_usage < 90.0 { "info" } else { "warning" },
            &format!("{cpu_usage:.1}%"),
            "investigate sustained CPU-heavy processes",
        ),
        doctor_status(
            "Memory usage",
            if memory_usage < 90.0 { "PASS" } else { "WARN" },
            if memory_usage < 90.0 {
                "info"
            } else {
                "warning"
            },
            &format!("{memory_usage:.1}%"),
            "reduce memory pressure or add capacity",
        ),
        doctor_status(
            "Disk usage",
            if disk_usage_ratio < 0.9 {
                "PASS"
            } else {
                "WARN"
            },
            if disk_usage_ratio < 0.9 {
                "info"
            } else {
                "warning"
            },
            &format!("{:.1}%", disk_usage_ratio * 100.0),
            "free disk space before usage becomes critical",
        ),
        doctor_status(
            "Swap usage",
            if system.total_swap() == 0 || swap_usage < 90.0 {
                "PASS"
            } else {
                "WARN"
            },
            if system.total_swap() == 0 || swap_usage < 90.0 {
                "info"
            } else {
                "warning"
            },
            &format!("{swap_usage:.1}%"),
            "high swap usage usually indicates memory pressure",
        ),
        doctor_status(
            "System uptime",
            if uptime_seconds.is_some() {
                "PASS"
            } else {
                "WARN"
            },
            if uptime_seconds.is_some() {
                "info"
            } else {
                "warning"
            },
            &uptime_seconds
                .map(format_uptime)
                .unwrap_or_else(|| "unavailable".to_string()),
            "verify the host exposes uptime information",
        ),
        doctor_status(
            "Process pressure",
            "PASS",
            "info",
            &format!("{} processes", system.processes().len()),
            "review long-running services if the host feels overloaded",
        ),
    ];

    let mut checks = checks;
    if server_mode {
        checks.push(doctor_status(
            "Load average",
            if load.one < 8.0 { "PASS" } else { "WARN" },
            if load.one < 8.0 { "info" } else { "warning" },
            &format!("{:.2} {:.2} {:.2}", load.one, load.five, load.fifteen),
            "investigate sustained host load if these values stay high",
        ));
        checks.push(doctor_status(
            "Running processes",
            if system.processes().len() < 1024 {
                "PASS"
            } else {
                "WARN"
            },
            if system.processes().len() < 1024 {
                "info"
            } else {
                "warning"
            },
            &system.processes().len().to_string(),
            "review runaway worker or service counts",
        ));
    }

    print_doctor_checks(&checks)?;
    if server_mode {
        print_server_mode_footer();
    }
    Ok(())
}

pub fn run_diagnostic_security(config: &Config) -> Result<(), String> {
    let checks = collect_security_checks(config);
    print_doctor_checks(&checks)?;
    print_server_mode_footer();
    Ok(())
}

pub fn run_diagnostic_leaks(config: &Config) -> Result<(), String> {
    let checks = collect_leak_checks(config);
    print_doctor_checks(&checks)?;
    print_server_mode_footer();
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
    if let Some(report) = read_cache(&key, config.cache.weather_ttl_secs) {
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

pub(crate) fn memory_line(system: &System) -> String {
    format!(
        "{} / {} used",
        format_bytes(system.used_memory()),
        format_bytes(system.total_memory())
    )
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    if bytes as f64 >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

pub(crate) fn format_uptime(seconds: u64) -> String {
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

fn print_server_mode_footer() {
    println!("[Server Mode Enabled]");
}

fn render_diagnostic_markdown(
    title: &str,
    checks: &[DoctorCheck],
    full: bool,
    server_mode: bool,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("# {title}\n\n"));
    output.push_str(&format!(
        "- Generated: {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S %Z")
    ));
    output.push_str(&format!(
        "- Mode: {}\n",
        if full { "full" } else { "quick" }
    ));
    output.push_str(&format!(
        "- Server mode: {}\n\n",
        if server_mode { "enabled" } else { "disabled" }
    ));
    output.push_str("| Check | Status | Severity | Detail | Fix |\n");
    output.push_str("| --- | --- | --- | --- | --- |\n");
    for check in checks {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            escape_markdown_table(&check.name),
            escape_markdown_table(&check.status),
            escape_markdown_table(&check.severity),
            escape_markdown_table(&check.detail),
            escape_markdown_table(&check.fix),
        ));
    }
    output
}

fn escape_markdown_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', "<br>")
}

fn print_doctor_checks(checks: &[DoctorCheck]) -> Result<(), String> {
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(checks).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    for check in checks {
        let prefix = match check.status.as_str() {
            "PASS" => success_prefix(),
            "WARN" => warn_prefix(),
            _ => error_prefix(),
        };
        println!(
            "{} {} [{}] ({})",
            prefix, check.name, check.severity, check.detail
        );
        if check.fix != "none" {
            println!("FIX: {}", check.fix);
        }
    }
    Ok(())
}

fn doctor_status(name: &str, status: &str, severity: &str, detail: &str, fix: &str) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: status.to_string(),
        severity: severity.to_string(),
        detail: detail.to_string(),
        fix: fix.to_string(),
    }
}

fn collect_latency_checks(mode: Option<&str>, server_mode: bool) -> Vec<LatencyCheck> {
    let targets = latency_targets(mode, server_mode);
    targets
        .into_iter()
        .map(|endpoint| match tcp_ping_latency(&endpoint) {
            Ok(latency_ms) => LatencyCheck {
                endpoint,
                latency_ms: Some(latency_ms),
                status: "PASS".to_string(),
                error: None,
            },
            Err(err) => LatencyCheck {
                endpoint,
                latency_ms: None,
                status: "FAIL".to_string(),
                error: Some(err),
            },
        })
        .collect()
}

fn latency_targets(mode: Option<&str>, server_mode: bool) -> Vec<String> {
    match mode {
        Some("full") => {
            let hosts = if server_mode {
                &SERVER_FULL_PING_HOSTS[..]
            } else {
                &FULL_PING_HOSTS[..]
            };
            hosts.iter().map(|host| (*host).to_string()).collect()
        }
        Some(host) => vec![host.to_string()],
        None => DEFAULT_PING_HOSTS
            .iter()
            .map(|host| (*host).to_string())
            .collect(),
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
    let mut checks = vec![
        doctor_status(
            "DNS resolution",
            if dns_ok { "PASS" } else { "FAIL" },
            if dns_ok { "info" } else { "error" },
            &format!("{dns_ms:.1} ms"),
            if dns_ok {
                "none"
            } else {
                "check your DNS settings or network connection"
            },
        ),
        doctor_status(
            "HTTP reachability",
            if http_ok { "PASS" } else { "FAIL" },
            if http_ok { "info" } else { "error" },
            if http_ok { "reachable" } else { "unreachable" },
            if http_ok {
                "none"
            } else {
                "verify outbound HTTP access"
            },
        ),
        doctor_status(
            "TLS certificate",
            if tls_ok { "PASS" } else { "FAIL" },
            if tls_ok { "info" } else { "error" },
            if tls_ok { "valid" } else { "failed" },
            if tls_ok {
                "none"
            } else {
                "verify system certificates and outbound HTTPS access"
            },
        ),
        doctor_status(
            "OS version",
            "PASS",
            "info",
            &System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
            "none",
        ),
        doctor_status(
            "Architecture",
            "PASS",
            "info",
            env::consts::ARCH,
            "none",
        ),
        doctor_status(
            "Shell",
            if current_shell().is_empty() { "WARN" } else { "PASS" },
            if current_shell().is_empty() {
                "warning"
            } else {
                "info"
            },
            &if current_shell().is_empty() {
                "unknown".to_string()
            } else {
                current_shell()
            },
            if current_shell().is_empty() {
                "set SHELL or COMSPEC if you rely on shell-specific behavior"
            } else {
                "none"
            },
        ),
        doctor_status(
            "tinfo version",
            "PASS",
            "info",
            env!("CARGO_PKG_VERSION"),
            "none",
        ),
        api_reachability_check(),
        proxy_detection_check(),
        config_syntax_check(),
        config_required_fields_check(),
        config_invalid_values_check(),
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
        if disk_status == "PASS" {
            "info"
        } else {
            "warning"
        },
        "basic disk check",
        "inspect disk usage and free space",
    ));
    checks.push(doctor_status(
        "Plugin integrity",
        "PASS",
        "info",
        "use plugin verify",
        "run `tinfo plugin verify` for a full plugin integrity check",
    ));
    checks.push(unknown_plugins_check());
    checks.push(broken_paths_check());
    checks.push(doctor_status(
        "Config integrity",
        "PASS",
        "info",
        "config parsed",
        "none",
    ));
    checks
}

fn collect_full_diagnostic_checks(config: &Config, server_mode: bool) -> Vec<DoctorCheck> {
    let mut checks = collect_diagnostic_checks();
    checks.extend(collect_endpoint_doctor_checks(
        &[
            (
                "Weather API connectivity",
                "https://api.open-meteo.com/v1/forecast",
            ),
            (
                "Plugin registry access",
                "https://raw.githubusercontent.com/T-1234567890/terminal-info/main/plugins/index.json",
            ),
        ],
        "check outbound HTTPS access or rely on cached data",
    ));
    let registry_cache = registry_cache_path();
    let cache_detail = if registry_cache.exists() {
        registry_cache.display().to_string()
    } else {
        "registry cache missing".to_string()
    };
    checks.push(doctor_status(
        "Cache integrity",
        if registry_cache.exists() {
            "PASS"
        } else {
            "WARN"
        },
        if registry_cache.exists() {
            "info"
        } else {
            "warning"
        },
        &cache_detail,
        "run `tinfo plugin search` to refresh the cache",
    ));
    if server_mode {
        checks.extend(collect_endpoint_doctor_checks(
            &SERVER_API_ENDPOINTS,
            "check outbound HTTPS access, DNS, or firewall rules",
        ));
        let load = System::load_average();
        checks.push(doctor_status(
            "Load average",
            if load.one < 8.0 { "PASS" } else { "WARN" },
            if load.one < 8.0 { "info" } else { "warning" },
            &format!("{:.2} {:.2} {:.2}", load.one, load.five, load.fifteen),
            "investigate sustained host load if the one-minute value stays high",
        ));
        checks.push(doctor_status(
            "DNS server availability",
            if dns_server().is_ok() { "PASS" } else { "WARN" },
            if dns_server().is_ok() {
                "info"
            } else {
                "warning"
            },
            &dns_server().unwrap_or_else(|_| "unavailable".to_string()),
            "check local DNS resolver configuration",
        ));
        checks.extend(collect_security_checks(config));
        checks.extend(collect_leak_checks(config));
        for latency in collect_latency_checks(Some("full"), true) {
            checks.push(doctor_status(
                &format!("Latency {}", latency.endpoint),
                &latency.status,
                if latency.status == "PASS" {
                    "info"
                } else {
                    "warning"
                },
                &latency
                    .latency_ms
                    .map(|value| format!("{value:.1} ms"))
                    .unwrap_or_else(|| latency.error.unwrap_or_else(|| "failed".to_string())),
                "check regional connectivity, DNS, or firewall rules",
            ));
        }
    }

    checks
}

fn collect_endpoint_doctor_checks(endpoints: &[(&str, &str)], fix: &str) -> Vec<DoctorCheck> {
    endpoints
        .iter()
        .map(|(label, url)| endpoint_doctor_check(label, url, fix))
        .collect()
}

fn endpoint_doctor_check(label: &str, url: &str, fix: &str) -> DoctorCheck {
    let reachable = http_reachable(url);
    doctor_status(
        label,
        if reachable { "PASS" } else { "WARN" },
        if reachable { "info" } else { "warning" },
        if reachable {
            "reachable"
        } else {
            "unreachable"
        },
        if reachable { "none" } else { fix },
    )
}

fn api_reachability_check() -> DoctorCheck {
    let reachable = NORMAL_API_ENDPOINTS
        .iter()
        .filter(|(_, url)| http_reachable(url))
        .count();
    let total = NORMAL_API_ENDPOINTS.len();
    doctor_status(
        "API reachability",
        if reachable == total {
            "PASS"
        } else if reachable > 0 {
            "WARN"
        } else {
            "FAIL"
        },
        if reachable == total {
            "info"
        } else if reachable > 0 {
            "warning"
        } else {
            "error"
        },
        &format!("{reachable}/{total} core APIs reachable"),
        if reachable == total {
            "none"
        } else {
            "check outbound HTTPS access or rely on cached data"
        },
    )
}

fn proxy_detection_check() -> DoctorCheck {
    let proxies = detected_proxies();
    doctor_status(
        "Proxy detection",
        if proxies.is_empty() { "PASS" } else { "WARN" },
        if proxies.is_empty() {
            "info"
        } else {
            "warning"
        },
        if proxies.is_empty() {
            "no proxy variables detected".to_string()
        } else {
            format!("detected {}", proxies.join(", "))
        }
        .as_str(),
        if proxies.is_empty() {
            "none"
        } else {
            "verify proxy settings if requests fail unexpectedly"
        },
    )
}

fn config_syntax_check() -> DoctorCheck {
    match config_path()
        .map_err(|err| err.to_string())
        .and_then(|path| fs::read_to_string(&path).map_err(|err| format!("{err}")))
    {
        Ok(contents) => {
            let status = toml::from_str::<toml::Value>(&contents).is_ok();
            doctor_status(
                "Config file syntax",
                if status { "PASS" } else { "FAIL" },
                if status { "info" } else { "error" },
                if status { "valid TOML" } else { "invalid TOML" },
                if status {
                    "none"
                } else {
                    "fix TOML syntax with `tinfo config edit`"
                },
            )
        }
        Err(err) => doctor_status(
            "Config file syntax",
            "WARN",
            "warning",
            &err,
            "ensure the config file exists and is readable",
        ),
    }
}

fn config_required_fields_check() -> DoctorCheck {
    match Config::load_or_create() {
        Ok(config) => {
            let mut missing = Vec::new();
            if config.config_version == 0 {
                missing.push("config_version");
            }
            if config.dashboard.widgets.is_empty() {
                missing.push("dashboard.widgets");
            }
            if let Some(active) = &config.active_profile {
                if !config.profile.contains_key(active) {
                    missing.push("active_profile target");
                }
            }
            doctor_status(
                "Missing required fields",
                if missing.is_empty() { "PASS" } else { "WARN" },
                if missing.is_empty() {
                    "info"
                } else {
                    "warning"
                },
                if missing.is_empty() {
                    "none missing".to_string()
                } else {
                    missing.join(", ")
                }
                .as_str(),
                if missing.is_empty() {
                    "none"
                } else {
                    "restore the missing config fields or run `tinfo config reset`"
                },
            )
        }
        Err(err) => doctor_status(
            "Missing required fields",
            "WARN",
            "warning",
            &err,
            "fix the config file first",
        ),
    }
}

fn config_invalid_values_check() -> DoctorCheck {
    match Config::load_or_create() {
        Ok(config) => {
            let mut invalid = Vec::new();
            if config.dashboard.refresh_interval == 0 {
                invalid.push("dashboard.refresh_interval");
            }
            if config.cache.weather_ttl_secs == 0 {
                invalid.push("cache.weather_ttl_secs");
            }
            if config.cache.network_ttl_secs == 0 {
                invalid.push("cache.network_ttl_secs");
            }
            if config.cache.time_ttl_secs == 0 {
                invalid.push("cache.time_ttl_secs");
            }
            for (alias, value) in &config.locations {
                if value.trim().is_empty() {
                    invalid.push(alias);
                }
            }
            doctor_status(
                "Invalid values",
                if invalid.is_empty() { "PASS" } else { "WARN" },
                if invalid.is_empty() {
                    "info"
                } else {
                    "warning"
                },
                if invalid.is_empty() {
                    "none detected".to_string()
                } else {
                    invalid.join(", ")
                }
                .as_str(),
                if invalid.is_empty() {
                    "none"
                } else {
                    "correct the invalid config values with `tinfo config edit`"
                },
            )
        }
        Err(err) => doctor_status(
            "Invalid values",
            "WARN",
            "warning",
            &err,
            "fix the config file first",
        ),
    }
}

fn unknown_plugins_check() -> DoctorCheck {
    match plugin_diagnostic_summary() {
        Ok(summary) => doctor_status(
            "Unknown plugins",
            if summary.unknown_plugins.is_empty() {
                "PASS"
            } else {
                "WARN"
            },
            if summary.unknown_plugins.is_empty() {
                "info"
            } else {
                "warning"
            },
            if summary.unknown_plugins.is_empty() {
                "none".to_string()
            } else {
                summary.unknown_plugins.join(", ")
            }
            .as_str(),
            if summary.unknown_plugins.is_empty() {
                "none"
            } else {
                "remove unknown plugins or add them to the reviewed registry"
            },
        ),
        Err(err) => doctor_status(
            "Unknown plugins",
            "WARN",
            "warning",
            &err,
            "check the plugin directory and registry cache",
        ),
    }
}

fn broken_paths_check() -> DoctorCheck {
    match plugin_diagnostic_summary() {
        Ok(summary) => doctor_status(
            "Broken paths",
            if summary.broken_paths.is_empty() {
                "PASS"
            } else {
                "FAIL"
            },
            if summary.broken_paths.is_empty() {
                "info"
            } else {
                "error"
            },
            if summary.broken_paths.is_empty() {
                "none".to_string()
            } else {
                summary.broken_paths.join(", ")
            }
            .as_str(),
            if summary.broken_paths.is_empty() {
                "none"
            } else {
                "reinstall affected plugins or repair the missing paths"
            },
        ),
        Err(err) => doctor_status(
            "Broken paths",
            "WARN",
            "warning",
            &err,
            "check the plugin directory and config paths",
        ),
    }
}

fn current_shell() -> String {
    env::var("SHELL")
        .or_else(|_| env::var("COMSPEC"))
        .unwrap_or_default()
}

fn detected_proxies() -> Vec<String> {
    [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ]
    .into_iter()
    .filter(|name| env::var(name).ok().filter(|value| !value.is_empty()).is_some())
    .map(str::to_string)
    .collect()
}

fn smart_status_check() -> DoctorCheck {
    match smart_info() {
        Some((status, _errors)) => doctor_status(
            "SMART status",
            if status.eq_ignore_ascii_case("verified") || status.eq_ignore_ascii_case("passed") {
                "PASS"
            } else {
                "WARN"
            },
            if status.eq_ignore_ascii_case("verified") || status.eq_ignore_ascii_case("passed") {
                "info"
            } else {
                "warning"
            },
            &status,
            if status.eq_ignore_ascii_case("verified") || status.eq_ignore_ascii_case("passed") {
                "none"
            } else {
                "inspect disk health with platform disk tools"
            },
        ),
        None => doctor_status(
            "SMART status",
            "WARN",
            "warning",
            "unavailable",
            "install smartmontools or use your platform disk utility",
        ),
    }
}

fn disk_errors_check() -> DoctorCheck {
    match smart_info() {
        Some((_status, Some(errors))) => doctor_status(
            "Disk errors",
            if errors == 0 { "PASS" } else { "WARN" },
            if errors == 0 { "info" } else { "warning" },
            &format!("{errors} reported"),
            if errors == 0 {
                "none"
            } else {
                "check the disk SMART report and back up important data"
            },
        ),
        Some((_status, None)) => doctor_status(
            "Disk errors",
            "WARN",
            "warning",
            "not reported by the platform",
            "use a platform-specific disk utility for a deeper disk scan",
        ),
        None => doctor_status(
            "Disk errors",
            "WARN",
            "warning",
            "unavailable",
            "install smartmontools or use your platform disk utility",
        ),
    }
}

fn smart_info() -> Option<(String, Option<u64>)> {
    if cfg!(target_os = "macos") {
        let output = Command::new("diskutil").args(["info", "/"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let status = text
            .lines()
            .find_map(|line| line.split_once("SMART Status:").map(|(_, value)| value.trim()))
            .map(str::to_string)?;
        return Some((status, None));
    }

    if cfg!(target_os = "linux") {
        let scan = Command::new("smartctl").arg("--scan-open").output().ok()?;
        if !scan.status.success() {
            return None;
        }
        let device = String::from_utf8_lossy(&scan.stdout)
            .lines()
            .next()?
            .split_whitespace()
            .next()?
            .to_string();
        let health = Command::new("smartctl")
            .args(["-H", "-A", &device])
            .output()
            .ok()?;
        if !health.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&health.stdout);
        let status = text
            .lines()
            .find_map(|line| {
                line.split(':')
                    .next_back()
                    .filter(|_| line.contains("SMART overall-health"))
                    .map(str::trim)
            })
            .or_else(|| {
                text.lines().find_map(|line| {
                    line.split(':')
                        .next_back()
                        .filter(|_| line.contains("SMART Health Status"))
                        .map(str::trim)
                })
            })?
            .to_string();
        let errors = text.lines().find_map(parse_disk_error_line);
        return Some((status, errors));
    }

    None
}

fn parse_disk_error_line(line: &str) -> Option<u64> {
    if !(line.contains("Reported_Uncorrect")
        || line.contains("Media and Data Integrity Errors")
        || line.contains("Error Information Log Entries"))
    {
        return None;
    }
    line.split_whitespace().last()?.parse().ok()
}

fn battery_checks() -> Vec<DoctorCheck> {
    match battery_info() {
        Some((health, cycle_count)) => vec![
            doctor_status(
                "Battery health",
                if health.eq_ignore_ascii_case("normal")
                    || health.eq_ignore_ascii_case("good")
                    || health.eq_ignore_ascii_case("ok")
                {
                    "PASS"
                } else {
                    "WARN"
                },
                if health.eq_ignore_ascii_case("normal")
                    || health.eq_ignore_ascii_case("good")
                    || health.eq_ignore_ascii_case("ok")
                {
                    "info"
                } else {
                    "warning"
                },
                &health,
                if health.eq_ignore_ascii_case("normal")
                    || health.eq_ignore_ascii_case("good")
                    || health.eq_ignore_ascii_case("ok")
                {
                    "none"
                } else {
                    "check battery service health if this is a laptop"
                },
            ),
            doctor_status(
                "Cycle count",
                "PASS",
                "info",
                &cycle_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "unavailable".to_string()),
                "none",
            ),
        ],
        None => vec![
            doctor_status(
                "Battery health",
                "WARN",
                "warning",
                "unavailable",
                "battery health is only available on supported laptop hardware",
            ),
            doctor_status(
                "Cycle count",
                "WARN",
                "warning",
                "unavailable",
                "cycle count is only available on supported laptop hardware",
            ),
        ],
    }
}

fn battery_info() -> Option<(String, Option<u64>)> {
    if cfg!(target_os = "macos") {
        let output = Command::new("system_profiler")
            .arg("SPPowerDataType")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let health = text
            .lines()
            .find_map(|line| line.split_once("Condition:").map(|(_, value)| value.trim()))
            .map(str::to_string)?;
        let cycle_count = text.lines().find_map(|line| {
            line.split_once("Cycle Count:")
                .and_then(|(_, value)| value.trim().parse::<u64>().ok())
        });
        return Some((health, cycle_count));
    }

    if cfg!(target_os = "linux") {
        let power_supply = fs::read_dir("/sys/class/power_supply").ok()?;
        for entry in power_supply.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("BAT") {
                continue;
            }
            let health = fs::read_to_string(path.join("health"))
                .ok()
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let cycle_count = fs::read_to_string(path.join("cycle_count"))
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok());
            return Some((health, cycle_count));
        }
    }

    None
}

fn collect_security_checks(config: &Config) -> Vec<DoctorCheck> {
    let suspicious_env = [
        "AWS_SECRET_ACCESS_KEY",
        "GITHUB_TOKEN",
        "OPENWEATHER_API_KEY",
        "DATABASE_URL",
    ]
    .into_iter()
    .filter(|name| {
        env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .is_some()
    })
    .collect::<Vec<_>>();

    vec![
        doctor_status(
            "Config secrets",
            if config.effective_api_key().is_some() {
                "WARN"
            } else {
                "PASS"
            },
            if config.effective_api_key().is_some() {
                "warning"
            } else {
                "info"
            },
            if config.effective_api_key().is_some() {
                "API key stored in config"
            } else {
                "no API key stored in config"
            },
            if config.effective_api_key().is_some() {
                "prefer environment variables or a dedicated secret store on servers"
            } else {
                "none"
            },
        ),
        doctor_status(
            "Environment secrets",
            if suspicious_env.is_empty() {
                "PASS"
            } else {
                "WARN"
            },
            if suspicious_env.is_empty() {
                "info"
            } else {
                "warning"
            },
            if suspicious_env.is_empty() {
                "no common secrets detected"
            } else {
                "common secret variables are present"
            },
            if suspicious_env.is_empty() {
                "none"
            } else {
                "audit environment variable scope for server services"
            },
        ),
        doctor_status(
            "Config file path",
            "PASS",
            "info",
            &config_path()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unavailable".to_string()),
            "verify the config file is only readable by the intended user",
        ),
    ]
}

fn collect_leak_checks(config: &Config) -> Vec<DoctorCheck> {
    let env_hits = [
        "OPENWEATHER_API_KEY",
        "AWS_SECRET_ACCESS_KEY",
        "GITHUB_TOKEN",
        "DATABASE_URL",
    ]
    .into_iter()
    .filter(|name| {
        env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .is_some()
    })
    .collect::<Vec<_>>();

    vec![
        doctor_status(
            "Config file secrets",
            if config.effective_api_key().is_some() {
                "WARN"
            } else {
                "PASS"
            },
            if config.effective_api_key().is_some() {
                "warning"
            } else {
                "info"
            },
            &config_path()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unavailable".to_string()),
            if config.effective_api_key().is_some() {
                "remove plaintext secrets from config if the host is shared"
            } else {
                "none"
            },
        ),
        doctor_status(
            "Environment variable leaks",
            if env_hits.is_empty() { "PASS" } else { "WARN" },
            if env_hits.is_empty() {
                "info"
            } else {
                "warning"
            },
            if env_hits.is_empty() {
                "none detected"
            } else {
                "sensitive environment variables detected"
            },
            if env_hits.is_empty() {
                "none"
            } else {
                "limit environment variables to the process that needs them"
            },
        ),
    ]
}

fn plugin_dir_path() -> Result<PathBuf, String> {
    Ok(home_dir_path().join(".terminal-info").join("plugins"))
}

fn registry_cache_path() -> PathBuf {
    home_dir_path()
        .join(".terminal-info")
        .join("cache")
        .join("plugins.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_latency_targets_use_default_endpoints() {
        assert_eq!(latency_targets(None, false).len(), 3);
        assert!(latency_targets(None, false).contains(&"google.com".to_string()));
    }

    #[test]
    fn full_latency_targets_include_additional_endpoints() {
        let targets = latency_targets(Some("full"), false);
        assert!(targets.len() > 3);
        assert!(targets.contains(&"1.1.1.1".to_string()));
        assert!(targets.contains(&"apple.com".to_string()));
    }

    #[test]
    fn server_full_latency_targets_are_broader() {
        let normal = latency_targets(Some("full"), false);
        let server = latency_targets(Some("full"), true);
        assert!(server.len() > normal.len());
        assert!(server.contains(&"quad9.net".to_string()));
        assert!(server.contains(&"akamai.com".to_string()));
        assert!(server.contains(&"azure.com".to_string()));
    }

    #[test]
    fn full_diagnostic_does_not_include_latency_checks() {
        let checks = collect_full_diagnostic_checks(&Config::default(), false);
        assert!(
            !checks
                .iter()
                .any(|check| check.name.starts_with("Latency "))
        );
    }

    #[test]
    fn server_full_diagnostic_includes_latency_checks() {
        let mut config = Config::default();
        config.server_mode = true;
        let checks = collect_full_diagnostic_checks(&config, true);
        assert!(
            checks
                .iter()
                .any(|check| check.name.starts_with("Latency "))
        );
        assert!(checks.iter().any(|check| check.name == "Load average"));
        assert!(
            checks
                .iter()
                .any(|check| check.name == "DNS server availability")
        );
        assert!(checks.iter().any(|check| check.name == "GitHub API"));
    }

    #[test]
    fn diagnostic_markdown_contains_table_and_title() {
        let checks = vec![doctor_status(
            "DNS resolution",
            "PASS",
            "info",
            "1.0 ms",
            "none",
        )];
        let rendered =
            render_diagnostic_markdown("Terminal Info Diagnostic", &checks, false, false);
        assert!(rendered.contains("# Terminal Info Diagnostic"));
        assert!(rendered.contains("| Check | Status | Severity | Detail | Fix |"));
        assert!(rendered.contains("| DNS resolution | PASS | info | 1.0 ms | none |"));
    }
}
