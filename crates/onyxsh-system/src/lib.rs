//! System telemetry, `/proc` parsers, and Sparkline coordinate generator
//! (`src/onyxsh/system/metrics.py` & `src/onyxsh/ui/widgets/sparkline.py`).

pub mod gpu_disk;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// CPU tick counters parsed from the aggregate `cpu` line of `/proc/stat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuStatCounters {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuStatCounters {
    pub fn idle_ticks(&self) -> u64 {
        self.idle.saturating_add(self.iowait)
    }

    pub fn total_ticks(&self) -> u64 {
        self.user
            .saturating_add(self.nice)
            .saturating_add(self.system)
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
            .saturating_add(self.steal)
    }
}

/// Parses the first aggregate `cpu  ...` line from `/proc/stat`.
pub fn parse_proc_stat(contents: &str) -> Option<CpuStatCounters> {
    let line = contents.lines().find(|l| l.starts_with("cpu "))?;
    let mut parts = line.split_whitespace();
    let _cpu_label = parts.next()?;

    let mut next_u64 = || parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    Some(CpuStatCounters {
        user: next_u64(),
        nice: next_u64(),
        system: next_u64(),
        idle: next_u64(),
        iowait: next_u64(),
        irq: next_u64(),
        softirq: next_u64(),
        steal: next_u64(),
    })
}

/// Calculates CPU usage percentage `[0.0, 100.0]` between two `/proc/stat` snapshots.
pub fn calculate_cpu_usage_percent(prev: &CpuStatCounters, curr: &CpuStatCounters) -> f32 {
    let total_delta = curr.total_ticks().saturating_sub(prev.total_ticks());
    if total_delta == 0 {
        return 0.0;
    }
    let idle_delta = curr.idle_ticks().saturating_sub(prev.idle_ticks());
    let used_delta = total_delta.saturating_sub(idle_delta);
    ((used_delta as f64 / total_delta as f64) * 100.0).clamp(0.0, 100.0) as f32
}

/// Memory and Swap statistics parsed from `/proc/meminfo` (mirrors `MemoryData` in Python).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryData {
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub ram_percent: f32,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_percent: f32,
}

/// Parses `/proc/meminfo` content into a [`MemoryData`] struct.
pub fn parse_proc_meminfo(contents: &str) -> MemoryData {
    let mut mem_total_kb = 0u64;
    let mut mem_free_kb = 0u64;
    let mut mem_available_kb = None::<u64>;
    let mut buffers_kb = 0u64;
    let mut cached_kb = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;

    for line in contents.lines() {
        if let Some((key, rest)) = line.split_once(':') {
            let val_kb = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            match key.trim() {
                "MemTotal" => mem_total_kb = val_kb,
                "MemFree" => mem_free_kb = val_kb,
                "MemAvailable" => mem_available_kb = Some(val_kb),
                "Buffers" => buffers_kb = val_kb,
                "Cached" => cached_kb = val_kb,
                "SwapTotal" => swap_total_kb = val_kb,
                "SwapFree" => swap_free_kb = val_kb,
                _ => {}
            }
        }
    }

    let avail_kb = mem_available_kb.unwrap_or_else(|| {
        mem_free_kb
            .saturating_add(buffers_kb)
            .saturating_add(cached_kb)
    });
    let ram_used_kb = mem_total_kb.saturating_sub(avail_kb);
    let swap_used_kb = swap_total_kb.saturating_sub(swap_free_kb);

    let ram_percent = if mem_total_kb > 0 {
        ((ram_used_kb as f64 / mem_total_kb as f64) * 100.0) as f32
    } else {
        0.0
    };
    let swap_percent = if swap_total_kb > 0 {
        ((swap_used_kb as f64 / swap_total_kb as f64) * 100.0) as f32
    } else {
        0.0
    };

    MemoryData {
        ram_used_bytes: ram_used_kb * 1024,
        ram_total_bytes: mem_total_kb * 1024,
        ram_percent,
        swap_used_bytes: swap_used_kb * 1024,
        swap_total_bytes: swap_total_kb * 1024,
        swap_percent,
    }
}

/// Cumulative RX/TX byte counters across non-loopback interfaces from `/proc/net/dev`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkCounters {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Parses `/proc/net/dev` and sums RX (column 0) and TX (column 8) bytes across non-`lo` interfaces.
pub fn parse_proc_net_dev(contents: &str) -> NetworkCounters {
    let mut rx_total = 0u64;
    let mut tx_total = 0u64;

    for line in contents.lines().skip(2) {
        if let Some((iface, stats)) = line.split_once(':') {
            let iface = iface.trim();
            if iface == "lo" || iface.is_empty() {
                continue;
            }
            let cols: Vec<&str> = stats.split_whitespace().collect();
            if cols.len() >= 9 {
                let rx = cols[0].parse::<u64>().unwrap_or(0);
                let tx = cols[8].parse::<u64>().unwrap_or(0);
                rx_total = rx_total.saturating_add(rx);
                tx_total = tx_total.saturating_add(tx);
            }
        }
    }

    NetworkCounters {
        rx_bytes: rx_total,
        tx_bytes: tx_total,
    }
}

/// High-level snapshot consumed by the UI footer/popover.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub cpu_usage_percent: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub net_rx_kbps: f64,
    pub net_tx_kbps: f64,
    pub gpu_vram_used_mb: Option<u64>,
    pub gpu_vram_total_mb: Option<u64>,
}

impl SystemSnapshot {
    pub fn ram_usage_percent(&self) -> f32 {
        if self.ram_total_mb == 0 {
            return 0.0;
        }
        ((self.ram_used_mb as f64 / self.ram_total_mb as f64) * 100.0) as f32
    }
}

/// Fixed-capacity ring buffer for rendering Cairo Sparkline graphs (`ui/widgets/sparkline.py`).
#[derive(Debug, Clone)]
pub struct SparklineBuffer {
    capacity: usize,
    samples: VecDeque<f32>,
}

impl SparklineBuffer {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(2);
        let mut samples = VecDeque::with_capacity(cap);
        samples.resize(cap, 0.0);
        Self {
            capacity: cap,
            samples,
        }
    }

    /// Pushes a percentage sample `[0.0, 100.0]` into the ring buffer.
    pub fn push(&mut self, value_percent: f32) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value_percent.clamp(0.0, 100.0));
    }

    pub fn samples(&self) -> &VecDeque<f32> {
        &self.samples
    }

    /// Computes `(x, y)` coordinates in `[0..width, 0..height]` for Cairo path rendering.
    pub fn to_cairo_points(&self, width: f64, height: f64) -> Vec<(f64, f64)> {
        let n = self.samples.len();
        if n < 2 {
            return Vec::new();
        }
        let step_x = width / (n - 1) as f64;
        self.samples
            .iter()
            .enumerate()
            .map(|(idx, &val)| {
                let x = idx as f64 * step_x;
                let normalized = (val as f64 / 100.0).clamp(0.0, 1.0);
                let y = height - (normalized * height);
                (x, y)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_percentage() {
        let snap = SystemSnapshot {
            cpu_usage_percent: 12.5,
            ram_used_mb: 4096,
            ram_total_mb: 16384,
            ..Default::default()
        };
        assert_eq!(snap.ram_usage_percent(), 25.0);
    }

    #[test]
    fn test_parse_proc_stat_and_cpu_delta() {
        let t0 = "cpu  1000 100 300 8000 600 0 0 0 0 0\ncpu0 500 50 150 4000 300 0 0 0 0 0\n";
        let t1 = "cpu  1150 100 350 8200 600 0 0 0 0 0\ncpu0 575 50 175 4100 300 0 0 0 0 0\n";

        let c0 = parse_proc_stat(t0).expect("parsed t0");
        let c1 = parse_proc_stat(t1).expect("parsed t1");

        // Total delta = 10400 - 10000 = 400; Idle delta = 8800 - 8600 = 200 -> 50% CPU
        let usage = calculate_cpu_usage_percent(&c0, &c1);
        assert!((usage - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_proc_meminfo() {
        let sample = "\
MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:   12288000 kB
Buffers:          512000 kB
Cached:          4096000 kB
SwapTotal:       8192000 kB
SwapFree:        6144000 kB
";
        let mem = parse_proc_meminfo(sample);
        assert!((mem.ram_percent - 25.0).abs() < 0.01);
        assert!((mem.swap_percent - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_proc_net_dev() {
        let sample = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 9999999    1000    0    0    0     0          0         0  9999999    1000    0    0    0     0       0          0
 wlan0:  102400     200    0    0    0     0          0         0    51200     100    0    0    0     0       0          0
  eth0:   47600      50    0    0    0     0          0         0    28800      40    0    0    0     0       0          0
";
        let net = parse_proc_net_dev(sample);
        assert_eq!(net.rx_bytes, 150_000);
        assert_eq!(net.tx_bytes, 80_000);
    }

    #[test]
    fn test_sparkline_buffer_points() {
        let mut buf = SparklineBuffer::new(4);
        buf.push(0.0);
        buf.push(50.0);
        buf.push(100.0);
        buf.push(25.0);

        let pts = buf.to_cairo_points(300.0, 100.0);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (0.0, 100.0));
        assert_eq!(pts[1], (100.0, 50.0));
        assert_eq!(pts[2], (200.0, 0.0));
        assert_eq!(pts[3], (300.0, 75.0));
    }
}
