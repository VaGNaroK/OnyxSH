//! Native GTK4 + Libadwaita + VTE4 Application Window (`src/onyxsh/window.py` & `ui/window_ui.py`).
//! Includes Phase 9 (Advanced VTE Search Bar with `Aa`, `\b`, `.*` & Match Counter)
//! and Phase 10 (Main Hamburger Menu with Theme Selector, Font Sizer, and 11 Actions).

use crate::main_menu::{
    apply_gtk_theme_mode, change_font_size, menu_custom_css, parse_font_string, GtkThemeMode,
};
use crate::terminal_search::{apply_vte_search, format_occurrence_counter};
use crate::theme_engine::{generate_theme_css, is_light_hex_color, resolve_palette};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::window_state::{TabNodeState, WindowSessionState};
use onyxsh_core::{
    APP_ID, APP_NAME, APP_VERSION, COPYRIGHT, DEVELOPERS, DEVELOPER_NAME, ISSUE_URL, WEBSITE,
};
use crate::tftp_server_dialog::{show_tftp_server_dialog, TftpServerState};
use onyxsh_system::{
    calculate_cpu_usage_percent, parse_proc_meminfo, parse_proc_net_dev, parse_proc_stat,
};
use onyxsh_core::platform::CommandBuilder;
use onyxsh_terminal::pty_proxy::{spawn_proxied_pty, OscStreamEvent, ProxiedPtyHandle};
use onyxsh_terminal::spawner::{
    build_session_launch, prepare_local_shell_launch, PreparedLaunch, ProcessTracker,
};
use onyxsh_terminal::ssh_health::{SshHealthRegistry, SshHealthStatus};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use vte4::prelude::*;

static NEXT_TERM_SEQ: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static PROXIED_PTYS: RefCell<HashMap<String, ProxiedPtyHandle>> = RefCell::new(HashMap::new());
    static PROCESS_TRACKER: RefCell<ProcessTracker> = RefCell::new(ProcessTracker::new());
    static SSH_HEALTH_REGISTRY: RefCell<SshHealthRegistry> = RefCell::new(SshHealthRegistry::new());
}

fn assign_unique_term_name(term: &vte4::Terminal) -> (String, u64) {
    let id = NEXT_TERM_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("onyx-term-{id}");
    term.set_widget_name(&name);
    (name, id)
}

fn term_u64_id(term: &vte4::Terminal) -> u64 {
    term.widget_name()
        .as_str()
        .strip_prefix("onyx-term-")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

/// Wraps a terminal/paned tree in an expandable container box so split transitions
/// can replace the tab's root widget in-place without destroying the `adw::TabPage`.
fn create_tab_slot_box(inner: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let slot = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    slot.set_hexpand(true);
    slot.set_vexpand(true);
    slot.append(inner);
    slot
}

/// Collects every leaf `vte4::Terminal` inside a tab's widget tree in visual DFS order.
fn collect_terminals_in_widget(widget: &gtk4::Widget) -> Vec<vte4::Terminal> {
    let mut out = Vec::new();
    if let Ok(term) = widget.clone().downcast::<vte4::Terminal>() {
        out.push(term);
        return out;
    }
    if let Ok(paned) = widget.clone().downcast::<gtk4::Paned>() {
        if let Some(start) = paned.start_child() {
            out.extend(collect_terminals_in_widget(&start));
        }
        if let Some(end) = paned.end_child() {
            out.extend(collect_terminals_in_widget(&end));
        }
        return out;
    }
    if let Ok(bx) = widget.clone().downcast::<gtk4::Box>() {
        let mut child = bx.first_child();
        while let Some(c) = child {
            out.extend(collect_terminals_in_widget(&c));
            child = c.next_sibling();
        }
    }
    out
}

fn find_tab_page_for_terminal(
    tab_view: &adw::TabView,
    target: &vte4::Terminal,
) -> Option<adw::TabPage> {
    let n = tab_view.n_pages();
    for i in 0..n {
        let page = tab_view.nth_page(i);
        if collect_terminals_in_widget(&page.child())
            .iter()
            .any(|t| t == target)
        {
            return Some(page);
        }
    }
    None
}

fn find_focused_terminal_in_tab(tab_view: &adw::TabView) -> Option<vte4::Terminal> {
    let page = tab_view.selected_page()?;
    let terms = collect_terminals_in_widget(&page.child());
    terms
        .iter()
        .find(|t| t.has_focus() || t.is_focus())
        .cloned()
        .or_else(|| terms.into_iter().next())
}

/// Replaces `old_child` inside its direct parent (`gtk4::Paned` or `gtk4::Box` tab slot) with `new_child`.
fn replace_widget_in_parent(
    parent: &gtk4::Widget,
    old_child: &gtk4::Widget,
    new_child: &gtk4::Widget,
) -> bool {
    if let Ok(paned) = parent.clone().downcast::<gtk4::Paned>() {
        let is_start = paned.start_child().is_some_and(|w| &w == old_child);
        let is_end = paned.end_child().is_some_and(|w| &w == old_child);
        if is_start {
            paned.set_start_child(None::<&gtk4::Widget>);
            paned.set_start_child(Some(new_child));
            return true;
        }
        if is_end {
            paned.set_end_child(None::<&gtk4::Widget>);
            paned.set_end_child(Some(new_child));
            return true;
        }
        return false;
    }
    if let Ok(bx) = parent.clone().downcast::<gtk4::Box>() {
        bx.remove(old_child);
        bx.append(new_child);
        return true;
    }
    false
}

/// Splits the currently focused terminal pane in-place using a nested `gtk4::Paned`.
fn split_focused_pane(
    tab_view: &adw::TabView,
    orientation: gtk4::Orientation,
    new_term: vte4::Terminal,
    status_lbl: &gtk4::Label,
) -> bool {
    let Some(focused) = find_focused_terminal_in_tab(tab_view) else {
        return false;
    };
    let focused_w: gtk4::Widget = focused.clone().upcast();
    let Some(parent) = focused_w.parent() else {
        return false;
    };

    let alloc_size = if orientation == gtk4::Orientation::Horizontal {
        focused_w.width()
    } else {
        focused_w.height()
    };

    let new_paned = gtk4::Paned::new(orientation);
    new_paned.set_wide_handle(true);
    new_paned.set_hexpand(true);
    new_paned.set_vexpand(true);
    new_paned.set_resize_start_child(true);
    new_paned.set_shrink_start_child(false);
    new_paned.set_resize_end_child(true);
    new_paned.set_shrink_end_child(false);

    let new_paned_w: gtk4::Widget = new_paned.clone().upcast();
    if !replace_widget_in_parent(&parent, &focused_w, &new_paned_w) {
        return false;
    }

    new_paned.set_start_child(Some(&focused));
    new_paned.set_end_child(Some(&new_term));
    if alloc_size > 80 {
        new_paned.set_position(alloc_size / 2);
    }
    new_term.grab_focus();
    status_lbl.set_text(&if orientation == gtk4::Orientation::Horizontal {
        tr("Terminal dividido horizontalmente.")
    } else {
        tr("Terminal dividido verticalmente.")
    });
    true
}

/// Detaches `target_w` from its parent `gtk4::Paned`, collapses `paned` back into its
/// remaining sibling inside `grandparent`, and returns the sibling widget if collapsed.
fn collapse_paned_removing_child(
    paned: &gtk4::Paned,
    target_w: &gtk4::Widget,
) -> Option<gtk4::Widget> {
    let start = paned.start_child();
    let end = paned.end_child();
    let sibling_opt = if start.as_ref().is_some_and(|w| w == target_w) {
        end
    } else {
        start
    };
    // Clear focus child before detaching children so GtkPaned doesn't log focus warnings.
    paned.set_focus_child(None::<&gtk4::Widget>);
    paned.set_start_child(None::<&gtk4::Widget>);
    paned.set_end_child(None::<&gtk4::Widget>);

    if let Some(sibling) = sibling_opt {
        let paned_w: gtk4::Widget = paned.clone().upcast();
        if let Some(grandparent) = paned_w.parent() {
            if replace_widget_in_parent(&grandparent, &paned_w, &sibling) {
                return Some(sibling);
            }
        }
    }
    None
}

/// Closes `target` terminal pane inside a split container (collapsing parent `Paned` back to its sibling),
/// or closes its `adw::TabPage` (and optionally the window if `close_last_tab_window` is true).
fn close_terminal_pane(
    tab_view: &adw::TabView,
    target: &vte4::Terminal,
    window: Option<&adw::ApplicationWindow>,
    status_lbl: &gtk4::Label,
    close_last_tab_window: bool,
) -> bool {
    let Some(page) = find_tab_page_for_terminal(tab_view, target) else {
        return false;
    };
    let target_w: gtk4::Widget = target.clone().upcast();
    if let Some(parent) = target_w.parent() {
        if let Ok(paned) = parent.downcast::<gtk4::Paned>() {
            if let Some(sibling) = collapse_paned_removing_child(&paned, &target_w) {
                if let Some(first_term) = collect_terminals_in_widget(&sibling).into_iter().next() {
                    first_term.grab_focus();
                }
                status_lbl.set_text(&tr("Painel dividido encerrado."));
                return true;
            }
        }
    }

    if tab_view.n_pages() > 1 {
        tab_view.close_page(&page);
        status_lbl.set_text(&tr("Painel/Aba encerrado."));
        return true;
    }

    if close_last_tab_window {
        if let Some(win) = window {
            win.close();
            return true;
        }
    }
    false
}

/// Closes the focused pane inside a split container or closes its tab when >1 tabs exist.
fn close_focused_pane(tab_view: &adw::TabView, status_lbl: &gtk4::Label) -> bool {
    let Some(focused) = find_focused_terminal_in_tab(tab_view) else {
        return false;
    };
    close_terminal_pane(tab_view, &focused, None, status_lbl, false)
}

/// Moves the focused split pane out into its own standalone `adw::TabPage`.
fn move_focused_pane_to_new_tab(tab_view: &adw::TabView, status_lbl: &gtk4::Label) -> bool {
    let Some(focused) = find_focused_terminal_in_tab(tab_view) else {
        return false;
    };
    let focused_w: gtk4::Widget = focused.clone().upcast();
    let Some(parent) = focused_w.parent() else {
        return false;
    };
    let Ok(paned) = parent.downcast::<gtk4::Paned>() else {
        return false;
    };

    let _ = collapse_paned_removing_child(&paned, &focused_w);

    let slot = create_tab_slot_box(&focused);
    let page = tab_view.append(&slot);
    let title = crate::session_sidebar::get_pane_session(&focused)
        .map(|s| s.name)
        .unwrap_or_else(|| format!("Terminal {}", tab_view.n_pages()));
    page.set_title(&title);
    tab_view.set_selected_page(&page);
    focused.grab_focus();
    status_lbl.set_text(&tr("Painel movido para nova aba."));
    true
}

/// Cycles focus across split panes in the active tab (`forward` vs `backward`).
fn focus_adjacent_pane(
    tab_view: &adw::TabView,
    forward: bool,
    status_lbl: &gtk4::Label,
) -> bool {
    let Some(page) = tab_view.selected_page() else {
        return false;
    };
    let terms = collect_terminals_in_widget(&page.child());
    if terms.len() <= 1 {
        return false;
    }
    let idx = terms
        .iter()
        .position(|t| t.has_focus() || t.is_focus())
        .unwrap_or(0);
    let next_idx = if forward {
        (idx + 1) % terms.len()
    } else if idx == 0 {
        terms.len() - 1
    } else {
        idx - 1
    };
    terms[next_idx].grab_focus();
    status_lbl.set_text(&tr_fmt(
        "Foco no painel {idx}/{total}",
        &[
            ("idx", &(next_idx + 1).to_string()),
            ("total", &terms.len().to_string()),
        ],
    ));
    true
}

/// Scrolls the terminal `vadjustment` to the exact OSC 133 previous/next prompt row if available.
fn scroll_to_prompt_row(term: &vte4::Terminal, previous: bool) -> bool {
    let adj = match term.vadjustment() {
        Some(a) => a,
        None => return false,
    };
    let current_row = adj.value().round() as i64;
    let target_row_opt = if previous {
        crate::terminal_ux::previous_prompt_row_for(term, current_row)
            .or_else(|| crate::terminal_ux::previous_prompt_row_for(term, term.cursor_position().1))
    } else {
        crate::terminal_ux::next_prompt_row_for(term, current_row)
    };

    if let Some(target_row) = target_row_opt {
        let clamped = (target_row as f64)
            .max(adj.lower())
            .min((adj.upper() - adj.page_size()).max(adj.lower()));
        adj.set_value(clamped);
        return true;
    }

    // Fallback delta when shell didn't emit OSC 133 prompt marks yet
    let delta = if previous {
        -adj.step_increment() * 5.0
    } else {
        adj.step_increment() * 5.0
    };
    let new_val = (adj.value() + delta)
        .max(adj.lower())
        .min((adj.upper() - adj.page_size()).max(adj.lower()));
    adj.set_value(new_val);
    false
}

#[derive(Clone)]
struct TermSpawnRuntime {
    window: adw::ApplicationWindow,
    tab_view: adw::TabView,
    settings_rc: Rc<RefCell<AppSettings>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    ai_widget: gtk4::Box,
    btn_ai: gtk4::ToggleButton,
    submit_ai_prompt_cb: Rc<dyn Fn(&str)>,
    status_lbl: gtk4::Label,
    clicked_url_rc: Rc<RefCell<Option<String>>>,
    fm_sync_cwd_cb: Rc<dyn Fn(&std::path::Path)>,
    fm_sftp_cb: Rc<dyn Fn(&SessionItem)>,
    btn_fm: gtk4::ToggleButton,
}

fn spawn_terminal_with_runtime(
    rt: &TermSpawnRuntime,
    working_dir: Option<&str>,
    session: Option<&SessionItem>,
    argv_override: &[&str],
) -> vte4::Terminal {
    let settings = rt.settings_rc.borrow().clone();
    let term = vte4::Terminal::new();
    let (term_key, term_id) = assign_unique_term_name(&term);
    term.set_hexpand(true);
    term.set_vexpand(true);
    term.set_allow_hyperlink(true);
    apply_terminal_appearance(&term, &settings);

    // PCRE2_MULTILINE (0x0000_0400) is required by VTE's match regex engine.
    if let Ok(regex) = vte4::Regex::for_match(r"https?://[^\s<>\]\[(){}'\x22]+", 0x0000_0400) {
        let tag = term.match_add_regex(&regex, 0);
        term.match_set_cursor_name(tag, "pointer");
    }

    let eff_cwd = working_dir
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            session
                .and_then(|s| (!s.local_working_directory.is_empty()).then(|| s.local_working_directory.clone()))
        })
        .unwrap_or_else(|| platform::home_dir().to_string_lossy().into_owned());

    let is_prod = session.is_some_and(|s| s.is_production);
    crate::session_sidebar::register_pane_session(&term, session.cloned());
    crate::terminal_ux::init_terminal_ux_state(&term, is_prod, &eff_cwd);
    if is_prod {
        crate::terminal_ux::set_terminal_production_mode(&term, true);
    }

    crate::terminal_ux::attach_vte_ux_controllers(
        &term,
        Rc::clone(&rt.clicked_url_rc),
        &rt.window,
        &rt.ai_widget,
        &rt.btn_ai,
        Rc::clone(&rt.submit_ai_prompt_cb),
        &rt.status_lbl,
        &rt.settings_rc,
    );

    let launch_res: Result<PreparedLaunch, String> = if !argv_override.is_empty() {
        let builder = CommandBuilder::new();
        let raw_argv: Vec<String> = argv_override.iter().map(|s| (*s).to_string()).collect();
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        let argv = builder.wrap_host_command(&raw_argv, Some(&eff_cwd), Some(&env));
        Ok(PreparedLaunch {
            argv,
            env,
            working_directory: eff_cwd.clone(),
            temp_dir: None,
        })
    } else if let Some(sess) = session {
        let secret = crate::session_sidebar::resolve_session_secret(sess);
        build_session_launch(
            Some(sess),
            working_dir,
            (!secret.is_empty()).then_some(secret),
            &settings,
            Some(&CommandBuilder::new()),
        )
    } else {
        Ok(prepare_local_shell_launch(
            None,
            Some(&eff_cwd),
            settings.use_login_shell,
            true,
        ))
    };

    let destroyed = Rc::new(Cell::new(false));
    {
        let key_d = term_key.clone();
        let terms_d = Rc::clone(&rt.terminals_rc);
        let term_d = term.clone();
        let dest_flag = Rc::clone(&destroyed);
        term.connect_destroy(move |_| {
            dest_flag.set(true);
            terms_d.borrow_mut().retain(|t| t != &term_d);
            crate::terminal_ux::remove_terminal_ux_state(&term_d);
            SSH_HEALTH_REGISTRY.with(|reg| {
                reg.borrow_mut().unregister(term_id);
            });
            let handle_opt = PROXIED_PTYS.with(|map| map.borrow_mut().remove(&key_d));
            if let Some(mut handle) = handle_opt {
                PROCESS_TRACKER.with(|tr| {
                    tr.borrow_mut().unregister(handle.child_pid);
                });
                handle.shutdown();
            }
        });
    }

    match launch_res {
        Ok(launch) => {
            let (ev_tx, ev_rx) = std::sync::mpsc::channel::<OscStreamEvent>();
            match spawn_proxied_pty(&launch, settings.enable_osc52_clipboard, ev_tx) {
                Ok((vte_master_fd, handle)) => {
                    let child_pid = handle.child_pid;
                    PROCESS_TRACKER.with(|tr| {
                        tr.borrow_mut().register(
                            child_pid,
                            &launch.argv.join(" "),
                            launch.temp_dir.clone(),
                        );
                    });
                    if let Ok(vte_pty) =
                        vte4::Pty::foreign_sync(vte_master_fd, gio::Cancellable::NONE)
                    {
                        term.set_pty(Some(&vte_pty));
                    }
                    PROXIED_PTYS.with(|map| {
                        map.borrow_mut().insert(term_key.clone(), handle);
                    });

                    let post_login_cmd: Option<String> = session.and_then(|s| {
                        if s.post_login_command_enabled && !s.post_login_command.trim().is_empty() {
                            Some(s.post_login_command.trim().to_string())
                        } else {
                            None
                        }
                    });
                    let post_login_sent = Rc::new(Cell::new(false));
                    if let Some(ref plc) = post_login_cmd {
                        let t_plc = term.clone();
                        let plc_copy = plc.clone();
                        let sent_flag = Rc::clone(&post_login_sent);
                        let dest_flag = Rc::clone(&destroyed);
                        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                            if !dest_flag.get() && !sent_flag.get() {
                                sent_flag.set(true);
                                let payload = format!("{plc_copy}\n");
                                t_plc.feed_child(payload.as_bytes());
                            }
                        });
                    }

                    let rt_pump = rt.clone();
                    let term_pump = term.clone();
                    let key_pump = term_key.clone();
                    let dest_pump = Rc::clone(&destroyed);
                    let plc_opt = post_login_cmd;
                    let sent_pump = Rc::clone(&post_login_sent);

                    let is_remote = session.is_some_and(|s| s.session_type != "local");
                    let is_ssh_or_sftp = session
                        .is_some_and(|s| matches!(s.session_type.as_str(), "ssh" | "sftp"));
                    let cwd_sync_cb: Rc<dyn Fn(&str)> = {
                        let fm_sync = Rc::clone(&rt.fm_sync_cwd_cb);
                        Rc::new(move |display_path: &str| {
                            if !is_remote {
                                let expanded = platform::expand_user_path(display_path);
                                fm_sync(&expanded);
                            }
                        })
                    };

                    glib::timeout_add_local(Duration::from_millis(40), move || {
                        if dest_pump.get() {
                            return glib::ControlFlow::Break;
                        }
                        let page_opt = find_tab_page_for_terminal(&rt_pump.tab_view, &term_pump);
                        let st_snapshot = rt_pump.settings_rc.borrow().clone();

                        while let Ok(ev) = ev_rx.try_recv() {
                            if matches!(ev, OscStreamEvent::PromptStart) {
                                if let Some(ref plc) = plc_opt {
                                    if !sent_pump.get() {
                                        sent_pump.set(true);
                                        let payload = format!("{plc}\n");
                                        term_pump.feed_child(payload.as_bytes());
                                    }
                                }
                            }
                            crate::terminal_ux::handle_osc_stream_event(
                                &term_pump,
                                &ev,
                                &st_snapshot,
                                &rt_pump.window,
                                page_opt.as_ref(),
                                &rt_pump.status_lbl,
                                Some(&cwd_sync_cb),
                            );
                            if let OscStreamEvent::ChildExited(status) = ev {
                                PROCESS_TRACKER.with(|tr| {
                                    tr.borrow_mut().unregister(child_pid);
                                });
                                PROXIED_PTYS.with(|map| {
                                    map.borrow_mut().remove(&key_pump);
                                });
                                let keep_open_ssh_err = is_ssh_or_sftp && status != 0;
                                if keep_open_ssh_err {
                                    let msg = tr_fmt(
                                        "Processo do terminal encerrado (código {code}).",
                                        &[("code", &status.to_string())],
                                    );
                                    rt_pump.status_lbl.set_text(&msg);
                                    if let Some(ref page) = page_opt {
                                        let err_icon =
                                            gio::ThemedIcon::new("network-error-symbolic");
                                        page.set_icon(Some(&err_icon));
                                        page.set_indicator_icon(Some(&err_icon));
                                        page.set_indicator_activatable(true);
                                        page.set_indicator_tooltip(&msg);
                                        page.set_tooltip(&msg);
                                    }
                                } else {
                                    close_terminal_pane(
                                        &rt_pump.tab_view,
                                        &term_pump,
                                        Some(&rt_pump.window),
                                        &rt_pump.status_lbl,
                                        true,
                                    );
                                }
                                return glib::ControlFlow::Break;
                            }
                        }
                        glib::ControlFlow::Continue
                    });
                }
                Err(_) => {
                    let rt_fb = rt.clone();
                    let term_fb = term.clone();
                    let is_ssh_or_sftp_fb = session
                        .is_some_and(|s| matches!(s.session_type.as_str(), "ssh" | "sftp"));
                    term.connect_child_exited(move |t, status| {
                        if !(is_ssh_or_sftp_fb && status != 0) {
                            close_terminal_pane(
                                &rt_fb.tab_view,
                                t,
                                Some(&rt_fb.window),
                                &rt_fb.status_lbl,
                                true,
                            );
                        } else {
                            rt_fb.status_lbl.set_text(&tr_fmt(
                                "Processo do terminal encerrado (código {code}).",
                                &[("code", &status.to_string())],
                            ));
                        }
                    });
                    let arg_refs: Vec<&str> = launch.argv.iter().map(String::as_str).collect();
                    let env_strs: Vec<String> =
                        launch.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    let env_refs: Vec<&str> = env_strs.iter().map(String::as_str).collect();
                    term_fb.spawn_async(
                        vte4::PtyFlags::DEFAULT,
                        Some(&launch.working_directory),
                        &arg_refs,
                        &env_refs,
                        glib::SpawnFlags::SEARCH_PATH,
                        || {},
                        -1,
                        gio::Cancellable::NONE,
                        |_| {},
                    );
                }
            }
        }
        Err(err_msg) => {
            rt.status_lbl.set_text(&err_msg);
            let banner = format!("\r\n\x1b[1;31m⚠️  {err_msg}\x1b[0m\r\n");
            term.feed(banner.as_bytes());
        }
    }

    if let Some(sess) = session {
        if sess.session_type == "ssh" && !sess.host.is_empty() {
            SSH_HEALTH_REGISTRY.with(|reg| {
                reg.borrow_mut()
                    .register(term_id, &sess.name, &sess.host, sess.port);
            });
        }
        if sess.sftp_session_enabled {
            rt.btn_fm.set_active(true);
            (rt.fm_sftp_cb)(sess);
        }
    }

    rt.terminals_rc.borrow_mut().push(term.clone());
    term
}

/// Starts the proactive SSH Health Monitor timer (RTT probe in background thread + auto-reconnect countdown).
fn start_ssh_health_monitor_timer(rt: &TermSpawnRuntime) {
    let last_probe_at = Rc::new(RefCell::new(Instant::now() - Duration::from_secs(60)));
    let probe_in_flight = Rc::new(Cell::new(false));
    let rt_timer = rt.clone();

    glib::timeout_add_seconds_local(1, move || {
        let settings = rt_timer.settings_rc.borrow().clone();
        if !settings.ssh_health_check_enabled {
            return glib::ControlFlow::Continue;
        }

        // Collect all active SSH terminals for probe/countdown
        let active_terms: Vec<(u64, vte4::Terminal, SessionItem)> = rt_timer
            .terminals_rc
            .borrow()
            .iter()
            .filter_map(|t| {
                let sess = crate::session_sidebar::get_pane_session(t)?;
                if sess.session_type == "ssh" && !sess.host.is_empty() {
                    Some((term_u64_id(t), t.clone(), sess))
                } else {
                    None
                }
            })
            .collect();

        if active_terms.is_empty() {
            return glib::ControlFlow::Continue;
        }

        // Tick countdowns every 1s for reconnecting terminals
        for (tid, term, sess) in &active_terms {
            let mut trigger_reconnect = false;
            SSH_HEALTH_REGISTRY.with(|reg| {
                let mut r = reg.borrow_mut();
                if let Some(rec) = r.get_mut(*tid) {
                    if rec.is_auto_reconnecting && rec.countdown_seconds > 0 {
                        rec.countdown_seconds -= 1;
                        if rec.countdown_seconds == 0 && settings.ssh_auto_reconnect_enabled {
                            trigger_reconnect = true;
                        }
                    }
                }
            });
            if trigger_reconnect {
                let port = sess.port;
                let target = sess.connection_string();
                rt_timer.status_lbl.set_text(&tr_fmt(
                    "Reconectando sessão SSH {name}...",
                    &[("name", &sess.name)],
                ));
                let cmd = format!("\rssh -p {port} {target}\n");
                term.feed_child(cmd.as_bytes());
            }
        }

        let interval_secs = settings.ssh_health_check_interval.max(5) as u64;
        if !probe_in_flight.get()
            && last_probe_at.borrow().elapsed() >= Duration::from_secs(interval_secs)
        {
            *last_probe_at.borrow_mut() = Instant::now();
            probe_in_flight.set(true);

            let targets: Vec<(u64, String, u16)> = active_terms
                .iter()
                .map(|(tid, _, s)| (*tid, s.host.clone(), s.port))
                .collect();
            let (tx, rx) = std::sync::mpsc::channel::<(u64, Result<f64, String>)>();
            std::thread::spawn(move || {
                for (tid, host, port) in targets {
                    let t0 = Instant::now();
                    let addr_res: Result<Vec<SocketAddr>, _> = (host.as_str(), port)
                        .to_socket_addrs()
                        .map(|iter| iter.collect());
                    let outcome = match addr_res {
                        Ok(addrs) if !addrs.is_empty() => {
                            let timeout = Duration::from_secs(3);
                            let mut connected = Err("Connection failed".to_string());
                            for addr in addrs {
                                match TcpStream::connect_timeout(&addr, timeout) {
                                    Ok(_) => {
                                        connected = Ok(t0.elapsed().as_secs_f64() * 1000.0);
                                        break;
                                    }
                                    Err(e) => connected = Err(e.to_string()),
                                }
                            }
                            connected
                        }
                        Ok(_) => Err("DNS empty".to_string()),
                        Err(e) => Err(e.to_string()),
                    };
                    if tx.send((tid, outcome)).is_err() {
                        break;
                    }
                }
            });

            let in_flight_done = Rc::clone(&probe_in_flight);
            let rt_res = rt_timer.clone();
            glib::timeout_add_local(Duration::from_millis(120), move || {
                let st_now = rt_res.settings_rc.borrow().clone();
                loop {
                    match rx.try_recv() {
                        Ok((tid, outcome)) => {
                            let badge_opt = SSH_HEALTH_REGISTRY.with(|reg| {
                                let mut r = reg.borrow_mut();
                                let rec = r.get_mut(tid)?;
                                rec.max_reconnect_attempts = st_now.ssh_auto_reconnect_attempts;
                                match outcome {
                                    Ok(rtt_ms) => rec.record_probe_success(rtt_ms),
                                    Err(err) => rec.record_probe_failure(
                                        &err,
                                        st_now.ssh_auto_reconnect_enabled,
                                        5,
                                    ),
                                }
                                Some((rec.status, rec.badge_label()))
                            });
                            if let Some((health_status, badge)) = badge_opt {
                                if let Some((_, term, sess)) = active_terms.iter().find(|(id, _, _)| *id == tid) {
                                    if let Some(page) = find_tab_page_for_terminal(&rt_res.tab_view, term) {
                                        page.set_tooltip(&format!("{} — {badge}", sess.name));
                                        if matches!(
                                            health_status,
                                            SshHealthStatus::Unreachable | SshHealthStatus::Reconnecting
                                        ) {
                                            let warn_icon =
                                                gio::ThemedIcon::new("network-error-symbolic");
                                            page.set_indicator_icon(Some(&warn_icon));
                                        }
                                    }
                                    if find_focused_terminal_in_tab(&rt_res.tab_view)
                                        .is_some_and(|ft| &ft == term)
                                    {
                                        rt_res
                                            .status_lbl
                                            .set_text(&format!("SSH {}: {badge}", sess.name));
                                    }
                                }
                            }
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            return glib::ControlFlow::Continue;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            in_flight_done.set(false);
                            return glib::ControlFlow::Break;
                        }
                    }
                }
            });
        }

        glib::ControlFlow::Continue
    });
}

/// Applies the palette, font, cursor, and VTE options from `AppSettings` to an existing `vte4::Terminal`.
fn apply_terminal_appearance(term: &vte4::Terminal, settings: &AppSettings) {
    crate::preferences_ui::apply_all_settings_to_terminal(term, settings);
}

/// Resolves the active `vte4::Terminal` inside the currently selected `adw::TabPage`.
fn get_active_vte_terminal(tab_view: &adw::TabView) -> Option<vte4::Terminal> {
    find_focused_terminal_in_tab(tab_view)
}

/// Recursively restores a [`TabNodeState`] tree into GTK4 `vte4::Terminal` / `gtk4::Paned` widgets.
fn build_restored_tab_widget(
    node: &TabNodeState,
    rt: &TermSpawnRuntime,
    auto_reconnect_ssh: bool,
) -> (gtk4::Widget, String) {
    match node {
        TabNodeState::Terminal {
            session_type,
            title,
            working_directory,
            session_data,
        } => {
            let parsed_sess: Option<SessionItem> = session_data.clone();
            let active_sess = if auto_reconnect_ssh && session_type == "ssh" {
                parsed_sess.as_ref().filter(|s| !s.host.is_empty())
            } else if session_type == "local" {
                parsed_sess.as_ref()
            } else {
                None
            };
            let term = spawn_terminal_with_runtime(
                rt,
                working_directory.as_deref(),
                active_sess,
                &[],
            );
            if active_sess.is_none() && parsed_sess.is_some() {
                crate::session_sidebar::register_pane_session(&term, parsed_sess);
            }
            (term.upcast(), title.clone())
        }
        TabNodeState::Paned {
            orientation,
            position,
            position_ratio: _,
            child1,
            child2,
        } => {
            let gtk_orient = if orientation == "vertical" {
                gtk4::Orientation::Vertical
            } else {
                gtk4::Orientation::Horizontal
            };
            let paned = gtk4::Paned::new(gtk_orient);
            paned.set_wide_handle(true);
            paned.set_hexpand(true);
            paned.set_vexpand(true);
            let (w1, t1) = build_restored_tab_widget(child1, rt, auto_reconnect_ssh);
            let (w2, _) = build_restored_tab_widget(child2, rt, auto_reconnect_ssh);
            paned.set_start_child(Some(&w1));
            paned.set_end_child(Some(&w2));
            if *position > 0 {
                paned.set_position(*position);
            }
            (paned.upcast(), t1)
        }
    }
}

/// Applies a [`WindowSessionState`] snapshot into `tab_view`, removing any pre-existing tabs.
fn restore_state_into_tab_view(
    state: &WindowSessionState,
    tab_view: &adw::TabView,
    rt: &TermSpawnRuntime,
    btn_sidebar: &gtk4::ToggleButton,
    btn_ai: &gtk4::ToggleButton,
) {
    if state.tabs.is_empty() {
        return;
    }
    let settings = rt.settings_rc.borrow().clone();
    let old_pages: Vec<adw::TabPage> = (0..tab_view.n_pages())
        .map(|i| tab_view.nth_page(i))
        .collect();
    let mut restored_pages = Vec::new();
    for tab_node in &state.tabs {
        let (widget, title) =
            build_restored_tab_widget(tab_node, rt, settings.session_restore_ssh_auto_reconnect);
        let slot = create_tab_slot_box(&widget);
        let page = tab_view.append(&slot);
        page.set_title(if title.is_empty() { "Terminal" } else { &title });
        restored_pages.push(page);
    }
    for old in old_pages {
        tab_view.close_page(&old);
    }
    if let Some(target) = restored_pages.get(state.active_tab_index.min(restored_pages.len().saturating_sub(1))) {
        tab_view.set_selected_page(target);
    }
    if settings.session_restore_ui_panels {
        btn_sidebar.set_active(state.sidebar_visible);
        btn_ai.set_active(state.ai_panel_visible);
    }
}

/// Opens the SSH Tunnel Manager dialog (`win.show-tunnel-manager`).
fn show_tunnel_manager_dialog(parent: &adw::ApplicationWindow, status_label: &gtk4::Label) {
    crate::tunnel_ui::show_tunnel_manager_window(parent, status_label);
}

/// Registers the terminal context menu actions on the window and configures accelerators
/// matching `CORRIGIR/menucontext3.jpg`, `src/onyxsh/ui/menus.py` and `src/onyxsh/app.py`.
fn setup_terminal_window_actions(
    app: &adw::Application,
    rt: &TermSpawnRuntime,
    active_term_cb: &Rc<dyn Fn() -> Option<vte4::Terminal>>,
) {
    let window = &rt.window;
    let tab_view = &rt.tab_view;
    let ai_widget = &rt.ai_widget;
    let btn_ai = &rt.btn_ai;
    let submit_ai_prompt_cb = &rt.submit_ai_prompt_cb;
    let settings_rc = &rt.settings_rc;
    let status_lbl = &rt.status_lbl;
    let clicked_url_rc = &rt.clicked_url_rc;
    // 1. Copy (`win.copy`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("copy", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.copy_clipboard_format(vte4::Format::Text);
                st.set_text(&tr("Seleção copiada."));
            }
        });
        window.add_action(&action);
    }

    // 2. Paste (`win.paste`)
    {
        let act = Rc::clone(active_term_cb);
        let action = gio::SimpleAction::new("paste", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.paste_clipboard();
            }
        });
        window.add_action(&action);
    }

    // 3. Select All (`win.select-all`)
    {
        let act = Rc::clone(active_term_cb);
        let action = gio::SimpleAction::new("select-all", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.select_all();
            }
        });
        window.add_action(&action);
    }

    // 4. Export Terminal Buffer (`win.export-terminal-buffer`)
    {
        let win_ref = window.clone();
        let act = Rc::clone(active_term_cb);
        let st_lbl = status_lbl.clone();
        let action = gio::SimpleAction::new("export-terminal-buffer", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                crate::export_dialog::show_export_terminal_dialog(
                    &win_ref, &term, None, false, &st_lbl,
                );
            }
        });
        window.add_action(&action);
    }

    // 5. Export Terminal Runbook (`win.export-terminal-runbook`)
    {
        let win_ref = window.clone();
        let act = Rc::clone(active_term_cb);
        let st_lbl = status_lbl.clone();
        let action = gio::SimpleAction::new("export-terminal-runbook", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                crate::export_dialog::show_export_terminal_dialog(
                    &win_ref, &term, None, true, &st_lbl,
                );
            }
        });
        window.add_action(&action);
    }

    // 6. Clear Session (`win.clear-session`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("clear-session", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.reset(true, true);
                term.feed_child(b"\x0c");
                st.set_text(&tr("Sessão limpa."));
            }
        });
        window.add_action(&action);
    }

    // 7. Jump Previous Prompt (`win.jump-previous-prompt`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("jump-previous-prompt", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                scroll_to_prompt_row(&term, true);
                st.set_text(&tr("Pular para o prompt anterior."));
            }
        });
        window.add_action(&action);
    }

    // 8. Jump Next Prompt (`win.jump-next-prompt`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("jump-next-prompt", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                scroll_to_prompt_row(&term, false);
                st.set_text(&tr("Pular para o próximo prompt."));
            }
        });
        window.add_action(&action);
    }

    // 9. Resolve Error with OpenCode (`win.resolve-error-opencode`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("resolve-error-opencode", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let text = crate::global_search_and_shortcuts::extract_vte_text(&term);
                let recent_lines = text
                    .lines()
                    .rev()
                    .take(25)
                    .collect::<Vec<&str>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<&str>>()
                    .join("\n");
                let prompt = if recent_lines.trim().is_empty() {
                    tr("Analise o projeto e elabore as correções necessárias com OpenCode.")
                } else {
                    tr_fmt(
                        "O comando executado no terminal falhou. Analise os arquivos do projeto e elabore as correções necessárias com OpenCode:\n\n<untrusted>\n{lines}\n</untrusted>",
                        &[("lines", recent_lines.trim())],
                    )
                };
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Resolvendo erro do terminal com OpenCode / IA..."));
            }
        });
        window.add_action(&action);
    }

    // 10. Ask AI Selection (`win.ask-ai-selection`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("ask-ai-selection", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let selected = term
                    .text_selected(vte4::Format::Text)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                let prompt = tr_fmt(
                    "Por favor, analise a seguinte seleção do terminal:\n\n<untrusted>\n{sel}\n</untrusted>",
                    &[("sel", selected.trim())],
                );
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Seleção enviada para análise no Assistente de IA."));
            }
        });
        window.add_action(&action);
    }

    // 11. Refactor Selection with OpenCode (`win.opencode-selection`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("opencode-selection", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let selected = term
                    .text_selected(vte4::Format::Text)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                let prompt = tr_fmt(
                    "Por favor, refatore o seguinte trecho de código com OpenCode:\n\n<untrusted>\n{sel}\n</untrusted>",
                    &[("sel", selected.trim())],
                );
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Seleção enviada para refatoração com OpenCode."));
            }
        });
        window.add_action(&action);
    }

    // 12. Open URL (`win.open-url`)
    {
        let c_url = Rc::clone(clicked_url_rc);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("open-url", None);
        action.connect_activate(move |_, _| {
            if let Some(url) = c_url.borrow().as_deref() {
                let _ = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>);
                st.set_text(&tr_fmt("Abrindo link: {url}", &[("url", url)]));
            }
        });
        window.add_action(&action);
    }

    // 13. Copy URL (`win.copy-url`)
    {
        let c_url = Rc::clone(clicked_url_rc);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("copy-url", None);
        action.connect_activate(move |_, _| {
            if let Some(url) = c_url.borrow().as_deref() {
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(url);
                    st.set_text(&tr("Link copiado para a área de transferência."));
                }
            }
        });
        window.add_action(&action);
    }

    // 14. Split Horizontal (`win.split-horizontal`)
    {
        let rt_sh = rt.clone();
        let action = gio::SimpleAction::new("split-horizontal", None);
        action.connect_activate(move |_, _| {
            let new_term = spawn_terminal_with_runtime(&rt_sh, None, None, &[]);
            if !split_focused_pane(
                &rt_sh.tab_view,
                gtk4::Orientation::Horizontal,
                new_term.clone(),
                &rt_sh.status_lbl,
            ) {
                let slot = create_tab_slot_box(&new_term);
                let page = rt_sh.tab_view.append(&slot);
                page.set_title(&format!("Terminal {}", rt_sh.tab_view.n_pages()));
                rt_sh.tab_view.set_selected_page(&page);
            }
        });
        window.add_action(&action);
    }

    // 15. Split Vertical (`win.split-vertical`)
    {
        let rt_sv = rt.clone();
        let action = gio::SimpleAction::new("split-vertical", None);
        action.connect_activate(move |_, _| {
            let new_term = spawn_terminal_with_runtime(&rt_sv, None, None, &[]);
            if !split_focused_pane(
                &rt_sv.tab_view,
                gtk4::Orientation::Vertical,
                new_term.clone(),
                &rt_sv.status_lbl,
            ) {
                let slot = create_tab_slot_box(&new_term);
                let page = rt_sv.tab_view.append(&slot);
                page.set_title(&format!("Terminal {}", rt_sv.tab_view.n_pages()));
                rt_sv.tab_view.set_selected_page(&page);
            }
        });
        window.add_action(&action);
    }

    // 16. Close Pane (`win.close-pane`)
    {
        let tv = tab_view.clone();
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("close-pane", None);
        action.connect_activate(move |_, _| {
            close_focused_pane(&tv, &st);
        });
        window.add_action(&action);
    }

    // 17. Copy Last Output (`win.copy-last-output`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("copy-last-output", None);
        action.connect_activate(move |_, _| {
            let Some(term) = act() else {
                return;
            };
            if let Some((cmd_name, out_txt)) = crate::terminal_ux::last_command_output_for(&term) {
                let payload = if out_txt.trim().is_empty() {
                    cmd_name.clone()
                } else {
                    out_txt
                };
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&payload);
                    st.set_text(&tr_fmt(
                        "Saída copiada para a área de transferência ({cmd}).",
                        &[("cmd", &cmd_name)],
                    ));
                }
            } else {
                st.set_text(&tr("Nenhum comando anterior registrado neste terminal."));
            }
        });
        window.add_action(&action);
    }

    // 18. Move Pane to New Tab (`win.move-pane-to-tab`)
    {
        let tv = tab_view.clone();
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("move-pane-to-tab", None);
        action.connect_activate(move |_, _| {
            move_focused_pane_to_new_tab(&tv, &st);
        });
        window.add_action(&action);
    }

    // 19. Focus Pane Up/Left/Down/Right
    for (act_name, forward) in [
        ("focus-pane-up", false),
        ("focus-pane-left", false),
        ("focus-pane-down", true),
        ("focus-pane-right", true),
    ] {
        let tv = tab_view.clone();
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new(act_name, None);
        action.connect_activate(move |_, _| {
            focus_adjacent_pane(&tv, forward, &st);
        });
        window.add_action(&action);
    }

    // Bind standard accelerators matching `CORRIGIR/menucontext3.jpg` and `src/onyxsh/app.py`
    let st = settings_rc.borrow();
    let copy_accel = st.shortcuts.get("copy").cloned().unwrap_or_else(|| "<Control><Shift>Insert".to_string());
    let paste_accel = st.shortcuts.get("paste").cloned().unwrap_or_else(|| "<Control><Shift>v".to_string());
    let sel_accel = st.shortcuts.get("select-all").cloned().unwrap_or_else(|| "<Control><Shift>a".to_string());
    let clear_accel = st.shortcuts.get("clear-session").cloned().unwrap_or_else(|| "<Control><Shift>l".to_string());
    let prev_p_accel = st.shortcuts.get("jump-previous-prompt").cloned().unwrap_or_else(|| "<Alt>Page_Up".to_string());
    let next_p_accel = st.shortcuts.get("jump-next-prompt").cloned().unwrap_or_else(|| "<Alt>Page_Down".to_string());
    let split_h_accel = st.shortcuts.get("split-horizontal").cloned().unwrap_or_else(|| "<Control><Shift>parenleft".to_string());
    let split_v_accel = st.shortcuts.get("split-vertical").cloned().unwrap_or_else(|| "<Control><Shift>parenright".to_string());
    let close_p_accel = st.shortcuts.get("close-pane").cloned().unwrap_or_else(|| "<Control><Shift>k".to_string());

    let mut copy_accels = vec!["<Control><Shift>Insert".to_string(), "<Control><Shift>c".to_string()];
    if !copy_accels.contains(&copy_accel) {
        copy_accels.insert(0, copy_accel);
    }
    let copy_refs: Vec<&str> = copy_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.copy", &copy_refs);

    let paste_accels = vec![paste_accel, "<Shift>Insert".to_string()];
    let paste_refs: Vec<&str> = paste_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.paste", &paste_refs);

    app.set_accels_for_action("win.select-all", &[sel_accel.as_str()]);
    app.set_accels_for_action("win.clear-session", &[clear_accel.as_str()]);

    let mut prev_p_accels = vec!["<Alt>Page_Up".to_string(), "<Alt>Up".to_string()];
    if !prev_p_accels.contains(&prev_p_accel) {
        prev_p_accels.insert(0, prev_p_accel);
    }
    let prev_p_refs: Vec<&str> = prev_p_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.jump-previous-prompt", &prev_p_refs);

    let mut next_p_accels = vec!["<Alt>Page_Down".to_string(), "<Alt>Down".to_string()];
    if !next_p_accels.contains(&next_p_accel) {
        next_p_accels.insert(0, next_p_accel);
    }
    let next_p_refs: Vec<&str> = next_p_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.jump-next-prompt", &next_p_refs);

    app.set_accels_for_action("win.split-horizontal", &[split_h_accel.as_str()]);
    app.set_accels_for_action("win.split-vertical", &[split_v_accel.as_str()]);
    app.set_accels_for_action("win.close-pane", &[close_p_accel.as_str()]);
}

/// Builds and presents the main `adw::ApplicationWindow`.
pub fn build_main_window(
    app: &adw::Application,
    initial_settings: AppSettings,
    initial_cli: crate::CliOptions,
) {
    let _ = build_main_window_internal(app, initial_settings, initial_cli, false);
}

fn build_main_window_internal(
    app: &adw::Application,
    initial_settings: AppSettings,
    initial_cli: crate::CliOptions,
    skip_initial_tab: bool,
) -> adw::TabView {
    let settings_rc = Rc::new(RefCell::new(initial_settings));
    let terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>> = Rc::new(RefCell::new(Vec::new()));
    let active_tftp_server: Rc<RefCell<TftpServerState>> =
        Rc::new(RefCell::new(TftpServerState::default()));
    let tftp_lbl = gtk4::Label::new(Some(&tr("Servidor TFTP...")));
    tftp_lbl.set_halign(gtk4::Align::Start);
    tftp_lbl.set_hexpand(true);
    let clicked_url_rc: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    {
        let initial_theme_mode = GtkThemeMode::from_setting(&settings_rc.borrow().gtk_theme);
        let initial_pal = resolve_palette(&settings_rc.borrow().color_scheme);
        apply_gtk_theme_mode(
            initial_theme_mode,
            is_light_hex_color(&initial_pal.background),
        );
    }

    let css_provider = gtk4::CssProvider::new();
    {
        let st = settings_rc.borrow();
        let opacity_percent = 100u8.saturating_sub(st.transparency.min(100));
        let css_data = format!(
            "{}\n{}",
            generate_theme_css(&st.color_scheme, opacity_percent),
            menu_custom_css()
        );
        css_provider.load_from_string(&css_data);
    }
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("OnyxSH — Native Rust Terminal")
        .default_width(1280)
        .default_height(800)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // ── 1. HeaderBar ──────────────────────────────────────────────────────────
    let header_bar = adw::HeaderBar::new();

    let btn_sidebar = gtk4::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .tooltip_text(&tr("Exibir Painel de Sessões"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_sidebar);

    let btn_fm = gtk4::ToggleButton::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text(&tr("Gerenciador de Arquivos Dual-Pane"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_fm);

    let btn_ai = gtk4::ToggleButton::builder()
        .icon_name("avatar-default-symbolic")
        .tooltip_text(&tr("Assistente de IA Zero-Trust"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_ai);

    // Phase 9: Search Toggle Button (Lupa)
    let btn_search = gtk4::ToggleButton::builder()
        .icon_name("edit-find-symbolic")
        .tooltip_text(&tr("Buscar no Terminal (Ctrl+Shift+F)"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_search);

    let btn_split = gtk4::Button::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text(&tr("Dividir Terminal Horizontalmente"))
        .build();
    header_bar.pack_start(&btn_split);

    // Phase 13: Command Manager Dialog Button (`utilities-terminal-symbolic`)
    let btn_cmd_manager = gtk4::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text(&tr("Gerenciador de Comandos & Formulários Dinâmicos"))
        .build();
    header_bar.pack_start(&btn_cmd_manager);

    // Phase 14: Real-Time Resource Monitor Button (`computer-symbolic`)
    let btn_resource_monitor = gtk4::Button::builder()
        .icon_name("computer-symbolic")
        .tooltip_text(&tr("Monitor de Recursos em Tempo Real (Sparklines Cairo)"))
        .build();
    header_bar.pack_start(&btn_resource_monitor);

    // Phase 16: Autocomplete Assistant Button (`insert-text-symbolic`)
    let btn_autocomplete = gtk4::Button::builder()
        .icon_name("insert-text-symbolic")
        .tooltip_text(&tr("Autocompletar Inteligente (CompletionEngine)"))
        .build();
    header_bar.pack_start(&btn_autocomplete);

    // Global Multi-Tab / Multi-Source Search Button (`system-search-symbolic` — Ctrl+Shift+G)
    let btn_global_search = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(&tr(
            "Busca Global Multi-Abas, Sessões, Histórico & Snippets (Ctrl+Shift+G)",
        ))
        .build();
    header_bar.pack_start(&btn_global_search);

    // Enriched Command History Button (`document-open-recent-symbolic` — Ctrl+H)
    let btn_cmd_history = gtk4::Button::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text(&tr("Histórico Enriquecido de Comandos (Ctrl+H)"))
        .build();
    header_bar.pack_start(&btn_cmd_history);

    // Phase 16: Command Palette Button (`preferences-system-search-symbolic`)
    let btn_palette = gtk4::Button::builder()
        .icon_name("preferences-system-search-symbolic")
        .tooltip_text(&tr("Command Palette — Busca Unificada (Ctrl+Shift+P)"))
        .build();
    header_bar.pack_start(&btn_palette);

    // Phase 10: Main Hamburger Menu Button (`open-menu-symbolic`)
    let btn_menu = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(&tr("Main Menu"))
        .build();
    header_bar.pack_end(&btn_menu);

    let btn_new_tab = gtk4::Button::builder()
        .icon_name("tab-new-symbolic")
        .tooltip_text(&tr("Nova Aba de Terminal (Ctrl+Shift+T)"))
        .build();
    header_bar.pack_end(&btn_new_tab);

    root_vbox.append(&header_bar);

    // ── 2. Phase 9: Terminal SearchBar (`Gtk.SearchBar` + `Aa`, `\b`, `.*` + Counter) ──
    let search_bar = gtk4::SearchBar::new();
    search_bar.set_show_close_button(true);

    let search_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    search_hbox.add_css_class("search-bar-box");

    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_hexpand(true);
    search_entry.set_placeholder_text(Some(&tr("Buscar texto no terminal...")));
    search_bar.connect_entry(&search_entry);

    let search_occurrence_label = gtk4::Label::new(Some("0/0"));
    search_occurrence_label.add_css_class("dim-label");
    search_occurrence_label.add_css_class("monospace");
    search_occurrence_label.set_tooltip_text(Some(&tr("Ocorrências encontradas no terminal")));

    // Linked options box (`Aa`, `\b`, `.*`)
    let options_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    options_box.add_css_class("linked");

    let btn_case_sensitive = gtk4::ToggleButton::with_label("Aa");
    btn_case_sensitive.add_css_class("flat");
    btn_case_sensitive.set_tooltip_text(Some(&tr(
        "Diferenciar Maiúsculas/Minúsculas (Match Case)",
    )));
    btn_case_sensitive.set_active(settings_rc.borrow().search_case_sensitive);
    options_box.append(&btn_case_sensitive);

    let btn_whole_word = gtk4::ToggleButton::with_label(r"\b");
    btn_whole_word.add_css_class("flat");
    btn_whole_word.set_tooltip_text(Some(&tr("Palavra Inteira (Whole Word)")));
    btn_whole_word.set_active(settings_rc.borrow().search_whole_word);
    options_box.append(&btn_whole_word);

    let btn_regex = gtk4::ToggleButton::with_label(".*");
    btn_regex.add_css_class("flat");
    btn_regex.set_tooltip_text(Some(&tr("Use Regular Expression (Regex)")));
    btn_regex.set_active(settings_rc.borrow().search_use_regex);
    options_box.append(&btn_regex);

    let btn_search_prev = gtk4::Button::from_icon_name("go-up-symbolic");
    btn_search_prev.add_css_class("flat");
    btn_search_prev.set_tooltip_text(Some(&tr("Ocorrência Anterior (Shift+Enter)")));

    let btn_search_next = gtk4::Button::from_icon_name("go-down-symbolic");
    btn_search_next.add_css_class("flat");
    btn_search_next.set_tooltip_text(Some(&tr("Próxima Ocorrência (Enter)")));

    search_hbox.append(&search_entry);
    search_hbox.append(&search_occurrence_label);
    search_hbox.append(&options_box);
    search_hbox.append(&btn_search_prev);
    search_hbox.append(&btn_search_next);
    search_bar.set_child(Some(&search_hbox));
    root_vbox.append(&search_bar);

    // Synchronize Search Button <-> SearchBar search-mode-enabled
    {
        let sb = search_bar.clone();
        let se = search_entry.clone();
        btn_search.connect_toggled(move |btn| {
            let active = btn.is_active();
            if sb.is_search_mode() != active {
                sb.set_search_mode(active);
            }
            if active {
                se.grab_focus();
            }
        });
    }

    // ── 3. TabView + TabBar ───────────────────────────────────────────────────
    let tab_view = adw::TabView::new();
    tab_view.set_hexpand(true);
    tab_view.set_vexpand(true);
    {
        let app_cw = app.clone();
        let st_cw = Rc::clone(&settings_rc);
        tab_view.connect_create_window(move |_| {
            Some(build_main_window_internal(
                &app_cw,
                st_cw.borrow().clone(),
                crate::CliOptions::default(),
                true,
            ))
        });
        tab_view.connect_indicator_activated(|_, page| {
            page.set_icon(None::<&gio::Icon>);
            page.set_indicator_icon(None::<&gio::Icon>);
            page.set_indicator_activatable(false);
            page.set_indicator_tooltip("");
            page.set_tooltip("");
        });
    }

    let tab_bar = adw::TabBar::new();
    tab_bar.set_view(Some(&tab_view));
    tab_bar.set_autohide(false);
    root_vbox.append(&tab_bar);

    // Wire Search Execution State (`current_match_idx`, `total_matches`)
    let search_match_state = Rc::new(RefCell::new((0usize, 0usize)));
    let run_terminal_search: Rc<dyn Fn()> = {
        let tv = tab_view.clone();
        let se = search_entry.clone();
        let cs = btn_case_sensitive.clone();
        let ww = btn_whole_word.clone();
        let rx = btn_regex.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);

        Rc::new(move || {
            let query = se.text().to_string();
            if let Some(term) = get_active_vte_terminal(&tv) {
                let total = apply_vte_search(
                    &term,
                    &query,
                    cs.is_active(),
                    ww.is_active(),
                    rx.is_active(),
                );
                *mstate.borrow_mut() = (0, total);
                lbl.set_text(&format_occurrence_counter(0, total));
            } else {
                *mstate.borrow_mut() = (0, 0);
                lbl.set_text("0/0");
            }
        })
    };

    {
        let run = Rc::clone(&run_terminal_search);
        search_entry.connect_search_changed(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_case_sensitive.connect_toggled(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_whole_word.connect_toggled(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_regex.connect_toggled(move |_| run());
    }
    {
        let tv = tab_view.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);
        let next_fn = move || {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.search_find_next();
                let mut st = mstate.borrow_mut();
                if st.1 > 0 {
                    st.0 = (st.0 + 1) % st.1;
                }
                lbl.set_text(&format_occurrence_counter(st.0, st.1));
            }
        };
        let next_fn_rc = Rc::new(next_fn);
        let nf1 = Rc::clone(&next_fn_rc);
        btn_search_next.connect_clicked(move |_| nf1());
        let nf2 = Rc::clone(&next_fn_rc);
        search_entry.connect_activate(move |_| nf2());
    }
    {
        let tv = tab_view.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);
        btn_search_prev.connect_clicked(move |_| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.search_find_previous();
                let mut st = mstate.borrow_mut();
                if st.1 > 0 {
                    st.0 = if st.0 == 0 { st.1 - 1 } else { st.0 - 1 };
                }
                lbl.set_text(&format_occurrence_counter(st.0, st.1));
            }
        });
    }
    {
        let bs = btn_search.clone();
        let tv = tab_view.clone();
        search_bar.connect_search_mode_enabled_notify(move |sb| {
            let enabled = sb.is_search_mode();
            if bs.is_active() != enabled {
                bs.set_active(enabled);
            }
            if !enabled {
                if let Some(term) = get_active_vte_terminal(&tv) {
                    term.search_set_regex(None, 0);
                    term.unselect_all();
                }
            }
        });
    }

    // ── 4. Footer Telemetry Bar (`onyxsh-system`) ─────────────────────────────
    let footer_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    footer_box.set_margin_start(12);
    footer_box.set_margin_end(12);
    footer_box.set_margin_top(4);
    footer_box.set_margin_bottom(4);

    let status_lbl = gtk4::Label::new(Some(&tr("OnyxSH Rust Engine v0.14.0 — Ready")));
    status_lbl.add_css_class("caption");
    status_lbl.set_halign(gtk4::Align::Start);
    status_lbl.set_hexpand(true);

    let metrics_lbl = gtk4::Label::new(Some("CPU: 0.0% | RAM: -- / -- MB"));
    metrics_lbl.add_css_class("caption");
    metrics_lbl.set_halign(gtk4::Align::End);

    footer_box.append(&status_lbl);
    footer_box.append(&metrics_lbl);

    // ── 5. Phase 11: Complete Session Sidebar (5-Button CRUD + Folder Tree + Search) ──
    let spawn_rt_holder: Rc<RefCell<Option<TermSpawnRuntime>>> = Rc::new(RefCell::new(None));

    let spawn_session_cb: Rc<dyn Fn(&SessionItem, bool)> = {
        let rt_h = Rc::clone(&spawn_rt_holder);

        Rc::new(move |sess: &SessionItem, in_split: bool| {
            let Some(rt) = rt_h.borrow().clone() else {
                return;
            };

            let auto_tunnels = crate::tunnel_ui::auto_start_tunnels_for_session(&sess.name);
            let make_term = || spawn_terminal_with_runtime(&rt, None, Some(sess), &[]);

            if in_split {
                let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
                paned.set_wide_handle(true);
                paned.set_hexpand(true);
                paned.set_vexpand(true);
                let t1 = make_term();
                let t2 = make_term();
                paned.set_start_child(Some(&t1));
                paned.set_end_child(Some(&t2));
                let slot = create_tab_slot_box(&paned);
                let page = rt.tab_view.append(&slot);
                page.set_title(&format!("{} (Split)", sess.name));
                rt.tab_view.set_selected_page(&page);
                t1.grab_focus();
            } else {
                let term = make_term();
                let slot = create_tab_slot_box(&term);
                let page = rt.tab_view.append(&slot);
                let title = if sess.is_production {
                    format!("🔴 [PROD] {}", sess.name)
                } else {
                    sess.name.clone()
                };
                page.set_title(&title);
                rt.tab_view.set_selected_page(&page);
                term.grab_focus();
            }

            if auto_tunnels.is_empty() {
                rt.status_lbl.set_text(&tr_fmt(
                    "Conectado à sessão: {name}",
                    &[("name", &sess.name)],
                ));
            } else {
                rt.status_lbl.set_text(&tr_fmt(
                    "Conectado à sessão: {name} ({count} túnel(eis) SSH iniciados)",
                    &[
                        ("name", &sess.name),
                        ("count", &auto_tunnels.len().to_string()),
                    ],
                ));
            }
        })
    };

    let (sidebar_box, populate_sessions_sidebar) =
        crate::session_sidebar::build_session_sidebar(
            &window,
            &tab_view,
            &status_lbl,
            Rc::clone(&spawn_session_cb),
        );

    // Phase 13: Command Toolbar (Pinned Buttons) + Broadcast Bar
    let cmd_buttons_rc = Rc::new(RefCell::new(
        crate::command_manager_ui::load_command_buttons(),
    ));
    let active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>> = {
        let tv = tab_view.clone();
        Rc::new(move || get_active_vte_terminal(&tv))
    };
    let (cmd_and_broadcast_bar, rebuild_cmd_toolbar, btn_broadcast_toggle) =
        crate::command_manager_ui::build_command_and_broadcast_bars(
            &window,
            Rc::clone(&cmd_buttons_rc),
            Rc::clone(&terminals_rc),
            Rc::clone(&active_term_cb),
            &status_lbl,
        );

    {
        let win_ref = window.clone();
        let btns = Rc::clone(&cmd_buttons_rc);
        let rebuild = Rc::clone(&rebuild_cmd_toolbar);
        let terms_all = Rc::clone(&terminals_rc);
        let act_fn = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_cmd_manager.connect_clicked(move |_| {
            let terms_inner = Rc::clone(&terms_all);
            let act_inner = Rc::clone(&act_fn);
            let status_inner = status.clone();
            let disp: Rc<dyn Fn(&str, bool, bool)> =
                Rc::new(move |cmd_str: &str, exec: bool, broadcast: bool| {
                    let payload = if exec {
                        format!("{cmd_str}\n")
                    } else {
                        cmd_str.to_string()
                    };
                    if broadcast {
                        for t in terms_inner.borrow().iter() {
                            t.feed_child(payload.as_bytes());
                        }
                        status_inner.set_text(&tr_fmt(
                            "Broadcast enviado: {cmd}",
                            &[("cmd", cmd_str)],
                        ));
                    } else if let Some(t) = act_inner() {
                        t.feed_child(payload.as_bytes());
                        status_inner.set_text(&tr_fmt(
                            "Comando enviado: {cmd}",
                            &[("cmd", cmd_str)],
                        ));
                    }
                });
            crate::command_manager_ui::show_command_manager_dialog(
                &win_ref,
                Rc::clone(&btns),
                Rc::clone(&rebuild),
                disp,
            );
        });
    }

    // Phase 14: Resource Monitor Dashboard Dialog
    {
        let win_ref = window.clone();
        btn_resource_monitor.connect_clicked(move |_| {
            crate::resource_dashboard::show_resource_dashboard_dialog(&win_ref);
        });
    }

    // Phase 16: Production Warning Banner + Toggle Button in HeaderBar
    let (prod_banner, btn_prod_shield) = crate::terminal_ux::build_production_banner(
        &window,
        Rc::clone(&active_term_cb),
        &status_lbl,
    );
    header_bar.pack_start(&btn_prod_shield);

    // Phase 16: Autocomplete Assistant Dialog
    {
        let win_ref = window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        btn_autocomplete.connect_clicked(move |_| {
            crate::terminal_ux::show_autocomplete_assistant_dialog(
                &win_ref,
                Rc::clone(&act_term),
                &status,
            );
        });
    }

    // Center + Bottom File Manager Vertical Box
    let center_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    center_vbox.set_hexpand(true);
    center_vbox.set_vexpand(true);
    center_vbox.append(&prod_banner);
    center_vbox.append(&cmd_and_broadcast_bar);
    center_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let on_close_ai: Rc<dyn Fn()> = {
        let bai = btn_ai.clone();
        Rc::new(move || {
            bai.set_active(false);
        })
    };

    // Phase 15: Complete Zero-Trust AI Assistant Bottom Horizontal Panel (Python window_ui.py specification)
    let (ai_widget, submit_ai_prompt_cb) =
        crate::ai_assistant_ui::build_complete_ai_assistant_panel(
            &window,
            &settings_rc,
            Rc::clone(&active_term_cb),
            &status_lbl,
            Some(on_close_ai),
        );
    ai_widget.set_visible(false);

    let cd_in_terminal_cb: Rc<dyn Fn(&std::path::Path)> = {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        Rc::new(move |dir_path: &std::path::Path| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                let escaped = dir_path.to_string_lossy().replace('\'', "'\\''");
                let cmd = format!("cd '{escaped}'\n");
                term.feed_child(cmd.as_bytes());
                status.set_text(&tr_fmt(
                    "Diretório enviado ao terminal: {dir}",
                    &[("dir", &dir_path.display().to_string())],
                ));
            }
        })
    };

    let feed_terminal_cb: Rc<dyn Fn(&str)> = {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        Rc::new(move |cmd_str: &str| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.feed_child(cmd_str.as_bytes());
                status.set_text(&tr_fmt(
                    "Comando enviado ao terminal: {cmd}",
                    &[("cmd", cmd_str.trim())],
                ));
            }
        })
    };

    let ai_prompt_cb: Rc<dyn Fn(&str)> = {
        let ai_w = ai_widget.clone();
        let bai = btn_ai.clone();
        let sub = Rc::clone(&submit_ai_prompt_cb);
        Rc::new(move |prompt: &str| {
            ai_w.set_visible(true);
            bai.set_active(true);
            sub(prompt);
        })
    };

    let (fm_widget, fm_sync_cwd_cb, fm_sftp_cb) =
        crate::filemanager_ui::build_complete_file_manager_with_controls(
            &window,
            &status_lbl,
            cd_in_terminal_cb,
            feed_terminal_cb,
            Some(ai_prompt_cb),
        );
    fm_widget.set_visible(false);

    let spawn_rt = TermSpawnRuntime {
        window: window.clone(),
        tab_view: tab_view.clone(),
        settings_rc: Rc::clone(&settings_rc),
        terminals_rc: Rc::clone(&terminals_rc),
        ai_widget: ai_widget.clone(),
        btn_ai: btn_ai.clone(),
        submit_ai_prompt_cb: Rc::clone(&submit_ai_prompt_cb),
        status_lbl: status_lbl.clone(),
        clicked_url_rc: Rc::clone(&clicked_url_rc),
        fm_sync_cwd_cb,
        fm_sftp_cb,
        btn_fm: btn_fm.clone(),
    };
    *spawn_rt_holder.borrow_mut() = Some(spawn_rt.clone());
    start_ssh_health_monitor_timer(&spawn_rt);

    // Vertical GtkPaned allowing user to resize file manager height up/down
    let fm_paned = gtk4::Paned::new(gtk4::Orientation::Vertical);
    fm_paned.add_css_class("filemanager-paned");
    fm_paned.set_hexpand(true);
    fm_paned.set_vexpand(true);
    fm_paned.set_wide_handle(true);
    fm_paned.set_start_child(Some(&tab_view));
    fm_paned.set_end_child(Some(&fm_widget));
    fm_paned.set_resize_start_child(true);
    fm_paned.set_shrink_start_child(false);
    fm_paned.set_resize_end_child(true);
    fm_paned.set_shrink_end_child(false);

    // Vertical GtkPaned for Zero-Trust AI Assistant bottom horizontal panel (Python window_ui.py specification)
    let ai_paned = gtk4::Paned::new(gtk4::Orientation::Vertical);
    ai_paned.add_css_class("ai-paned");
    ai_paned.set_hexpand(true);
    ai_paned.set_vexpand(true);
    ai_paned.set_wide_handle(true);
    ai_paned.set_start_child(Some(&fm_paned));
    ai_paned.set_end_child(Some(&ai_widget));
    ai_paned.set_resize_start_child(true);
    ai_paned.set_shrink_start_child(false);
    ai_paned.set_resize_end_child(true);
    ai_paned.set_shrink_end_child(false);

    center_vbox.append(&ai_paned);

    // Main Horizontal Layout
    let main_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    main_hbox.set_hexpand(true);
    main_hbox.set_vexpand(true);
    sidebar_box.set_visible(false);
    let sidebar_sep = gtk4::Separator::new(gtk4::Orientation::Vertical);
    sidebar_sep.set_visible(false);
    main_hbox.append(&sidebar_box);
    main_hbox.append(&sidebar_sep);
    main_hbox.append(&center_vbox);

    root_vbox.append(&main_hbox);
    root_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    root_vbox.append(&footer_box);

    // ── 6. Phase 10: Build Main Hamburger Menu Popover (`ui/menus.py`) ────────
    {
        let menu_popover = gtk4::Popover::new();
        menu_popover.set_has_arrow(true);
        let menu_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        menu_vbox.set_width_request(280);
        menu_vbox.set_margin_start(10);
        menu_vbox.set_margin_end(10);
        menu_vbox.set_margin_top(8);
        menu_vbox.set_margin_bottom(8);

        // 6.1 ThemeSelectorWidget (4 circular buttons: Follow System, Light, Dark, Terminal Match)
        let theme_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 22);
        theme_hbox.add_css_class("themeselector");
        theme_hbox.set_halign(gtk4::Align::Center);

        let rb_system = gtk4::CheckButton::builder()
            .tooltip_text(&tr("Seguir Estilo do Sistema"))
            .build();
        rb_system.add_css_class("follow");
        rb_system.add_css_class("theme-selector");

        let rb_light = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Estilo Claro (Light)"))
            .build();
        rb_light.add_css_class("light");
        rb_light.add_css_class("theme-selector");

        let rb_dark = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Estilo Escuro (Dark)"))
            .build();
        rb_dark.add_css_class("dark");
        rb_dark.add_css_class("theme-selector");

        let rb_terminal = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Combinar com Cores do Terminal"))
            .build();
        rb_terminal.add_css_class("terminal");
        rb_terminal.add_css_class("theme-selector");

        match settings_rc.borrow().gtk_theme.as_str() {
            "light" => rb_light.set_active(true),
            "dark" => rb_dark.set_active(true),
            "terminal" => rb_terminal.set_active(true),
            _ => rb_system.set_active(true),
        }

        {
            let st = Rc::clone(&settings_rc);
            rb_system.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "default".to_string();
                    let pal = resolve_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::System, is_light_hex_color(&pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_light.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "light".to_string();
                    let pal = resolve_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Light, is_light_hex_color(&pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_dark.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "dark".to_string();
                    let pal = resolve_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Dark, is_light_hex_color(&pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_terminal.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "terminal".to_string();
                    let pal = resolve_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(
                        GtkThemeMode::TerminalMatch,
                        is_light_hex_color(&pal.background),
                    );
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }

        theme_hbox.append(&rb_system);
        theme_hbox.append(&rb_light);
        theme_hbox.append(&rb_dark);
        theme_hbox.append(&rb_terminal);
        menu_vbox.append(&theme_hbox);

        // 6.2 FontSizerWidget (`-`, `12 pt`, `+`)
        let font_center = gtk4::CenterBox::new();
        font_center.add_css_class("font-sizer");
        let zoom_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);

        let btn_zoom_out = gtk4::Button::from_icon_name("zoom-out-symbolic");
        btn_zoom_out.add_css_class("flat");
        btn_zoom_out.set_tooltip_text(Some(&tr("Diminuir Fonte (Ctrl+-)")));

        let (_, initial_pt) = parse_font_string(&settings_rc.borrow().font);
        let font_size_lbl = gtk4::Label::new(Some(&format!("{initial_pt} pt")));
        font_size_lbl.set_width_chars(6);
        let btn_zoom_reset = gtk4::Button::new();
        btn_zoom_reset.add_css_class("flat");
        btn_zoom_reset.set_tooltip_text(Some(&tr("Resetar Tamanho da Fonte (12 pt)")));
        btn_zoom_reset.set_child(Some(&font_size_lbl));

        let btn_zoom_in = gtk4::Button::from_icon_name("zoom-in-symbolic");
        btn_zoom_in.add_css_class("flat");
        btn_zoom_in.set_tooltip_text(Some(&tr("Aumentar Fonte (Ctrl++)")));

        let update_all_fonts = {
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let lbl = font_size_lbl.clone();
            Rc::new(move |delta: Option<i32>| {
                let mut st = st_rc.borrow_mut();
                let (new_font, new_pt) = match delta {
                    Some(d) => change_font_size(&st.font, d),
                    None => ("Monospace 12".to_string(), 12),
                };
                st.font = new_font;
                lbl.set_text(&format!("{new_pt} pt"));
                for term in terms_rc.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                let cfg_path = platform::config_dir().join("settings.json");
                let _ = st.save_to_path(&cfg_path);
            })
        };

        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_out.connect_clicked(move |_| uf(Some(-1)));
        }
        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_reset.connect_clicked(move |_| uf(None));
        }
        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_in.connect_clicked(move |_| uf(Some(1)));
        }

        zoom_box.append(&btn_zoom_out);
        zoom_box.append(&btn_zoom_reset);
        zoom_box.append(&btn_zoom_in);
        font_center.set_center_widget(Some(&zoom_box));
        menu_vbox.append(&font_center);
        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // Helper to create flat menu action buttons with optional shortcut accelerator
        let make_menu_btn = |label: &str, icon: Option<&str>, shortcut: Option<&str>| -> gtk4::Button {
            let btn = gtk4::Button::new();
            btn.add_css_class("flat");
            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            if let Some(ic) = icon {
                hbox.append(&gtk4::Image::from_icon_name(ic));
            }
            let lbl = gtk4::Label::new(Some(label));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_hexpand(true);
            hbox.append(&lbl);
            if let Some(sc) = shortcut {
                let sc_lbl = gtk4::Label::new(Some(sc));
                sc_lbl.set_halign(gtk4::Align::End);
                sc_lbl.add_css_class("dim-label");
                hbox.append(&sc_lbl);
            }
            btn.set_child(Some(&hbox));
            btn
        };

        // 1. Nova Janela
        let mi_new_win = make_menu_btn(&tr("Nova Janela"), Some("window-new-symbolic"), Some("Shift+Ctrl+N"));
        {
            let app_clone = app.clone();
            let st_rc = Rc::clone(&settings_rc);
            let pop = menu_popover.clone();
            mi_new_win.connect_clicked(move |_| {
                pop.popdown();
                let st = st_rc.borrow().clone();
                build_main_window(&app_clone, st, crate::CliOptions::default());
            });
        }
        menu_vbox.append(&mi_new_win);

        // 2. Exportar Saída do Terminal & Caderno de Bordo (Runbook)
        let mi_export = make_menu_btn(
            &tr("Export Terminal Output..."),
            Some("document-save-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_export.connect_clicked(move |_| {
                pop.popdown();
                if let Some(term) = get_active_vte_terminal(&tv) {
                    let tab_title = tv.selected_page().map(|p| p.title().to_string());
                    crate::export_dialog::show_export_terminal_dialog(
                        &win_clone,
                        &term,
                        tab_title.as_deref(),
                        false,
                        &status,
                    );
                }
            });
        }
        menu_vbox.append(&mi_export);

        let mi_runbook = make_menu_btn(
            &tr("Exportar Sessão como Caderno de Bordo (Runbook)..."),
            Some("document-edit-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_runbook.connect_clicked(move |_| {
                pop.popdown();
                if let Some(term) = get_active_vte_terminal(&tv) {
                    let tab_title = tv.selected_page().map(|p| p.title().to_string());
                    crate::export_dialog::show_export_terminal_dialog(
                        &win_clone,
                        &term,
                        tab_title.as_deref(),
                        true,
                        &status,
                    );
                }
            });
        }
        menu_vbox.append(&mi_runbook);

        // 3. Preferências (4 Abas Completas — `adw::PreferencesWindow`)
        let mi_prefs = make_menu_btn(&tr("Preferences"), Some("preferences-system-symbolic"), Some("Shift+Ctrl+,"));
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_prefs.connect_clicked(move |_| {
                pop.popdown();
                crate::preferences_ui::show_preferences_window(
                    &win_clone,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_prefs);

        // 4. Cores (Abre HighlightDialog / coresfix.png)
        let mi_colors = make_menu_btn(
            &tr("Colors"),
            Some("format-text-highlight-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_colors.connect_clicked(move |_| {
                pop.popdown();
                crate::highlight_dialog_ui::show_highlight_dialog(
                    &win_clone,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_colors);

        // 5. Assistente de IA
        let mi_ai = make_menu_btn(&tr("Assistente de IA"), Some("avatar-default-symbolic"), None);
        {
            let bai = btn_ai.clone();
            let pop = menu_popover.clone();
            mi_ai.connect_clicked(move |_| {
                pop.popdown();
                bai.set_active(!bai.is_active());
            });
        }
        menu_vbox.append(&mi_ai);

        // 5b. Configurar Assistente de IA
        let mi_ai_cfg = make_menu_btn(
            &tr("Configurar Assistente de IA..."),
            Some("preferences-system-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_ai_cfg.connect_clicked(move |_| {
                pop.popdown();
                crate::ai_config_ui::show_ai_config_window(&win_clone, Rc::clone(&st_rc), &status);
            });
        }
        menu_vbox.append(&mi_ai_cfg);

        // 6. Atalhos de Teclado (Editável — 34 Ações)
        let mi_shortcuts = make_menu_btn(
            &tr("Atalhos de Teclado"),
            Some("preferences-desktop-keyboard-shortcuts-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_shortcuts.connect_clicked(move |_| {
                pop.popdown();
                let cfg_path = platform::config_dir().join("settings.json");
                crate::global_search_and_shortcuts::show_editable_shortcuts_dialog(
                    &win_clone,
                    &st_rc,
                    &cfg_path,
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_shortcuts);

        // 7. Importar Sessões (~/.ssh/config)
        let mi_import = make_menu_btn(
            &tr("Importar Sessões (~/.ssh/config)"),
            Some("document-open-symbolic"),
            None,
        );
        {
            let pop_sess = Rc::clone(&populate_sessions_sidebar);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_import.connect_clicked(move |_| {
                pop.popdown();
                pop_sess();
                status.set_text(&tr(
                    "Sessões sincronizadas a partir de ~/.config/onyxsh/sessions.json e ~/.ssh/config.",
                ));
            });
        }
        menu_vbox.append(&mi_import);

        // 8. Gerenciador de Túneis SSH
        let mi_tunnels = make_menu_btn(
            &tr("SSH Tunnel Manager"),
            Some("network-vpn-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_tunnels.connect_clicked(move |_| {
                pop.popdown();
                show_tunnel_manager_dialog(&win_clone, &status);
            });
        }
        menu_vbox.append(&mi_tunnels);

        // 9. Servidor TFTP (RFC 1350)
        let mi_tftp = gtk4::Button::new();
        mi_tftp.add_css_class("flat");
        let tftp_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        tftp_hbox.append(&gtk4::Image::from_icon_name("network-transmit-receive-symbolic"));
        tftp_hbox.append(&tftp_lbl);
        mi_tftp.set_child(Some(&tftp_hbox));
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let tftp_rc = Rc::clone(&active_tftp_server);
            let lbl_clone = tftp_lbl.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_tftp.connect_clicked(move |_| {
                pop.popdown();
                let lbl_upd = lbl_clone.clone();
                show_tftp_server_dialog(&win_clone, &st_rc, &tftp_rc, &status, move |active_addr| {
                    if let Some(addr) = active_addr {
                        lbl_upd.set_text(&tr_fmt(
                            "Servidor TFTP ({addr})",
                            &[("addr", &addr)],
                        ));
                    } else {
                        lbl_upd.set_text(&tr("Servidor TFTP..."));
                    }
                });
            });
        }
        menu_vbox.append(&mi_tftp);

        // 10. Sobre
        let mi_about = make_menu_btn(&tr("Sobre o OnyxSH"), Some("help-about-symbolic"), None);
        {
            let win_clone = window.clone();
            let pop = menu_popover.clone();
            mi_about.connect_clicked(move |_| {
                pop.popdown();

                let gtk_ver = format!(
                    "{}.{}.{}",
                    gtk4::major_version(),
                    gtk4::minor_version(),
                    gtk4::micro_version()
                );
                let adw_ver = format!(
                    "{}.{}.{}",
                    adw::major_version(),
                    adw::minor_version(),
                    adw::micro_version()
                );
                let debug_info = format!(
                    "App ID: {}\nVersion: {}\nPlatform: Linux\nArchitecture: {}\nShell: {}\nGTK: {}\nlibadwaita: {}\nFlatpak: {}\nConfig: {}",
                    APP_ID,
                    APP_VERSION,
                    std::env::consts::ARCH,
                    std::env::var("SHELL").unwrap_or_else(|_| "N/A".to_string()),
                    gtk_ver,
                    adw_ver,
                    platform::is_flatpak_sandbox(),
                    platform::config_dir().display(),
                );

                let about = adw::AboutWindow::builder()
                    .transient_for(&win_clone)
                    .modal(true)
                    .application_name(APP_NAME)
                    .application_icon(APP_ID)
                    .version(APP_VERSION)
                    .developer_name(DEVELOPER_NAME)
                    .developers(DEVELOPERS)
                    .copyright(COPYRIGHT)
                    .website(WEBSITE)
                    .issue_url(ISSUE_URL)
                    .license_type(gtk4::License::Gpl30)
                    .comments(&tr(
                        "Emulador de terminal moderno para fluxos de trabalho locais e remotos no Linux, com ProductionGuard, Dual-Pane SFTP/TFTP e Assistente de IA Zero-Trust.",
                    ))
                    .debug_info(&debug_info)
                    .build();

                about.add_link(&tr("Código-Fonte"), WEBSITE);
                about.add_link(&tr("Reportar Problema"), ISSUE_URL);

                about.present();
            });
        }
        menu_vbox.append(&mi_about);

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // 11. Sair
        let mi_quit = make_menu_btn(&tr("Quit"), Some("application-exit-symbolic"), Some("Shift+Ctrl+Q"));
        {
            let app_clone = app.clone();
            mi_quit.connect_clicked(move |_| {
                app_clone.quit();
            });
        }
        menu_vbox.append(&mi_quit);

        menu_popover.set_child(Some(&menu_vbox));
        btn_menu.set_popover(Some(&menu_popover));
    }

    // ── 7. Spawn Initial Terminal Tab (or Restore state.json) & Wire Shortcuts ─
    let state_path = platform::config_dir().join("state.json");
    let saved_state = if state_path.exists() {
        WindowSessionState::load_from_path(&state_path).ok()
    } else {
        None
    };

    let cli_has_override =
        initial_cli.execute_command.is_some() || initial_cli.working_directory.is_some();
    let policy = settings_rc.borrow().session_restore_policy.clone();

    if skip_initial_tab {
        // Detached window receiving a dragged `AdwTabPage` via `create-window`
    } else if !cli_has_override
        && policy == "always"
        && saved_state.as_ref().is_some_and(|s| !s.tabs.is_empty())
    {
        if let Some(ref state) = saved_state {
            restore_state_into_tab_view(state, &tab_view, &spawn_rt, &btn_sidebar, &btn_ai);
        }
    } else {
        let initial_term = if let Some(ref cmd) = initial_cli.execute_command {
            let argv = ["/bin/sh", "-c", cmd.as_str()];
            spawn_terminal_with_runtime(
                &spawn_rt,
                initial_cli.working_directory.as_deref(),
                None,
                &argv,
            )
        } else {
            spawn_terminal_with_runtime(
                &spawn_rt,
                initial_cli.working_directory.as_deref(),
                None,
                &[],
            )
        };
        let first_slot = create_tab_slot_box(&initial_term);
        let first_page = tab_view.append(&first_slot);
        first_page.set_title("Terminal 1");
        tab_view.set_selected_page(&first_page);

        if !cli_has_override && policy == "ask" {
            if let Some(state) = saved_state {
                let has_interesting_state = state.tabs.len() > 1
                    || state.tabs.iter().any(|t| t.terminal_count() > 1);
                if has_interesting_state {
                    let dialog = adw::MessageDialog::builder()
                        .transient_for(&window)
                        .modal(true)
                        .heading(&tr("Restaurar Sessão Anterior"))
                        .body(&tr_fmt(
                            "Deseja restaurar {count} aba(s) da sessão anterior?",
                            &[("count", &state.tabs.len().to_string())],
                        ))
                        .default_response("restore")
                        .close_response("dismiss")
                        .build();
                    dialog.add_response("dismiss", &tr("Cancel"));
                    dialog.add_response("restore", &tr("Restaurar Sessão"));
                    dialog.set_response_appearance("restore", adw::ResponseAppearance::Suggested);

                    let tv_r = tab_view.clone();
                    let rt_r = spawn_rt.clone();
                    let bs_r = btn_sidebar.clone();
                    let ba_r = btn_ai.clone();
                    dialog.connect_response(None, move |dlg, resp| {
                        if resp == "restore" {
                            restore_state_into_tab_view(&state, &tv_r, &rt_r, &bs_r, &ba_r);
                        }
                        dlg.close();
                    });
                    dialog.present();
                }
            }
        }
    }

    // Window action for secondary single-instance CLI invocations (`win.open-cli-tab`)
    {
        let rt_cli = spawn_rt.clone();
        let act = gio::SimpleAction::new("open-cli-tab", Some(glib::VariantTy::STRING));
        act.connect_activate(move |_, param| {
            let Some(raw) = param.and_then(|v| v.str()) else {
                return;
            };
            let Ok(opts) = serde_json::from_str::<crate::CliOptions>(raw) else {
                return;
            };
            let term = if let Some(ref cmd) = opts.execute_command {
                let argv = ["/bin/sh", "-c", cmd.as_str()];
                spawn_terminal_with_runtime(
                    &rt_cli,
                    opts.working_directory.as_deref(),
                    None,
                    &argv,
                )
            } else {
                spawn_terminal_with_runtime(
                    &rt_cli,
                    opts.working_directory.as_deref(),
                    None,
                    &[],
                )
            };
            let slot = create_tab_slot_box(&term);
            let page = rt_cli.tab_view.append(&slot);
            page.set_title(&format!("Terminal {}", rt_cli.tab_view.n_pages()));
            rt_cli.tab_view.set_selected_page(&page);
            rt_cli.window.present();
        });
        window.add_action(&act);
    }

    {
        let sb = sidebar_box.clone();
        let sep = sidebar_sep.clone();
        btn_sidebar.connect_toggled(move |btn| {
            let active = btn.is_active();
            sb.set_visible(active);
            sep.set_visible(active);
            btn.set_icon_name(if active {
                "sidebar-hide-symbolic"
            } else {
                "sidebar-show-symbolic"
            });
            btn.set_tooltip_text(Some(&if active {
                tr("Hide Sessions Panel")
            } else {
                tr("Exibir Painel de Sessões")
            }));
        });
    }
    let is_programmatic_pos = Rc::new(Cell::new(false));
    {
        let fm = fm_widget.clone();
        let paned = fm_paned.clone();
        let st_rc = Rc::clone(&settings_rc);
        let win_ref = window.clone();
        let is_prog = Rc::clone(&is_programmatic_pos);

        btn_fm.connect_toggled(move |btn| {
            let active = btn.is_active();
            if active {
                is_prog.set(true);

                // Make file manager visible first so GTK Paned has both children realized/visible
                fm.set_visible(true);

                let paned_h = paned.height();
                let win_h = win_ref.height();
                let alloc_h = if paned_h > 200 {
                    paned_h
                } else {
                    (win_h - 100).max(400)
                };

                let raw_saved = st_rc.borrow().file_manager_height as i32;
                let (_target_fm_h, target_pos) =
                    crate::filemanager_ui::calculate_file_manager_split(alloc_h, raw_saved);

                paned.set_position(target_pos);

                let p_clone = paned.clone();
                let st_clone = Rc::clone(&st_rc);
                let w_clone = win_ref.clone();
                let is_prog_clone = Rc::clone(&is_prog);

                glib::idle_add_local_once(move || {
                    let total_h = p_clone.height();
                    let alloc_h = if total_h > 200 {
                        total_h
                    } else {
                        (w_clone.height() - 100).max(400)
                    };
                    let raw_saved = st_clone.borrow().file_manager_height as i32;
                    let (_target_fm_h, target_pos) =
                        crate::filemanager_ui::calculate_file_manager_split(alloc_h, raw_saved);
                    p_clone.set_position(target_pos);

                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        is_prog_clone.set(false);
                    });
                });
            } else {
                is_prog.set(true);
                // Save last position before hiding, matching python tabs.py closing logic
                let total_h = paned.height();
                let pos = paned.position();
                if total_h > 200 && pos > 50 {
                    let min_fm = 240;
                    let min_term = 120;
                    let max_fm = (total_h - min_term).max(min_fm);
                    let last_h = (total_h - pos).clamp(min_fm, max_fm) as u32;
                    if last_h >= 240 {
                        st_rc.borrow_mut().file_manager_height = last_h;
                        let st_to_save = st_rc.borrow().clone();
                        let cfg_p = platform::config_dir().join("settings.json");
                        let _ = st_to_save.save_to_path(&cfg_p);
                    }
                }
                fm.set_visible(false);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    is_prog_clone.set(false);
                });
            }
        });
    }

    // Connect notify::position on fm_paned to dynamically update and persist file_manager_height
    {
        let paned = fm_paned.clone();
        let fm = fm_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let debounce_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let is_prog_track = Rc::clone(&is_programmatic_pos);

        paned.connect_position_notify(move |p| {
            if !fm.is_visible() || is_prog_track.get() {
                return;
            }
            let total_h = p.height();
            let pos = p.position();
            if total_h <= 200 || pos <= 50 {
                return;
            }
            let min_fm = 240;
            let min_term = 120;
            let max_fm = (total_h - min_term).max(min_fm);
            let calculated_fm_h = (total_h - pos).clamp(min_fm, max_fm) as u32;

            if st_rc.borrow().file_manager_height != calculated_fm_h {
                st_rc.borrow_mut().file_manager_height = calculated_fm_h;

                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let st_persist = Rc::clone(&st_rc);
                let timer_holder = Rc::clone(&debounce_timer);
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        timer_holder.borrow_mut().take();
                        let cfg_path = platform::config_dir().join("settings.json");
                        let _ = st_persist.borrow().save_to_path(&cfg_path);
                    },
                );
                *debounce_timer.borrow_mut() = Some(new_id);
            }
        });
    }
    // Programmatic position flag to prevent feedback loops between set_position and position_notify on ai_paned
    let is_programmatic_ai_pos = Rc::new(std::cell::Cell::new(false));

    // Toggle AI Assistant panel visibility & set ~50% horizontal split (Python window_ui.py specification)
    {
        let paned = ai_paned.clone();
        let ai = ai_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let win = window.clone();
        let is_prog = Rc::clone(&is_programmatic_ai_pos);

        btn_ai.connect_toggled(move |btn| {
            let active = btn.is_active();
            if active {
                is_prog.set(true);
                ai.set_visible(true);
                let total_h = paned.height();
                let alloc_h = if total_h > 200 {
                    total_h
                } else {
                    (win.height() - 100).max(400)
                };
                let raw_saved = st_rc.borrow().ai_panel_height as i32;
                let (_target_ai_h, target_pos) =
                    crate::ai_assistant_ui::calculate_ai_panel_split(alloc_h, raw_saved);
                paned.set_position(target_pos);

                let p_clone = paned.clone();
                let w_clone = win.clone();
                let st_clone = Rc::clone(&st_rc);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::idle_add_local_once(move || {
                    let total_h = p_clone.height();
                    let alloc_h = if total_h > 200 {
                        total_h
                    } else {
                        (w_clone.height() - 100).max(400)
                    };
                    let raw_saved = st_clone.borrow().ai_panel_height as i32;
                    let (_target_ai_h, target_pos) =
                        crate::ai_assistant_ui::calculate_ai_panel_split(alloc_h, raw_saved);
                    p_clone.set_position(target_pos);

                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        is_prog_clone.set(false);
                    });
                });
            } else {
                is_prog.set(true);
                let total_h = paned.height();
                let pos = paned.position();
                if total_h > 200 && pos > 50 {
                    let min_ai = 200;
                    let min_term = 120;
                    let max_ai = (total_h - min_term).max(min_ai);
                    let last_h = (total_h - pos).clamp(min_ai, max_ai) as u32;
                    if last_h >= 200 {
                        st_rc.borrow_mut().ai_panel_height = last_h;
                        let st_to_save = st_rc.borrow().clone();
                        let cfg_p = platform::config_dir().join("settings.json");
                        let _ = st_to_save.save_to_path(&cfg_p);
                    }
                }
                ai.set_visible(false);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    is_prog_clone.set(false);
                });
            }
        });
    }

    // Connect notify::position on ai_paned to dynamically update and persist ai_panel_height
    {
        let paned = ai_paned.clone();
        let ai = ai_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let debounce_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let is_prog_track = Rc::clone(&is_programmatic_ai_pos);

        paned.connect_position_notify(move |p| {
            if !ai.is_visible() || is_prog_track.get() {
                return;
            }
            let total_h = p.height();
            let pos = p.position();
            if total_h <= 200 || pos <= 50 {
                return;
            }
            let min_ai = 200;
            let min_term = 120;
            let max_ai = (total_h - min_term).max(min_ai);
            let calculated_ai_h = (total_h - pos).clamp(min_ai, max_ai) as u32;

            if st_rc.borrow().ai_panel_height != calculated_ai_h {
                st_rc.borrow_mut().ai_panel_height = calculated_ai_h;

                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let st_persist = Rc::clone(&st_rc);
                let timer_holder = Rc::clone(&debounce_timer);
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        timer_holder.borrow_mut().take();
                        let cfg_path = platform::config_dir().join("settings.json");
                        let _ = st_persist.borrow().save_to_path(&cfg_path);
                    },
                );
                *debounce_timer.borrow_mut() = Some(new_id);
            }
        });
    }
    {
        let rt_nt = spawn_rt.clone();
        let counter = Rc::new(RefCell::new(1u32));
        btn_new_tab.connect_clicked(move |_| {
            let mut n = counter.borrow_mut();
            *n += 1;
            let term = spawn_terminal_with_runtime(&rt_nt, None, None, &[]);
            let slot = create_tab_slot_box(&term);
            let page = rt_nt.tab_view.append(&slot);
            page.set_title(&format!("Terminal {}", *n));
            rt_nt.tab_view.set_selected_page(&page);
            term.grab_focus();
        });
    }
    {
        let rt_sp = spawn_rt.clone();
        btn_split.connect_clicked(move |_| {
            let new_term = spawn_terminal_with_runtime(&rt_sp, None, None, &[]);
            if !split_focused_pane(
                &rt_sp.tab_view,
                gtk4::Orientation::Horizontal,
                new_term.clone(),
                &rt_sp.status_lbl,
            ) {
                let slot = create_tab_slot_box(&new_term);
                let page = rt_sp.tab_view.append(&slot);
                page.set_title(&tr("Split Terminal"));
                rt_sp.tab_view.set_selected_page(&page);
            }
        });
    }

    setup_terminal_window_actions(app, &spawn_rt, &active_term_cb);

    // Global Search (`Ctrl+Shift+G`) & Command History (`Ctrl+H`) HeaderBar Buttons
    {
        let win_ref = window.clone();
        let tv = tab_view.clone();
        let cmds_rc = Rc::clone(&cmd_buttons_rc);
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        let sp_sess = Rc::clone(&spawn_session_cb);

        btn_global_search.connect_clicked(move |_| {
            let sp_cb = Rc::clone(&sp_sess);
            let st_sess = status.clone();
            let on_sess: Rc<dyn Fn(&SessionItem)> = Rc::new(move |sess: &SessionItem| {
                sp_cb(sess, false);
                st_sess.set_text(&tr_fmt(
                    "Conectando via Busca Global: {name}",
                    &[("name", &sess.name)],
                ));
            });
            crate::global_search_and_shortcuts::show_global_search_dialog(
                &win_ref,
                &tv,
                &cmds_rc.borrow(),
                Rc::clone(&act_term),
                on_sess,
                &status,
            );
        });
    }
    {
        let win_ref = window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        btn_cmd_history.connect_clicked(move |_| {
            crate::global_search_and_shortcuts::show_command_history_dialog(
                &win_ref,
                Rc::clone(&act_term),
                &status,
            );
        });
    }

    // Phase 16: Unified Command Palette (`Ctrl+Shift+P`)
    {
        let win_ref = window.clone();
        let tv = tab_view.clone();
        let st_rc_pal = Rc::clone(&settings_rc);
        let terms_rc_pal = Rc::clone(&terminals_rc);
        let css_prov_pal = css_provider.clone();
        let cmds_rc = Rc::clone(&cmd_buttons_rc);
        let b_nt = btn_new_tab.clone();
        let b_sp = btn_split.clone();
        let b_sb = btn_sidebar.clone();
        let b_fm = btn_fm.clone();
        let b_ai = btn_ai.clone();
        let b_sr = btn_search.clone();
        let b_gs = btn_global_search.clone();
        let b_ch = btn_cmd_history.clone();
        let b_bc = btn_broadcast_toggle.clone();
        let b_cm = btn_cmd_manager.clone();
        let b_rm = btn_resource_monitor.clone();
        let b_ac = btn_autocomplete.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_palette.connect_clicked(move |_| {
            let win_c = win_ref.clone();
            let tv_c = tv.clone();
            let st_c = Rc::clone(&st_rc_pal);
            let terms_c = Rc::clone(&terms_rc_pal);
            let css_c = css_prov_pal.clone();
            let b_nt_c = b_nt.clone();
            let b_sp_c = b_sp.clone();
            let b_sb_c = b_sb.clone();
            let b_fm_c = b_fm.clone();
            let b_ai_c = b_ai.clone();
            let b_sr_c = b_sr.clone();
            let b_gs_c = b_gs.clone();
            let b_ch_c = b_ch.clone();
            let b_bc_c = b_bc.clone();
            let b_cm_c = b_cm.clone();
            let b_rm_c = b_rm.clone();
            let b_ac_c = b_ac.clone();
            let st_act = status.clone();
            let at_act = Rc::clone(&act_term);

            let on_act: Rc<dyn Fn(&str)> = Rc::new(move |act: &str| match act {
                "global-search" => b_gs_c.emit_clicked(),
                "toggle-search" => b_sr_c.set_active(!b_sr_c.is_active()),
                "show-command-history" => b_ch_c.emit_clicked(),
                "command-manager" => b_cm_c.emit_clicked(),
                "new-tab" => b_nt_c.emit_clicked(),
                "close-tab" => {
                    if let Some(page) = tv_c.selected_page() {
                        if tv_c.n_pages() > 1 {
                            tv_c.close_page(&page);
                        }
                    }
                }
                "next-tab" => {
                    tv_c.select_next_page();
                }
                "previous-tab" => {
                    tv_c.select_previous_page();
                }
                "split" => b_sp_c.emit_clicked(),
                "split-vertical" => {
                    let _ = gtk4::prelude::WidgetExt::activate_action(
                        &win_c,
                        "win.split-vertical",
                        None,
                    );
                }
                "export-terminal-buffer" => {
                    if let Some(term) = at_act() {
                        let tab_title = tv_c.selected_page().map(|p| p.title().to_string());
                        crate::export_dialog::show_export_terminal_dialog(
                            &win_c,
                            &term,
                            tab_title.as_deref(),
                            false,
                            &st_act,
                        );
                    }
                }
                "export-terminal-runbook" => {
                    if let Some(term) = at_act() {
                        let tab_title = tv_c.selected_page().map(|p| p.title().to_string());
                        crate::export_dialog::show_export_terminal_dialog(
                            &win_c,
                            &term,
                            tab_title.as_deref(),
                            true,
                            &st_act,
                        );
                    }
                }
                "clear-session" => {
                    if let Some(term) = at_act() {
                        term.reset(true, true);
                        term.feed_child(b"\x0c");
                        st_act.set_text(&tr("Terminal session cleared."));
                    }
                }
                "toggle-broadcast" => b_bc_c.set_active(!b_bc_c.is_active()),
                "zoom-in" | "zoom-out" | "zoom-reset" => {
                    let mut st = st_c.borrow_mut();
                    let (new_font, _) = match act {
                        "zoom-in" => change_font_size(&st.font, 1),
                        "zoom-out" => change_font_size(&st.font, -1),
                        _ => ("Monospace 11".to_string(), 11),
                    };
                    st.font = new_font;
                    for term in terms_c.borrow().iter() {
                        apply_terminal_appearance(term, &st);
                    }
                }
                "toggle-sidebar" => b_sb_c.set_active(!b_sb_c.is_active()),
                "toggle-fm" => b_fm_c.set_active(!b_fm_c.is_active()),
                "toggle-ai" => b_ai_c.set_active(!b_ai_c.is_active()),
                "resource-monitor" => b_rm_c.emit_clicked(),
                "autocomplete" => b_ac_c.emit_clicked(),
                "system-diagnostics" => {
                    crate::global_search_and_shortcuts::show_system_diagnostics_dialog(
                        &win_c, &st_act,
                    );
                }
                "shortcuts" => {
                    let cfg_path = platform::config_dir().join("settings.json");
                    crate::global_search_and_shortcuts::show_editable_shortcuts_dialog(
                        &win_c, &st_c, &cfg_path, &st_act,
                    );
                }
                "preferences" => {
                    crate::preferences_ui::show_preferences_window(
                        &win_c,
                        Rc::clone(&st_c),
                        Rc::clone(&terms_c),
                        css_c.clone(),
                        &st_act,
                    );
                }
                "colors" | "highlight-settings" => {
                    crate::highlight_dialog_ui::show_highlight_dialog(
                        &win_c,
                        Rc::clone(&st_c),
                        Rc::clone(&terms_c),
                        css_c.clone(),
                        &st_act,
                    );
                }
                "git-assistant" => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| platform::home_dir());
                    if let Some(git_sum) =
                        onyxsh_terminal::git_assistant::inspect_git_repository(&cwd)
                    {
                        let commit_msg =
                            onyxsh_terminal::git_assistant::suggest_conventional_commit(&git_sum);
                        if let Some(term) = at_act() {
                            let cmd = format!("git commit -m \"{commit_msg}\"");
                            term.feed_child(cmd.as_bytes());
                        }
                        st_act.set_text(&format!("Git Assistant: {commit_msg}"));
                    } else {
                        st_act.set_text(&tr(
                            "Git Assistant: diretório atual não é um repositório Git.",
                        ));
                    }
                }
                "backup-config" => {
                    crate::preferences_ui::run_encrypted_backup_flow(&win_c, &st_act);
                }
                "show-tunnel-manager" => {
                    crate::tunnel_ui::show_tunnel_manager_window(&win_c, &st_act);
                }
                "configure-ai" => {
                    crate::ai_config_ui::show_ai_config_window(&win_c, Rc::clone(&st_c), &st_act);
                }
                "show-audit-log" | "audit-log" => {
                    crate::audit_log_dialog::show_audit_log_dialog(&win_c, &st_act);
                }
                "show-scope-dialog" | "agent-scope" => {
                    crate::agent_scope_dialog::show_agent_scope_dialog(&win_c, &st_c);
                }
                _ => {}
            });

            let at_1 = Rc::clone(&act_term);
            let st_1 = status.clone();
            let on_sess: Rc<dyn Fn(&SessionItem)> = Rc::new(move |sess: &SessionItem| {
                if let Some(term) = at_1() {
                    if sess.session_type == "ssh" && !sess.host.is_empty() {
                        let cmd = format!("ssh -p {} {}\n", sess.port, sess.connection_string());
                        term.feed_child(cmd.as_bytes());
                    }
                    st_1.set_text(&tr_fmt(
                        "Conectando via Command Palette: {name}",
                        &[("name", &sess.name)],
                    ));
                }
            });

            let at_2 = Rc::clone(&act_term);
            let st_2 = status.clone();
            let on_cmd: Rc<dyn Fn(&str)> = Rc::new(move |cmd_tpl: &str| {
                if let Some(term) = at_2() {
                    term.feed_child(format!("{cmd_tpl}\n").as_bytes());
                    st_2.set_text(&tr_fmt(
                        "Comando executado via Palette: {cmd}",
                        &[("cmd", cmd_tpl)],
                    ));
                }
            });

            crate::terminal_ux::show_command_palette_dialog(
                &win_ref,
                &cmds_rc.borrow(),
                on_act,
                on_sess,
                on_cmd,
            );
        });
    }

    // Complete 34-Action Window-Level Keyboard Controller (`PropagationPhase::Capture`)
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let app_k = app.clone();
        let win_k = window.clone();
        let tv_k = tab_view.clone();
        let st_k = Rc::clone(&settings_rc);
        let terms_k = Rc::clone(&terminals_rc);
        let css_k = css_provider.clone();
        let bs = btn_search.clone();
        let bgs = btn_global_search.clone();
        let bch = btn_cmd_history.clone();
        let bnt = btn_new_tab.clone();
        let bsb = btn_sidebar.clone();
        let bfm = btn_fm.clone();
        let bai = btn_ai.clone();
        let bbc = btn_broadcast_toggle.clone();
        let bcm = btn_cmd_manager.clone();
        let bp = btn_palette.clone();
        let brm = btn_resource_monitor.clone();
        let act_term_k = Rc::clone(&active_term_cb);
        let status_k = status_lbl.clone();
        let tftp_k = Rc::clone(&active_tftp_server);
        let tftp_lbl_k = tftp_lbl.clone();

        key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
            let is_mod_only = matches!(
                keyval,
                gdk::Key::Control_L
                    | gdk::Key::Control_R
                    | gdk::Key::Shift_L
                    | gdk::Key::Shift_R
                    | gdk::Key::Alt_L
                    | gdk::Key::Alt_R
                    | gdk::Key::Super_L
                    | gdk::Key::Super_R
                    | gdk::Key::ISO_Level3_Shift
            );
            if is_mod_only {
                return glib::Propagation::Proceed;
            }

            let mods = state & gtk4::accelerator_get_default_mod_mask();

            let matches_action = |action_key: &str| -> bool {
                let st = st_k.borrow();
                let accel = match st.shortcuts.get(action_key) {
                    Some(s) if !s.is_empty() => s.as_str(),
                    _ => return false,
                };
                if let Some((target_keyval, target_mods)) = gtk4::accelerator_parse(accel) {
                    return target_keyval.to_lower() == keyval.to_lower() && target_mods == mods;
                }
                false
            };

            // 1. Tab & Window Management
            if matches_action("new-local-tab") {
                bnt.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("close-tab") {
                if let Some(page) = tv_k.selected_page() {
                    if tv_k.n_pages() > 1 {
                        tv_k.close_page(&page);
                        status_k.set_text(&tr("Aba encerrada."));
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_action("next-tab") {
                tv_k.select_next_page();
                return glib::Propagation::Stop;
            }
            if matches_action("previous-tab") {
                tv_k.select_previous_page();
                return glib::Propagation::Stop;
            }
            if matches_action("new-window") {
                let st_clone = st_k.borrow().clone();
                build_main_window(&app_k, st_clone, crate::CliOptions::default());
                return glib::Propagation::Stop;
            }
            if matches_action("quit") {
                app_k.quit();
                return glib::Propagation::Stop;
            }

            // 2. Splitting & Panes
            if matches_action("split-horizontal") {
                let _ = gtk4::prelude::WidgetExt::activate_action(
                    &win_k,
                    "win.split-horizontal",
                    None,
                );
                return glib::Propagation::Stop;
            }
            if matches_action("split-vertical") {
                let _ = gtk4::prelude::WidgetExt::activate_action(
                    &win_k,
                    "win.split-vertical",
                    None,
                );
                return glib::Propagation::Stop;
            }
            if matches_action("close-pane") {
                close_focused_pane(&tv_k, &status_k);
                return glib::Propagation::Stop;
            }
            if matches_action("focus-pane-up") || matches_action("focus-pane-left") {
                if focus_adjacent_pane(&tv_k, false, &status_k) {
                    return glib::Propagation::Stop;
                }
            }
            if matches_action("focus-pane-down") || matches_action("focus-pane-right") {
                if focus_adjacent_pane(&tv_k, true, &status_k) {
                    return glib::Propagation::Stop;
                }
            }

            // 3. Search, Global Search, Command Palette & Command History
            if matches_action("toggle-search") {
                bs.set_active(!bs.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("global-search") {
                bgs.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("command-palette") {
                bp.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("show-command-history") {
                bch.emit_clicked();
                return glib::Propagation::Stop;
            }

            // 4. Panels & Overlays (Sidebar, File Manager, AI Assistant, Broadcast)
            if matches_action("toggle-sidebar") {
                bsb.set_active(!bsb.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-file-manager") {
                bfm.set_active(!bfm.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("ai-assistant") {
                bai.set_active(!bai.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-broadcast") {
                bbc.set_active(!bbc.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("show-command-manager") {
                bcm.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("opencode-launcher") {
                if let Some(term) = act_term_k() {
                    term.feed_child(b"opencode\n");
                    status_k.set_text(&tr("OpenCode iniciado no terminal."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-selection-mode") {
                if let Some(term) = act_term_k() {
                    let mode = !term.is_mouse_autohide();
                    term.set_mouse_autohide(mode);
                    status_k.set_text(&if mode {
                        tr("Modo Seleção de Texto ativado.")
                    } else {
                        tr("Modo Seleção de Texto desativado.")
                    });
                }
                return glib::Propagation::Stop;
            }
            if matches_action("jump-previous-prompt") {
                if let Some(term) = act_term_k() {
                    scroll_to_prompt_row(&term, true);
                    status_k.set_text(&tr("Pular para o prompt anterior."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("jump-next-prompt") {
                if let Some(term) = act_term_k() {
                    scroll_to_prompt_row(&term, false);
                    status_k.set_text(&tr("Pular para o próximo prompt."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("show-tunnel-manager") {
                show_tunnel_manager_dialog(&win_k, &status_k);
                return glib::Propagation::Stop;
            }
            if matches_action("show-resource-monitor") {
                brm.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-tftp-server") {
                let lbl_upd = tftp_lbl_k.clone();
                show_tftp_server_dialog(&win_k, &st_k, &tftp_k, &status_k, move |active_addr| {
                    if let Some(addr) = active_addr {
                        lbl_upd.set_text(&tr_fmt(
                            "Servidor TFTP ({addr})",
                            &[("addr", &addr)],
                        ));
                    } else {
                        lbl_upd.set_text(&tr("Servidor TFTP..."));
                    }
                });
                return glib::Propagation::Stop;
            }

            // 5. Terminal Interaction (Copy, Paste, Select All, Clear)
            if matches_action("copy") {
                if let Some(term) = act_term_k() {
                    term.copy_clipboard_format(vte4::Format::Text);
                    status_k.set_text(&tr("Seleção copiada."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("paste") {
                if let Some(term) = act_term_k() {
                    term.paste_clipboard();
                }
                return glib::Propagation::Stop;
            }
            if matches_action("select-all") {
                if let Some(term) = act_term_k() {
                    term.select_all();
                }
                return glib::Propagation::Stop;
            }
            if matches_action("clear-session") {
                if let Some(term) = act_term_k() {
                    term.reset(true, true);
                    term.feed_child(b"\x0c");
                    status_k.set_text(&tr("Sessão limpa."));
                }
                return glib::Propagation::Stop;
            }

            // 6. Zoom In / Zoom Out / Reset Zoom (`zoom-in`, `zoom-out`, `zoom-reset`)
            if matches_action("zoom-in") {
                let mut st = st_k.borrow_mut();
                let (new_font, new_pt) = change_font_size(&st.font, 1);
                st.font = new_font;
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&format!("Zoom: {new_pt} pt"));
                return glib::Propagation::Stop;
            }
            if matches_action("zoom-out") {
                let mut st = st_k.borrow_mut();
                let (new_font, new_pt) = change_font_size(&st.font, -1);
                st.font = new_font;
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&format!("Zoom: {new_pt} pt"));
                return glib::Propagation::Stop;
            }
            if matches_action("zoom-reset") {
                let mut st = st_k.borrow_mut();
                st.font = "Monospace 11".to_string();
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&tr("Zoom restaurado (11 pt)."));
                return glib::Propagation::Stop;
            }

            // 7. Preferences (`preferences`)
            if matches_action("preferences") {
                crate::preferences_ui::show_preferences_window(
                    &win_k,
                    Rc::clone(&st_k),
                    Rc::clone(&terms_k),
                    css_k.clone(),
                    &status_k,
                );
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);
    }

    // Live `/proc` Telemetry Polling Timer (every 2 seconds)
    let prev_cpu = Rc::new(RefCell::new(
        std::fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| parse_proc_stat(&s))
            .unwrap_or_default(),
    ));
    let metrics_lbl_clone = metrics_lbl.clone();
    glib::timeout_add_seconds_local(2, move || {
        let cpu_pct = if let Ok(stat_str) = std::fs::read_to_string("/proc/stat") {
            if let Some(curr) = parse_proc_stat(&stat_str) {
                let mut prev = prev_cpu.borrow_mut();
                let pct = calculate_cpu_usage_percent(&prev, &curr);
                *prev = curr;
                pct
            } else {
                0.0
            }
        } else {
            0.0
        };

        let mem = std::fs::read_to_string("/proc/meminfo")
            .map(|s| parse_proc_meminfo(&s))
            .unwrap_or_default();
        let net = std::fs::read_to_string("/proc/net/dev")
            .map(|s| parse_proc_net_dev(&s))
            .unwrap_or_default();

        let used_mb = mem.ram_used_bytes / (1024 * 1024);
        let total_mb = mem.ram_total_bytes / (1024 * 1024);
        let rx_mb = net.rx_bytes / (1024 * 1024);
        let tx_mb = net.tx_bytes / (1024 * 1024);

        metrics_lbl_clone.set_text(&format!(
            "CPU: {cpu_pct:.1}% | RAM: {used_mb}/{total_mb} MB ({:.0}%) | NET: ↓{rx_mb}MB ↑{tx_mb}MB",
            mem.ram_percent
        ));
        glib::ControlFlow::Continue
    });

    {
        let st_close = Rc::clone(&settings_rc);
        let tv_close = tab_view.clone();
        let sb_close = sidebar_box.clone();
        let ai_close = ai_widget.clone();
        let win_close = window.clone();
        let tftp_close = Rc::clone(&active_tftp_server);
        window.connect_close_request(move |_| {
            tftp_close.borrow_mut().stop();
            let cfg_dir = platform::config_dir();
            let cfg_path = cfg_dir.join("settings.json");
            let _ = st_close.borrow().save_to_path(&cfg_path);
            let win_state = crate::session_sidebar::capture_window_session_state(
                &tv_close,
                sb_close.is_visible(),
                ai_close.is_visible(),
            );
            let _ = win_state.save_to_path(&cfg_dir.join("state.json"));
            let is_last_window = win_close
                .application()
                .is_none_or(|app| app.windows().len() <= 1);
            if is_last_window {
                crate::tunnel_ui::stop_all_tunnel_processes();
                if st_close.borrow().clear_remote_edit_files_on_exit {
                    crate::quick_look_dialog::cleanup_remote_temp_files();
                }
            }
            glib::Propagation::Proceed
        });
    }

    window.set_content(Some(&root_vbox));
    window.present();
    tab_view
}
