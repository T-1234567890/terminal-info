use std::fs;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{FixedOffset, Local, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;
use sysinfo::{Disks, System};

use crate::config::Config;
use crate::output::{error_prefix, success_prefix};
use crate::plugin::run_diagnostic_plugins;
use crate::weather::WeatherClient;

const DEFAULT_PING_HOSTS: [&str; 3] = ["google.com", "cloudflare.com", "github.com"];

pub struct DashboardSnapshot {
    pub time: String,
    pub weather: Option<String>,
    pub network: String,
    pub cpu: String,
    pub memory: String,
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
    let info = gather_network_info();

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

    println!(
        "OS: {}",
        System::long_os_version().unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "CPU: {}",
        system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("Memory: {}", memory_line(&system));
    println!("Disk usage: {disk_line}");
    println!(
        "Uptime: {}",
        system_uptime_seconds()
            .map(format_uptime)
            .unwrap_or_else(|| "unavailable".to_string())
    );

    Ok(())
}

pub fn show_time(city: Option<String>) -> Result<(), String> {
    match city {
        Some(city) => {
            let (label, formatted) = city_time(&city).ok_or_else(|| {
                format!("Unsupported city '{city}'. Try Tokyo, London, New York, or Local.")
            })?;
            println!("{label}: {formatted}");
        }
        None => {
            for city in ["Local", "Tokyo", "London", "New York"] {
                let (label, formatted) =
                    city_time(city).ok_or_else(|| format!("Failed to build time for {city}."))?;
                println!("{label}: {formatted}");
            }
        }
    }

    Ok(())
}

pub fn run_diagnostic_all() -> Result<(), String> {
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

#[derive(Default)]
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

fn dashboard_weather(config: &Config) -> Option<String> {
    let city = config.configured_location()?;
    let client = WeatherClient::new();
    let report = client.current_weather(city, config).ok()?;

    Some(format!(
        "{}, {:.1}{}",
        report.summary,
        report.temperature,
        config.units.temperature_symbol()
    ))
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
