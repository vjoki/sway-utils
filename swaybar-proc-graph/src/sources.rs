use std::{fs, time, thread};
use std::io::{self, BufRead, Seek};
use anyhow::Result;

#[derive(Default, Copy, Clone)]
pub struct Measurement {
    pub free: u64,
    pub total: u64,
}

pub trait StatTaker {
    fn measurement(&self) -> Measurement;
    fn measure(&mut self) -> Result<f64>;
}

#[cfg(feature = "nvidia")]
pub use self::nvml::*;
#[cfg(feature = "nvidia")]
mod nvml {
    use anyhow::Result;
    use super::{StatTaker, Measurement};
    use {
        nvml_wrapper::Nvml,
        once_cell::sync::Lazy,
    };

    static NVML: Lazy<Nvml> = Lazy::new(|| Nvml::init().expect("NVML init failed"));

    pub struct NvmlGpu {
        device: nvml_wrapper::Device<'static>,
        pct: u32,
    }

    impl NvmlGpu {
        pub fn new(gpu_index: u32) -> Result<Self> {
            let device = NVML.device_by_index(gpu_index)?;
            Ok(Self { device, pct: 0 })
        }
    }

    impl StatTaker for NvmlGpu {
        fn measurement(&self) -> Measurement {
            Measurement { free: 100_u32.saturating_sub(self.pct) as u64, total: 100 }
        }

        fn measure(&mut self) -> Result<f64> {
            self.pct = self.device.utilization_rates().map(|util| util.gpu)?;
            Ok(self.pct as f64)
        }
    }

    pub struct NvmlVram {
        device: nvml_wrapper::Device<'static>,
        curr: Measurement,
    }

    impl NvmlVram {
        pub fn new(gpu_index: u32) -> Result<Self> {
            let device = NVML.device_by_index(gpu_index)?;
            Ok(Self { device, curr: Measurement::default() })
        }
    }

    impl StatTaker for NvmlVram {
        fn measurement(&self) -> Measurement {
            self.curr
        }

        fn measure(&mut self) -> Result<f64> {
            let mem = self.device.memory_info()?;
            self.curr = Measurement { free: mem.free, total: mem.total };
            let pct = 100.0 * (mem.used as f64 / mem.total as f64);
            Ok(pct)
        }
    }
}

pub struct ProcMeminfo {
    reader: io::BufReader<fs::File>,
    buf: String,
    curr: Measurement,
}

impl ProcMeminfo {
    const SEEK_TO_START: io::SeekFrom = io::SeekFrom::Start(0);

    pub fn new(f: fs::File) -> Self {
        Self {
            reader: io::BufReader::with_capacity(8192, f),
            buf: String::with_capacity(8192),
            curr: Measurement::default(),
        }
    }

    fn parse_proc_meminfo(reader: &mut io::BufReader<fs::File>, buf: &mut String) -> Result<Measurement> {
        reader.seek(Self::SEEK_TO_START)?;
        buf.clear();
        let mut ct = Measurement::default();

        loop {
            let bytes_read = reader.read_line(buf)?;
            if bytes_read == 0 {
                break;
            }

            if buf.starts_with("MemTotal") {
                let val = buf.split_whitespace().nth(1).expect("value").parse::<u64>()?;
                ct.total += val;
            } else if buf.starts_with("MemAvailable") {
                let val = buf.split_whitespace().nth(1).expect("value").parse::<u64>()?;
                ct.free += val;

                // Assume MemAvailable comes after MemTotal...
                break;
            }
            buf.clear();
        }

        Ok(ct)
    }
}

impl StatTaker for ProcMeminfo {
    fn measurement(&self) -> Measurement {
        self.curr
    }

    fn measure(&mut self) -> Result<f64> {
        self.curr = Self::parse_proc_meminfo(&mut self.reader, &mut self.buf)?;
        let pct = 100.0 * ((self.curr.total as f64 - self.curr.free as f64) / self.curr.total as f64);
        Ok(pct)
    }
}

pub struct ProcStat {
    reader: io::BufReader<fs::File>,
    buf: String,
    curr: Measurement,
    prev: Measurement,
}

impl ProcStat {
    const SEEK_TO_START: io::SeekFrom = io::SeekFrom::Start(0);

    pub fn new(f: fs::File) -> Self {
        let mut s = Self {
            reader: io::BufReader::with_capacity(8192, f),
            buf: String::with_capacity(8192),
            curr: Measurement::default(),
            prev: Measurement::default(),
        };

        // Try to initialize prev value.
        if let Ok(val) = Self::parse_proc_stat(&mut s.reader, &mut s.buf) {
            s.prev = val;
            thread::sleep(time::Duration::from_millis(100));
        }

        s
    }

    fn parse_proc_stat(reader: &mut io::BufReader<fs::File>, buf: &mut String) -> Result<Measurement> {
        reader.seek(Self::SEEK_TO_START)?;
        buf.clear();
        let mut ct = Measurement::default();

        loop {
            let bytes_read = reader.read_line(buf)?;

            // TODO: Figure out how to stop before intr, so that we can allocate a fixed amount of bytes for buf.
            //       Maybe get cpu count and just read <n_cpus> of lines?
            //       (seems to require libc (+ optionally num_cpus crate))
            if bytes_read == 0 || !buf.starts_with("cpu") {
                break;
            }

            for (i, val) in buf.split_whitespace().skip(1).enumerate() {
                let val = val.parse::<u64>()?;
                ct.total += val;

                // 4th element is the idle time.
                if i == 3 {
                    ct.free += val;
                }
            }
            buf.clear();
        }

        Ok(ct)
    }
}

impl StatTaker for ProcStat {
    fn measurement(&self) -> Measurement {
        self.curr
    }

    fn measure(&mut self) -> Result<f64> {
        self.curr = Self::parse_proc_stat(&mut self.reader, &mut self.buf)?;

        let di = self.curr.free - self.prev.free;
        let dt = self.curr.total - self.prev.total;
        let pct = 100.0 * (1.0 - di as f64 / dt as f64);

        self.prev.free = self.curr.free;
        self.prev.total = self.curr.total;

        Ok(pct)
    }
}
