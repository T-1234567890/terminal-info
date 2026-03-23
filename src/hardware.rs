use serde::Serialize;
use sysinfo::{Disks, System};

use crate::builtins::format_bytes;

#[derive(Serialize)]
struct HardwareView {
    hostname: String,
    os: String,
    kernel_version: String,
    architecture: String,
    cpu: CpuView,
    memory: MemoryView,
    disks: Vec<DiskView>,
}

#[derive(Serialize)]
struct CpuView {
    model: String,
    vendor: String,
    architecture: String,
    logical_cores: usize,
    physical_cores: Option<usize>,
    frequency_mhz: u64,
}

#[derive(Serialize)]
struct MemoryView {
    total_bytes: u64,
    swap_total_bytes: u64,
}

#[derive(Serialize)]
struct DiskView {
    name: String,
    mount_point: String,
    kind: String,
    removable: bool,
    total_bytes: u64,
}

pub fn show_system_hardware() -> Result<(), String> {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();

    let cpu = system.cpus().first();
    let view = HardwareView {
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
        cpu: CpuView {
            model: cpu
                .map(|value| value.brand().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            vendor: cpu
                .map(|value| value.vendor_id().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            architecture: std::env::consts::ARCH.to_string(),
            logical_cores: system.cpus().len(),
            physical_cores: system.physical_core_count(),
            frequency_mhz: cpu.map(|value| value.frequency()).unwrap_or_default(),
        },
        memory: MemoryView {
            total_bytes: system.total_memory(),
            swap_total_bytes: system.total_swap(),
        },
        disks: disks
            .list()
            .iter()
            .map(|disk| DiskView {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().display().to_string(),
                kind: format!("{:?}", disk.kind()),
                removable: disk.is_removable(),
                total_bytes: disk.total_space(),
            })
            .collect(),
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("Host: {}", view.hostname);
    println!("OS: {}", view.os);
    println!("Kernel: {}", view.kernel_version);
    println!("Architecture: {}", view.architecture);
    println!("CPU: {}", view.cpu.model);
    println!("CPU vendor: {}", view.cpu.vendor);
    println!("Logical cores: {}", view.cpu.logical_cores);
    println!(
        "Physical cores: {}",
        view.cpu
            .physical_cores
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    );
    println!("CPU frequency: {} MHz", view.cpu.frequency_mhz);
    println!("Memory: {}", format_bytes(view.memory.total_bytes));
    println!("Swap: {}", format_bytes(view.memory.swap_total_bytes));

    if view.disks.is_empty() {
        println!("Disks: unavailable");
    } else {
        println!("Disks:");
        for disk in view.disks {
            println!(
                "- {} [{}] {} at {}{}",
                disk.name,
                disk.kind,
                format_bytes(disk.total_bytes),
                disk.mount_point,
                if disk.removable { " (removable)" } else { "" }
            );
        }
    }

    Ok(())
}
