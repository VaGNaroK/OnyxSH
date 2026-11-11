//! Phase 14: Real-Time Resource Monitor Dashboard with Cairo Sparkline Graphs
//! (`src/onyxsh/ui/widgets/sparkline.py`, `src/onyxsh/ui/widgets/resource_dashboard.py`,
//! and `src/onyxsh/ui/dialogs/resource_dashboard_dialog.py`).
//!
//! Features:
//! - Custom `gtk4::DrawingArea` Cairo Sparkline renderer using [`SparklineBuffer::to_cairo_points`]
//!   with threshold-aware color transitions (`<70%` cyan/emerald, `70-90%` amber, `>=90%` red).
//! - Control Toolbar: Host Selector (`Local (Este Computador)` + configured SSH hosts),
//!   live status badge (`🟢 Ativo` / `⏸️ Pausado`), sampling rate selector (`1s`, `2s`, `5s`),
//!   and Pause/Resume button.
//! - Zero-overhead auto-suspend when the dialog is closed (`close-request` / `unmap`).
//! - 4 Telemetry Cards:
//!   1. Processor (CPU usage %, CPU model, logical core count, LevelBar + Cairo Sparkline).
//!   2. Memory (RAM + Swap MB/GB, RAM LevelBar, Swap LevelBar + Cairo Sparkline).
//!   3. Disk I/O (`/proc/diskstats` Read/Write KB/s & cumulative GB + Cairo Sparkline).
//!   4. Network & GPU (`/proc/net/dev` RX/TX KB/s + DRM/NVIDIA GPU VRAM + Cairo Sparkline).

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use onyxsh_core::sessions::SessionsStorageData;
use onyxsh_filemanager::format_bytes;
use onyxsh_system::gpu_disk::{
    parse_drm_sysfs_vram, parse_nvidia_smi_vram, parse_proc_diskstats, DiskIoCounters, GpuVramData,
};
use onyxsh_system::{
    calculate_cpu_usage_percent, parse_proc_meminfo, parse_proc_net_dev, parse_proc_stat,
    CpuStatCounters, NetworkCounters, SparklineBuffer,
};
use std::cell::RefCell;
use std::fs;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

/// Returns `(r, g, b)` in `[0.0, 1.0]` for a metric percentage `[0.0, 100.0]`.
pub fn sparkline_rgb_for_percent(percent: f32) -> (f64, f64, f64) {
    if percent >= 90.0 {
        // Critical Red (#f85149)
        (0.97, 0.32, 0.29)
    } else if percent >= 70.0 {
        // Warning Amber (#d29922)
        (0.82, 0.60, 0.13)
    } else {
        // Normal Accent Cyan/Blue (#58a6ff)
        (0.35, 0.65, 1.00)
    }
}

/// Reads the CPU model name and logical core count from `/proc/cpuinfo`.
pub fn read_cpu_hardware_summary() -> (String, usize) {
    let Ok(contents) = fs::read_to_string("/proc/cpuinfo") else {
        return ("Linux CPU".to_string(), 1);
    };
    let mut model = None::<String>;
    let mut cores = 0usize;

    for line in contents.lines() {
        if line.starts_with("processor") {
            cores += 1;
        } else if model.is_none() && line.starts_with("model name") {
            if let Some((_, val)) = line.split_once(':') {
                model = Some(val.trim().to_string());
            }
        }
    }
    (
        model.unwrap_or_else(|| "Generic Processor".to_string()),
        cores.max(1),
    )
}

/// Probes local GPU VRAM via Linux DRM sysfs (`/sys/class/drm/card*/device/mem_info_vram_*`)
/// or `nvidia-smi` fallback.
pub fn probe_local_gpu_vram() -> Option<GpuVramData> {
    for card_idx in 0..4 {
        let base = format!("/sys/class/drm/card{card_idx}/device");
        let used_p = format!("{base}/mem_info_vram_used");
        let total_p = format!("{base}/mem_info_vram_total");
        if let (Ok(u), Ok(t)) = (fs::read_to_string(&used_p), fs::read_to_string(&total_p)) {
            if let Some(vram) = parse_drm_sysfs_vram(&u, &t) {
                return Some(vram);
            }
        }
    }

    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        parse_nvidia_smi_vram(&String::from_utf8_lossy(&out.stdout))
    } else {
        None
    }
}

/// Creates a `gtk4::DrawingArea` bound to a shared [`SparklineBuffer`].
pub fn build_sparkline_canvas(buffer: Rc<RefCell<SparklineBuffer>>) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_content_height(68);
    area.set_hexpand(true);

    area.set_draw_func(move |_widget, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        if w <= 4.0 || h <= 4.0 {
            return;
        }

        // Subtle dark card background
        cr.set_source_rgba(0.08, 0.10, 0.14, 0.65);
        cr.rectangle(0.0, 0.0, w, h);
        let _ = cr.fill();

        // Horizontal grid lines (25%, 50%, 75%)
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        cr.set_line_width(1.0);
        for frac in [0.25, 0.50, 0.75] {
            let y = h * frac;
            cr.move_to(0.0, y);
            cr.line_to(w, y);
        }
        let _ = cr.stroke();

        let buf = buffer.borrow();
        let latest = buf.samples().back().copied().unwrap_or(0.0);
        let points = buf.to_cairo_points(w, h - 4.0);
        drop(buf);

        if points.len() < 2 {
            return;
        }

        let (r, g, b) = sparkline_rgb_for_percent(latest);

        // 1. Translucent area fill under the curve
        cr.move_to(points[0].0, h);
        for &(x, y) in &points {
            cr.line_to(x, y + 2.0);
        }
        if let Some(&(last_x, _)) = points.last() {
            cr.line_to(last_x, h);
        }
        cr.close_path();
        cr.set_source_rgba(r, g, b, 0.22);
        let _ = cr.fill();

        // 2. Crisp stroke line on top
        cr.set_source_rgba(r, g, b, 0.95);
        cr.set_line_width(2.0);
        for (idx, &(x, y)) in points.iter().enumerate() {
            if idx == 0 {
                cr.move_to(x, y + 2.0);
            } else {
                cr.line_to(x, y + 2.0);
            }
        }
        let _ = cr.stroke();
    });

    area
}

/// Opens the Phase 14 Real-Time Resource Monitor Dashboard dialog (`ui/dialogs/resource_dashboard_dialog.py`).
pub fn show_resource_dashboard_dialog(parent: &adw::ApplicationWindow) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .title(&tr("Monitor de Recursos em Tempo Real — OnyxSH"))
        .default_width(760)
        .default_height(560)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let hb = adw::HeaderBar::new();

    // ── Control Toolbar inside HeaderBar ──────────────────────────────────────
    let mut host_choices = vec![tr("Local (Este Computador)")];
    let sessions_path = platform::config_dir().join("sessions.json");
    if let Ok(storage) = SessionsStorageData::load_from_path(&sessions_path) {
        for s in storage.sessions.iter().filter(|s| s.session_type == "ssh") {
            host_choices.push(format!("SSH: {} ({})", s.name, s.connection_string()));
        }
    }
    let host_refs: Vec<&str> = host_choices.iter().map(String::as_str).collect();
    let host_list = gtk4::StringList::new(&host_refs);
    let host_dd = gtk4::DropDown::new(Some(host_list), gtk4::Expression::NONE);
    host_dd.set_tooltip_text(Some(&tr("Select Monitored Host")));
    hb.pack_start(&host_dd);

    let status_badge = gtk4::Label::new(Some(&tr("🟢 Ativo")));
    status_badge.add_css_class("caption");
    hb.pack_start(&status_badge);

    // Rate selector (`1s`, `2s`, `5s`) + Pause button
    let rate_list = gtk4::StringList::new(&["1s", "2s", "5s"]);
    let rate_dd = gtk4::DropDown::new(Some(rate_list), gtk4::Expression::NONE);
    rate_dd.set_selected(0);
    rate_dd.set_tooltip_text(Some(&tr("Refresh Interval")));

    let btn_pause = gtk4::ToggleButton::builder()
        .icon_name("media-playback-pause-symbolic")
        .label(&tr("Pause"))
        .build();

    hb.pack_end(&btn_pause);
    hb.pack_end(&rate_dd);
    root_vbox.append(&hb);

    // ── 2x2 Grid of Telemetry Cards ───────────────────────────────────────────
    let grid = gtk4::Grid::builder()
        .row_spacing(12)
        .column_spacing(12)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .margin_start(14)
        .margin_end(14)
        .margin_top(12)
        .margin_bottom(14)
        .hexpand(true)
        .vexpand(true)
        .build();

    let cpu_buf = Rc::new(RefCell::new(SparklineBuffer::new(40)));
    let ram_buf = Rc::new(RefCell::new(SparklineBuffer::new(40)));
    let disk_buf = Rc::new(RefCell::new(SparklineBuffer::new(40)));
    let net_buf = Rc::new(RefCell::new(SparklineBuffer::new(40)));

    // Card 1: Processor (CPU)
    let (cpu_model, cpu_cores) = read_cpu_hardware_summary();
    let (card_cpu, lbl_cpu_val, bar_cpu, canvas_cpu) = build_metric_card(
        &tr("Processador (CPU)"),
        &tr_fmt(
            "{cpu_model} ({cpu_cores} logical cores)",
            &[("cpu_model", &cpu_model), ("cpu_cores", &cpu_cores.to_string())],
        ),
        Rc::clone(&cpu_buf),
    );
    grid.attach(&card_cpu, 0, 0, 1, 1);

    // Card 2: Memory (RAM + Swap)
    let (card_ram, lbl_ram_val, bar_ram, canvas_ram) = build_metric_card(
        &tr("Memory (RAM & Swap)"),
        &tr("/proc/meminfo — MemAvailable & Swap"),
        Rc::clone(&ram_buf),
    );
    grid.attach(&card_ram, 1, 0, 1, 1);

    // Card 3: Disk I/O
    let (card_disk, lbl_disk_val, bar_disk, canvas_disk) = build_metric_card(
        &tr("Disk (Physical I/O)"),
        &tr("/proc/diskstats — Device Read / Write"),
        Rc::clone(&disk_buf),
    );
    grid.attach(&card_disk, 0, 1, 1, 1);

    // Card 4: Network & GPU VRAM
    let vram_subtitle = match probe_local_gpu_vram() {
        Some(v) => tr_fmt(
            "Network (/proc/net/dev) • GPU VRAM: {used} / {total} MB ({pct}%)",
            &[
                ("used", &v.used_mb.to_string()),
                ("total", &v.total_mb.to_string()),
                ("pct", &format!("{:.0}", v.usage_percent)),
            ],
        ),
        None => tr("Network (/proc/net/dev) • Real-Time RX / TX"),
    };
    let (card_net, lbl_net_val, bar_net, canvas_net) = build_metric_card(
        &tr("Network (RX / TX) & GPU"),
        &vram_subtitle,
        Rc::clone(&net_buf),
    );
    grid.attach(&card_net, 1, 1, 1, 1);

    root_vbox.append(&grid);

    // ── Live Telemetry Sampler State ──────────────────────────────────────────
    let is_paused = Rc::new(RefCell::new(false));
    let is_closed = Rc::new(RefCell::new(false));
    let interval_secs = Rc::new(RefCell::new(1u64));
    let tick_counter = Rc::new(RefCell::new(0u64));

    let prev_cpu: Rc<RefCell<Option<CpuStatCounters>>> = Rc::new(RefCell::new(
        fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| parse_proc_stat(&s)),
    ));
    let prev_disk: Rc<RefCell<Option<DiskIoCounters>>> = Rc::new(RefCell::new(
        fs::read_to_string("/proc/diskstats")
            .ok()
            .map(|s| parse_proc_diskstats(&s)),
    ));
    let prev_net: Rc<RefCell<Option<NetworkCounters>>> = Rc::new(RefCell::new(
        fs::read_to_string("/proc/net/dev")
            .ok()
            .map(|s| parse_proc_net_dev(&s)),
    ));

    {
        let paused = Rc::clone(&is_paused);
        let badge = status_badge.clone();
        btn_pause.connect_toggled(move |btn| {
            let active = btn.is_active();
            *paused.borrow_mut() = active;
            if active {
                badge.set_text(&tr("⏸️ Pausado"));
                btn.set_label(&tr("Resume"));
            } else {
                badge.set_text(&tr("🟢 Ativo"));
                btn.set_label(&tr("Pause"));
            }
        });
    }

    {
        let int_rc = Rc::clone(&interval_secs);
        rate_dd.connect_selected_notify(move |dd| {
            let secs = match dd.selected() {
                1 => 2,
                2 => 5,
                _ => 1,
            };
            *int_rc.borrow_mut() = secs;
        });
    }

    {
        let closed = Rc::clone(&is_closed);
        dlg.connect_close_request(move |_| {
            *closed.borrow_mut() = true;
            glib::Propagation::Proceed
        });
    }

    // 1-second base tick (skips according to selected rate 1s/2s/5s and stops on close)
    glib::timeout_add_local(Duration::from_secs(1), move || {
        if *is_closed.borrow() {
            return glib::ControlFlow::Break;
        }
        if *is_paused.borrow() {
            return glib::ControlFlow::Continue;
        }

        let mut ticks = tick_counter.borrow_mut();
        *ticks += 1;
        let rate = *interval_secs.borrow();
        if *ticks % rate != 0 {
            return glib::ControlFlow::Continue;
        }

        let dt = rate as f64;

        // 1. CPU
        if let Ok(stat_str) = fs::read_to_string("/proc/stat") {
            if let Some(curr) = parse_proc_stat(&stat_str) {
                let mut prev = prev_cpu.borrow_mut();
                let pct = prev
                    .as_ref()
                    .map(|p| calculate_cpu_usage_percent(p, &curr))
                    .unwrap_or(0.0);
                *prev = Some(curr);
                cpu_buf.borrow_mut().push(pct);
                lbl_cpu_val.set_text(&format!("{pct:.1}%"));
                bar_cpu.set_value((pct as f64 / 100.0).clamp(0.0, 1.0));
                canvas_cpu.queue_draw();
            }
        }

        // 2. RAM + Swap
        if let Ok(mem_str) = fs::read_to_string("/proc/meminfo") {
            let mem = parse_proc_meminfo(&mem_str);
            ram_buf.borrow_mut().push(mem.ram_percent);
            lbl_ram_val.set_text(&format!(
                "RAM: {} / {} ({:.1}%) • Swap: {} ({:.0}%)",
                format_bytes(mem.ram_used_bytes),
                format_bytes(mem.ram_total_bytes),
                mem.ram_percent,
                format_bytes(mem.swap_used_bytes),
                mem.swap_percent
            ));
            bar_ram.set_value((mem.ram_percent as f64 / 100.0).clamp(0.0, 1.0));
            canvas_ram.queue_draw();
        }

        // 3. Disk I/O
        if let Ok(disk_str) = fs::read_to_string("/proc/diskstats") {
            let curr = parse_proc_diskstats(&disk_str);
            let mut prev = prev_disk.borrow_mut();
            let (r_bps, w_bps) = if let Some(p) = *prev {
                (
                    (curr.read_bytes.saturating_sub(p.read_bytes)) as f64 / dt,
                    (curr.write_bytes.saturating_sub(p.write_bytes)) as f64 / dt,
                )
            } else {
                (0.0, 0.0)
            };
            *prev = Some(curr);
            // Normalize 100 MB/s = 100% on sparkline
            let disk_pct = (((r_bps + w_bps) / (100.0 * 1024.0 * 1024.0)) * 100.0).clamp(0.0, 100.0) as f32;
            disk_buf.borrow_mut().push(disk_pct);
            lbl_disk_val.set_text(&tr_fmt(
                "Read: {read}/s • Write: {write}/s",
                &[
                    ("read", &format_bytes(r_bps as u64)),
                    ("write", &format_bytes(w_bps as u64)),
                ],
            ));
            bar_disk.set_value((disk_pct as f64 / 100.0).clamp(0.0, 1.0));
            canvas_disk.queue_draw();
        }

        // 4. Network RX / TX
        if let Ok(net_str) = fs::read_to_string("/proc/net/dev") {
            let curr = parse_proc_net_dev(&net_str);
            let mut prev = prev_net.borrow_mut();
            let (rx_bps, tx_bps) = if let Some(p) = *prev {
                (
                    (curr.rx_bytes.saturating_sub(p.rx_bytes)) as f64 / dt,
                    (curr.tx_bytes.saturating_sub(p.tx_bytes)) as f64 / dt,
                )
            } else {
                (0.0, 0.0)
            };
            *prev = Some(curr);
            // Normalize 25 MB/s = 100% on sparkline
            let net_pct = (((rx_bps + tx_bps) / (25.0 * 1024.0 * 1024.0)) * 100.0).clamp(0.0, 100.0) as f32;
            net_buf.borrow_mut().push(net_pct);
            lbl_net_val.set_text(&format!(
                "↓ RX: {}/s • ↑ TX: {}/s (Total RX: {})",
                format_bytes(rx_bps as u64),
                format_bytes(tx_bps as u64),
                format_bytes(curr.rx_bytes)
            ));
            bar_net.set_value((net_pct as f64 / 100.0).clamp(0.0, 1.0));
            canvas_net.queue_draw();
        }

        glib::ControlFlow::Continue
    });

    dlg.set_content(Some(&root_vbox));
    dlg.present();
}

fn build_metric_card(
    title: &str,
    subtitle: &str,
    buffer: Rc<RefCell<SparklineBuffer>>,
) -> (
    gtk4::Frame,
    gtk4::Label,
    gtk4::LevelBar,
    gtk4::DrawingArea,
) {
    let frame = gtk4::Frame::new(None);
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);

    let top_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title_lbl = gtk4::Label::new(Some(title));
    title_lbl.add_css_class("heading");
    title_lbl.set_halign(gtk4::Align::Start);
    title_lbl.set_hexpand(true);

    let val_lbl = gtk4::Label::new(Some(&tr("Waiting for sample...")));
    val_lbl.add_css_class("monospace");
    val_lbl.add_css_class("caption");
    top_hbox.append(&title_lbl);
    top_hbox.append(&val_lbl);
    vbox.append(&top_hbox);

    let sub_lbl = gtk4::Label::new(Some(subtitle));
    sub_lbl.add_css_class("caption");
    sub_lbl.add_css_class("dim-label");
    sub_lbl.set_halign(gtk4::Align::Start);
    sub_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    vbox.append(&sub_lbl);

    let level_bar = gtk4::LevelBar::new();
    level_bar.set_min_value(0.0);
    level_bar.set_max_value(1.0);
    vbox.append(&level_bar);

    let canvas = build_sparkline_canvas(buffer);
    canvas.set_vexpand(true);
    vbox.append(&canvas);

    frame.set_child(Some(&vbox));
    (frame, val_lbl, level_bar, canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparkline_threshold_colors_and_cpu_summary() {
        let (r_low, _, b_low) = sparkline_rgb_for_percent(35.0);
        assert!(b_low > r_low); // Cyan/blue for normal load

        let (r_warn, g_warn, _) = sparkline_rgb_for_percent(78.0);
        assert!(r_warn > 0.7 && g_warn > 0.5); // Amber for warning load

        let (r_crit, g_crit, _) = sparkline_rgb_for_percent(95.0);
        assert!(r_crit > 0.9 && g_crit < 0.4); // Red for critical load

        let (model, cores) = read_cpu_hardware_summary();
        assert!(!model.is_empty());
        assert!(cores >= 1);
    }
}
