use std::fs;
use std::env;
use std::thread;
use std::time;
use std::io;
use std::io::{Write, BufRead, Seek};
use std::collections::VecDeque;
use anyhow::Result;
use getopts::Options;

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

struct BrailleGraph {
    output: String,
    data: VecDeque<i64>,
    length: usize,
}

impl BrailleGraph {
    pub fn new(length: usize) -> Self {
        Self {
            output: String::with_capacity(4 * length),
            data: VecDeque::with_capacity(length),
            length,
        }
    }

    pub fn graph(&self) -> &str {
        &self.output
    }

    pub fn update(&mut self, pct: i64) {
        if self.data.len() >= self.length {
            self.data.pop_front();
        }
        self.data.push_back(pct);

        self.refresh_graph();
    }

    fn pct_thresholds(i: i64) -> i64 {
        if i > 80 {
            4
        } else if i > 60 {
            3
        } else if i > 40 {
            2
        } else if i > 20 {
            1
        } else {
            0
        }
    }

    fn refresh_graph(&mut self) {
        self.output.clear();

        let mut iter = self.data.iter().peekable();
        while iter.peek().is_some() {
            let next: i64 = **iter.peek().unwrap();
            let curr: i64 = *iter.next().unwrap();

            let c = match (BrailleGraph::pct_thresholds(next), BrailleGraph::pct_thresholds(curr)) {
                (0, 0) => '\u{2800}', // '⠀'
                (0, 1) => '\u{2880}', // '⢀'
                (0, 2) => '\u{28A0}', // '⢠'
                (0, 3) => '\u{28B0}', // '⢰'
                (0, 4) => '\u{28B8}', // '⢸'
                (1, 0) => '\u{2840}', // '⡀'
                (1, 1) => '\u{28C0}', // '⣀'
                (1, 2) => '\u{28E0}', // '⣠'
                (1, 3) => '\u{28F0}', // '⣰'
                (1, 4) => '\u{28F8}', // '⣸'
                (2, 0) => '\u{2844}', // '⡄'
                (2, 1) => '\u{28C4}', // '⣄'
                (2, 2) => '\u{28E4}', // '⣤'
                (2, 3) => '\u{28F4}', // '⣴'
                (2, 4) => '\u{28FC}', // '⣼'
                (3, 0) => '\u{2846}', // '⡆'
                (3, 1) => '\u{28C6}', // '⣆'
                (3, 2) => '\u{28E6}', // '⣦'
                (3, 3) => '\u{28F6}', // '⣶'
                (3, 4) => '\u{28FE}', // '⣾'
                (4, 0) => '\u{2847}', // '⡇'
                (4, 1) => '\u{28C7}', // '⣇'
                (4, 2) => '\u{28E7}', // '⣧'
                (4, 3) => '\u{28F7}', // '⣷'
                (4, 4) => '\u{28FF}', // '⣿'
                _ => unreachable!("WHOAHOA!")
            };
            self.output.push(c);
        }
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} FILE [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() -> Result<()> {
    let mut opts = Options::new();
    opts.optflag("m", "memory", "graph memory used");
    opts.optopt("i", "interval", "update interval in seconds", "INTERVAL_SECS");
    opts.optopt("n", "graph_len", "graph length in characters", "LENGTH");
    opts.optflag("h", "help", "print help");

    let matches = opts.parse(env::args().skip(1))?;
    if matches.opt_present("h") {
        print_usage(env::args().collect::<String>().as_str(), opts);
        return Ok(());
    }

    let graph_memory = matches.opt_present("memory");
    let interval = time::Duration::from_secs(matches.opt_get_default::<u64>("interval", 1)?);
    let graph_len = matches.opt_get_default::<usize>("graph_len", 10)?;

    let f = fs::File::open(if graph_memory { "/proc/meminfo" } else { "/proc/stat" })?;
    let mut reader = ProcReader::new(f);
    let mut graph = BrailleGraph::new(graph_len);

    let stdout = io::stdout();
    let mut stdout_handle = stdout.lock();

    if graph_memory {
        loop {
            reader.read_mem_to_curr()?;

            let curr = reader.curr();
            let pct = 100.0 * ((curr.total as f64 - curr.free as f64) / curr.total as f64);

            graph.update(pct as i64);

            // Simply exit if unable to write to stdout.
            if writeln!(stdout_handle, "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"Memory usage {:.2}%\"}}",
                        pct, graph.graph(), pct, pad=graph_len).is_err() {
                return Ok(());
            }

            thread::sleep(interval);
        }
    } else {
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

            // Simply exit if unable to write to stdout.
            if writeln!(stdout_handle, "{{\"percentage\": {:.0}, \"text\": \"{:\u{2800}>pad$}\", \"tooltip\": \"CPU usage {:.2}%\"}}",
                        pct, graph.graph(), pct, pad=graph_len).is_err() {
                return Ok(());
            }

            reader.store_curr_to_prev();
            thread::sleep(interval);
        }
    }
}
