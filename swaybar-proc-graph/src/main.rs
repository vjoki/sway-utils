use std::{fs, str, time, thread, fmt};
use std::io::{self, Write, BufRead, Seek};
use anyhow::Result;
use argh::FromArgs;
#[cfg(feature = "nvidia")]
use nvml_wrapper::Nvml;

struct Measurement {
    free: i64,
    total: i64,
}

struct ProcReader {
    reader: io::BufReader<fs::File>,
    buf: String,
    curr: Measurement,
    prev: Measurement,
}

impl ProcReader {
    const SEEK_TO_START: io::SeekFrom = io::SeekFrom::Start(0);

    pub fn new(f: fs::File) -> Self {
        Self {
            reader: io::BufReader::with_capacity(8192, f),
            buf: String::with_capacity(8192),
            curr: Measurement { free: 0, total: 0 },
            prev: Measurement { free: 0, total: 0 },
        }
    }

    pub fn curr(&self) -> &Measurement {
        &self.curr
    }

    pub fn prev(&self) -> &Measurement {
        &self.prev
    }

    pub fn store_curr_to_prev(&mut self) {
        self.prev.free = self.curr.free;
        self.prev.total = self.curr.total;
    }

    pub fn read_cpu_time_to_prev(&mut self) -> Result<()> {
        ProcReader::parse_proc_stat(&mut self.reader, &mut self.buf, &mut self.prev)
    }

    pub fn read_cpu_time_to_curr(&mut self) -> Result<()> {
        ProcReader::parse_proc_stat(&mut self.reader, &mut self.buf, &mut self.curr)
    }

    fn parse_proc_stat(reader: &mut io::BufReader<fs::File>, buf: &mut String, ct: &mut Measurement) -> Result<()> {
        reader.seek(ProcReader::SEEK_TO_START)?;
        buf.clear();
        ct.free = 0;
        ct.total = 0;

        loop {
            let bytes_read = reader.read_line(buf)?;

            // TODO: Figure out how to stop before intr, so that we can allocate a fixed amount of bytes for buf.
            //       Maybe get cpu count and just read <n_cpus> of lines?
            //       (seems to require libc (+ optionally num_cpus crate))
            if bytes_read == 0 || !buf.starts_with("cpu") {
                break;
            }

            for (i, val) in buf.split_whitespace().skip(1).enumerate() {
                let val = val.parse::<i64>()?;
                ct.total += val;

                // 4th element is the idle time.
                if i == 3 {
                    ct.free += val;
                }
            }
            buf.clear();
        }

        Ok(())
    }

    pub fn read_mem_to_curr(&mut self) -> Result<()> {
        ProcReader::parse_proc_meminfo(&mut self.reader, &mut self.buf, &mut self.curr)
    }

    fn parse_proc_meminfo(reader: &mut io::BufReader<fs::File>, buf: &mut String, ct: &mut Measurement) -> Result<()> {
        reader.seek(ProcReader::SEEK_TO_START)?;
        buf.clear();
        ct.free = 0;
        ct.total = 0;

        loop {
            let bytes_read = reader.read_line(buf)?;
            if bytes_read == 0 {
                break;
            }

            if buf.starts_with("MemTotal") {
                let val = buf.split_whitespace().nth(1).expect("value").parse::<i64>()?;
                ct.total += val;
            } else if buf.starts_with("MemAvailable") {
                let val = buf.split_whitespace().nth(1).expect("value").parse::<i64>()?;
                ct.free += val;

                // Assume MemAvailable comes after MemTotal...
                break;
            }
            buf.clear();
        }

        Ok(())
    }
}

mod graph;
use crate::graph::BrailleGraph;

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
            let nvml = Nvml::init()?;
            let device = nvml.device_by_index(subargs.gpu_index)?;

            loop {
                let pct = device.utilization_rates()
                    .map(|util| util.gpu as f64)?;
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
            let nvml = Nvml::init()?;
            let device = nvml.device_by_index(subargs.gpu_index)?;

            loop {
                let curr = device.memory_info()?;
                let pct = 100.0 * (curr.used as f64 / curr.total as f64);
                graph.update(pct as i64);

                write!(
                    stdout_handle,
                    "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"GPU VRAM usage ",
                    pct, graph, pad=graph_len
                )?;
                // NVML MemoryInfo values are in bytes.
                if curr.total as f64 / (1024_i32.pow(2) as f64) < 1024.0 {
                    let div = 1024_i32.pow(2) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} MiB", curr.used as f64 / div, curr.total as f64 / div)
                } else if curr.total as f64 / (1024_i32.pow(3) as f64) < 1024.0 {
                    let div = 1024_i32.pow(3) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} GiB", curr.used as f64 / div, curr.total as f64 / div)
                } else {
                    let div = 1024_i64.pow(4) as f64;
                    write!(stdout_handle, "{:.1}/{:.1} TiB", curr.used as f64 / div, curr.total as f64 / div)
                }?;
                writeln!(stdout_handle, " ({:.2}%)\"}}", pct)?;

                thread::sleep(interval);
            }
        },
        GraphType::Memory(_) => {
            let f = fs::File::open("/proc/meminfo")?;
            let mut reader = ProcReader::new(f);

            loop {
                reader.read_mem_to_curr()?;

                let curr = reader.curr();
                let pct = 100.0 * ((curr.total as f64 - curr.free as f64) / curr.total as f64);

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
            let mut reader = ProcReader::new(f);

            reader.read_cpu_time_to_prev()?;
            thread::sleep(time::Duration::from_millis(100));

            loop {
                reader.read_cpu_time_to_curr()?;

                let curr = reader.curr();
                let prev = reader.prev();
                let di = curr.free - prev.free;
                let dt = curr.total - prev.total;
                let pct = 100.0 * (1.0 - di as f64 / dt as f64);

                graph.update(pct as i64);

                writeln!(
                    stdout_handle, "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"CPU usage {:.2}%\"}}",
                    pct, graph, pct, pad=graph_len
                )?;

                reader.store_curr_to_prev();
                thread::sleep(interval);
            }
        }
    }
}
