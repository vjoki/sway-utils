use std::{fs, str, time, thread};
use std::io::{self, Write};
use anyhow::Result;
use argh::FromArgs;

mod sources;
mod graph;
use crate::graph::BrailleGraph;
use crate::sources::*;

#[derive(FromArgs)]
#[argh(subcommand)]
enum GraphType {
    Cpu(SubCommandCpu),
    Memory(SubCommandMemory),
    #[cfg(feature = "nvidia")]
    NvGpu(SubCommandNvGpu),
    #[cfg(feature = "nvidia")]
    NvVram(SubCommandNvVram),
}

/// CPU usage graph
#[derive(FromArgs)]
#[argh(subcommand, name = "cpu")]
struct SubCommandCpu {}

/// Memory usage graph
#[derive(FromArgs)]
#[argh(subcommand, name = "memory")]
struct SubCommandMemory {}

/// Nvidia GPU usage graph
#[cfg(feature = "nvidia")]
#[derive(FromArgs)]
#[argh(subcommand, name = "nvgpu")]
struct SubCommandNvGpu {
    /// select GPU by index (starts from 0)
    #[argh(option, default = "0")]
    gpu_index: u32,
}

/// Nvidia GPU VRAM usage graph
#[cfg(feature = "nvidia")]
#[derive(FromArgs)]
#[argh(subcommand, name = "nvvram")]
struct SubCommandNvVram {
    /// select GPU by index (starts from 0)
    #[argh(option, default = "0")]
    gpu_index: u32,
}

fn dur_from_str_secs(s: &str) -> Result<time::Duration, String> {
    s.parse()
        .map(time::Duration::from_secs)
        .map_err(|_| "value not a valid integer".to_owned())
}

#[derive(FromArgs)]
/// Print out CPU, memory, or Nvidia GPU usage graph in Waybar compatible JSON format.
struct Args {
    /// graph length in characters
    #[argh(option, default = "10")]
    len: usize,
    /// update interval in seconds
    #[argh(option, short = 'i', default = "time::Duration::from_secs(1)", from_str_fn(dur_from_str_secs))]
    interval: time::Duration,
    /// graph type
    #[argh(subcommand)]
    graph_type: GraphType,
}

fn main() -> Result<()> {
    let Args { graph_type, interval, len: graph_len } = argh::from_env();

    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();
    let mut graph = BrailleGraph::new(graph_len);

    match graph_type {
        #[cfg(feature = "nvidia")]
        GraphType::NvGpu(subargs) => {
            let mut stat = NvmlGpu::new(subargs.gpu_index)?;

            loop {
                let pct = stat.measure()?;
                graph.update(pct as i64);
                writeln!(
                    stdout_handle,
                    "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"GPU usage {:.2}%\"}}",
                    pct, graph, pct, pad=graph_len
                )?;
                thread::sleep(interval);
            }
        },
        #[cfg(feature = "nvidia")]
        GraphType::NvVram(subargs) => {
            let mut stat = NvmlVram::new(subargs.gpu_index)?;

            loop {
                let pct = stat.measure()?;
                let curr = stat.measurement();
                graph.update(pct as i64);

                write!(
                    stdout_handle,
                    "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"GPU VRAM usage ",
                    pct, graph, pad=graph_len
                )?;
                // NVML MemoryInfo values are in bytes.
                if curr.total as f64 / (1024_i32.pow(2) as f64) < 1024.0 {
                    let div = 1024_i32.pow(2) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} MiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                } else if curr.total as f64 / (1024_i32.pow(3) as f64) < 1024.0 {
                    let div = 1024_i32.pow(3) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} GiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                } else {
                    let div = 1024_i64.pow(4) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} TiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                }?;
                writeln!(stdout_handle, " ({:.2}%)\"}}", pct)?;

                thread::sleep(interval);
            }
        },
        GraphType::Memory(_) => {
            let f = fs::File::open("/proc/meminfo")?;
            let mut stat = ProcMeminfo::new(f);

            loop {
                let pct = stat.measure()?;
                let curr = stat.measurement();
                graph.update(pct as i64);

                write!(
                    stdout_handle,
                    "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"Memory usage ",
                    pct, graph, pad=graph_len
                )?;
                // /proc/meminfo values are in KiBs.
                if curr.total as f64 / 1024_f64 < 1024.0 {
                    let div = 1024_f64;
                    write!(stdout_handle, "{:.1}/{:.1} MiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                } else if curr.total as f64 / (1024_i32.pow(2) as f64) < 1024.0 {
                    let div = 1024_i32.pow(2) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} GiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                } else {
                    let div = 1024_i32.pow(3) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} TiB", (curr.total - curr.free) as f64 / div, curr.total as f64 / div)
                }?;
                writeln!(stdout_handle, " ({:.2}%)\"}}", pct)?;

                thread::sleep(interval);
            }
        },
        GraphType::Cpu(_) => {
            let f = fs::File::open("/proc/stat")?;
            let mut stat = ProcStat::new(f);

            loop {
                let pct = stat.measure()?;
                graph.update(pct as i64);

                writeln!(
                    stdout_handle, "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"CPU usage {:.2}%\"}}",
                    pct, graph, pct, pad=graph_len
                )?;

                thread::sleep(interval);
            }
        }
    }
}
