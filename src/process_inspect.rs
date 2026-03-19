use clap::ValueEnum;
use serde::Serialize;
use sysinfo::System;

use crate::builtins::{format_bytes, format_uptime};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ProcessSort {
    Cpu,
    Memory,
}

#[derive(Serialize)]
struct ProcessListView {
    sort: String,
    limit: usize,
    processes: Vec<ProcessView>,
}

#[derive(Serialize)]
struct ProcessView {
    pid: String,
    name: String,
    status: String,
    cpu_percent: f32,
    memory_bytes: u64,
    virtual_memory_bytes: u64,
    runtime_secs: u64,
}

pub fn show_processes(limit: usize, sort: ProcessSort) -> Result<(), String> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut processes = system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessView {
            pid: pid.to_string(),
            name: process.name().to_string_lossy().to_string(),
            status: format!("{:?}", process.status()),
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
            virtual_memory_bytes: process.virtual_memory(),
            runtime_secs: process.run_time(),
        })
        .collect::<Vec<_>>();

    match sort {
        ProcessSort::Cpu => {
            processes.sort_by(|left, right| right.cpu_percent.total_cmp(&left.cpu_percent))
        }
        ProcessSort::Memory => {
            processes.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes))
        }
    }
    processes.truncate(limit.max(1));

    let view = ProcessListView {
        sort: match sort {
            ProcessSort::Cpu => "cpu".to_string(),
            ProcessSort::Memory => "memory".to_string(),
        },
        limit,
        processes,
    };

    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    println!("Processes (sorted by {})", view.sort);
    if view.processes.is_empty() {
        println!("No process data available.");
        return Ok(());
    }
    println!(
        "{:<8} {:<24} {:>7} {:>10} {:>9} Status",
        "PID", "Name", "CPU%", "Memory", "Runtime"
    );
    for process in view.processes {
        println!(
            "{:<8} {:<24} {:>6.1} {:>10} {:>9} {}",
            process.pid,
            truncate_name(&process.name, 24),
            process.cpu_percent,
            format_bytes(process.memory_bytes),
            format_uptime(process.runtime_secs),
            process.status
        );
    }

    Ok(())
}

fn truncate_name(value: &str, width: usize) -> String {
    let mut truncated = value.chars().take(width).collect::<String>();
    if value.chars().count() > width {
        truncated.pop();
        truncated.push('~');
    }
    truncated
}
