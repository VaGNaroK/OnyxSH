//! GPU VRAM (`nvidia-smi` / DRM sysfs) and Disk I/O (`/proc/diskstats`) Telemetry Parsers
//! (`src/onyxsh/system/metrics.py`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuVramData {
    pub used_mb: u64,
    pub total_mb: u64,
    pub usage_percent: f32,
}

/// Parses `nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits` output.
pub fn parse_nvidia_smi_vram(output: &str) -> Option<GpuVramData> {
    let first_line = output.lines().find(|l| !l.trim().is_empty())?;
    let (used_str, total_str) = first_line.split_once(',')?;
    let used_mb = used_str.trim().parse::<u64>().ok()?;
    let total_mb = total_str.trim().parse::<u64>().ok()?;
    if total_mb == 0 {
        return None;
    }
    let usage_percent = ((used_mb as f64 / total_mb as f64) * 100.0).clamp(0.0, 100.0) as f32;
    Some(GpuVramData {
        used_mb,
        total_mb,
        usage_percent,
    })
}

/// Parses Linux DRM sysfs `mem_info_vram_used` and `mem_info_vram_total` (in bytes).
pub fn parse_drm_sysfs_vram(used_bytes_str: &str, total_bytes_str: &str) -> Option<GpuVramData> {
    let used_bytes = used_bytes_str.trim().parse::<u64>().ok()?;
    let total_bytes = total_bytes_str.trim().parse::<u64>().ok()?;
    if total_bytes == 0 {
        return None;
    }
    let used_mb = used_bytes / (1024 * 1024);
    let total_mb = total_bytes / (1024 * 1024);
    let usage_percent = ((used_bytes as f64 / total_bytes as f64) * 100.0).clamp(0.0, 100.0) as f32;
    Some(GpuVramData {
        used_mb,
        total_mb,
        usage_percent,
    })
}

/// Cumulative sectors read/written across physical block devices from `/proc/diskstats`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskIoCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// Parses `/proc/diskstats` and sums sectors read (col 5) and written (col 9) for whole-disk
/// devices (`nvme0n1`, `sda`, `vda`), converting 512-byte sectors to bytes.
pub fn parse_proc_diskstats(contents: &str) -> DiskIoCounters {
    let mut read_sectors = 0u64;
    let mut write_sectors = 0u64;

    for line in contents.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let dev = cols[2];
        let is_whole_disk = (dev.starts_with("nvme") && !dev.contains('p'))
            || ((dev.starts_with("sd") || dev.starts_with("vd"))
                && dev.chars().last().map(|c| c.is_ascii_alphabetic()).unwrap_or(false));
        if !is_whole_disk {
            continue;
        }
        let r_sec = cols[5].parse::<u64>().unwrap_or(0);
        let w_sec = cols[9].parse::<u64>().unwrap_or(0);
        read_sectors = read_sectors.saturating_add(r_sec);
        write_sectors = write_sectors.saturating_add(w_sec);
    }

    DiskIoCounters {
        read_bytes: read_sectors.saturating_mul(512),
        write_bytes: write_sectors.saturating_mul(512),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nvidia_and_drm_vram() {
        let nv = parse_nvidia_smi_vram("2048, 8192\n").unwrap();
        assert_eq!(nv.used_mb, 2048);
        assert_eq!(nv.total_mb, 8192);
        assert!((nv.usage_percent - 25.0).abs() < 0.01);

        let drm = parse_drm_sysfs_vram("4294967296\n", "8589934592\n").unwrap();
        assert_eq!(drm.used_mb, 4096);
        assert_eq!(drm.total_mb, 8192);
        assert!((drm.usage_percent - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_proc_diskstats() {
        let sample = "\
   8       0 sda 100 0 2000 50 200 0 1000 30 0 40 80
   8       1 sda1 90 0 1800 40 190 0 900 25 0 35 65
 259       0 nvme0n1 500 0 4000 100 300 0 2000 60 0 50 160
 259       1 nvme0n1p1 450 0 3500 90 280 0 1900 55 0 45 145
";
        let disk = parse_proc_diskstats(sample);
        // Whole disks only: sda (2000 R, 1000 W) + nvme0n1 (4000 R, 2000 W) = 6000 R, 3000 W sectors
        assert_eq!(disk.read_bytes, 6000 * 512);
        assert_eq!(disk.write_bytes, 3000 * 512);
    }
}
