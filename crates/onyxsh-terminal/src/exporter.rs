//! Terminal Content Exporter (Plain Text, Log, Markdown, Dark HTML, Asciinema v2 `.cast`).
//! Port of `src/onyxsh/terminal/exporter.py`.

use onyxsh_core::i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub session_name: String,
    pub host: String,
    pub cwd: String,
    pub cols: u32,
    pub rows: u32,
    pub timestamp: String,
    pub unix_timestamp: u64,
}

impl Default for ExportMetadata {
    fn default() -> Self {
        Self {
            session_name: "Local Terminal".to_string(),
            host: "localhost".to_string(),
            cwd: "~".to_string(),
            cols: 80,
            rows: 24,
            timestamp: "2026-11-04 12:00:00".to_string(),
            unix_timestamp: 1793793600,
        }
    }
}

/// Escapes HTML special characters (`&`, `<`, `>`, `"`, `'`).
pub fn escape_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Strips trailing blank lines and ensures a single trailing newline.
pub fn export_plain_text(raw_text: &str) -> String {
    let mut lines: Vec<&str> = raw_text.lines().collect();
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else {
            break;
        }
    }
    if lines.is_empty() {
        return "\n".to_string();
    }
    format!("{}\n", lines.join("\n"))
}

/// Exports terminal content as a structured `.log` file with header.
pub fn export_log(raw_text: &str, meta: &ExportMetadata) -> String {
    let content = export_plain_text(raw_text);
    let title = tr("OnyxSH Terminal Output Log");
    let line_meta = tr_fmt(
        "Session: {session} | Host: {host} | CWD: {cwd}",
        &[
            ("session", &meta.session_name),
            ("host", &meta.host),
            ("cwd", &meta.cwd),
        ],
    );
    let line_dims = tr_fmt(
        "Date: {date} | Dimensions: {cols}x{rows}",
        &[
            ("date", &meta.timestamp),
            ("cols", &meta.cols.to_string()),
            ("rows", &meta.rows.to_string()),
        ],
    );
    format!(
        "================================================================================\n\
         \x20{title}\n\
         \x20{line_meta}\n\
         \x20{line_dims}\n\
         ================================================================================\n\n\
         {content}"
    )
}

/// Exports terminal content as a structured Markdown document with metadata and bash fence.
pub fn export_markdown(raw_text: &str, meta: &ExportMetadata) -> String {
    let content = export_plain_text(raw_text);
    let line_count = content.lines().count();
    let heading = tr("# 🖥️ OnyxSH Terminal Export");
    let lbl_session = tr("Session");
    let lbl_host = tr("Host");
    let lbl_dir = tr("Directory");
    let lbl_datetime = tr("Date / Time");
    let lbl_dims = tr("Dimensions");
    let lbl_lines = tr("Total Lines");
    format!(
        "{heading}\n\n\
         - **{lbl_session}:** `{}`\n\
         - **{lbl_host}:** `{}`\n\
         - **{lbl_dir}:** `{}`\n\
         - **{lbl_datetime}:** `{}`\n\
         - **{lbl_dims}:** `{} x {}`\n\
         - **{lbl_lines}:** `{}`\n\n\
         ---\n\n\
         ```bash\n\
         {}\n\
         ```\n",
        meta.session_name,
        meta.host,
        meta.cwd,
        meta.timestamp,
        meta.cols,
        meta.rows,
        line_count,
        content.trim_end_matches('\n')
    )
}

/// Exports terminal content as a standalone dark-themed HTML document with copy button and metadata grid.
pub fn export_html(raw_text: &str, meta: &ExportMetadata) -> String {
    let escaped = escape_html(&export_plain_text(raw_text));
    let session_esc = escape_html(&meta.session_name);
    let host_esc = escape_html(&meta.host);
    let cwd_esc = escape_html(&meta.cwd);
    let date_esc = escape_html(&meta.timestamp);
    let page_title = escape_html(&tr_fmt(
        "Exportação de Terminal OnyxSH - {session}",
        &[("session", &meta.session_name)],
    ));
    let header_title = escape_html(&tr("⚡ Exportação de Terminal OnyxSH"));
    let copy_btn_lbl = escape_html(&tr("📋 Copiar Saída"));
    let copy_btn_js = tr("📋 Copiar Saída").replace('\\', "\\\\").replace('\'', "\\'");
    let copied_js = tr("✅ Copiado!").replace('\\', "\\\\").replace('\'', "\\'");
    let lbl_session = escape_html(&tr("Session"));
    let lbl_host = escape_html(&tr("Host"));
    let lbl_dir = escape_html(&tr("Directory"));
    let lbl_date = escape_html(&tr("Date"));
    let lbl_dims = escape_html(&tr("Dimensions"));

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{page_title}</title>
  <style>
    :root {{
      --bg-color: #121418;
      --card-bg: #1c1f26;
      --border-color: #2e3440;
      --text-color: #e5e9f0;
      --text-dim: #8892b0;
      --accent: #5e81ac;
      --accent-hover: #81a1c1;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      background-color: var(--bg-color);
      color: var(--text-color);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      padding: 24px;
      line-height: 1.5;
    }}
    .container {{ max-width: 1200px; margin: 0 auto; }}
    .header-card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border-color);
      border-radius: 10px;
      padding: 18px 24px;
      margin-bottom: 20px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 12px;
    }}
    .header-title {{ font-size: 1.25rem; font-weight: 600; }}
    .metadata-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 10px;
      width: 100%;
      margin-top: 10px;
      font-size: 0.875rem;
      color: var(--text-dim);
    }}
    .metadata-item span {{ color: var(--text-color); font-weight: 500; }}
    .copy-btn {{
      background-color: var(--accent);
      color: white;
      border: none;
      border-radius: 6px;
      padding: 8px 16px;
      font-size: 0.875rem;
      font-weight: 500;
      cursor: pointer;
    }}
    .copy-btn:hover {{ background-color: var(--accent-hover); }}
    .terminal-container {{
      background-color: #0b0c10;
      border: 1px solid var(--border-color);
      border-radius: 10px;
      overflow: hidden;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }}
    .terminal-header {{
      background-color: #1a1c23;
      padding: 10px 16px;
      display: flex;
      align-items: center;
      gap: 8px;
      border-bottom: 1px solid var(--border-color);
    }}
    .circle {{ width: 12px; height: 12px; border-radius: 50%; display: inline-block; }}
    .circle.red {{ background-color: #ff5f56; }}
    .circle.yellow {{ background-color: #ffbd2e; }}
    .circle.green {{ background-color: #27c93f; }}
    .terminal-title {{ color: var(--text-dim); font-size: 0.8rem; font-family: monospace; margin-left: 8px; }}
    pre.terminal-content {{
      padding: 20px;
      overflow-x: auto;
      font-family: "Cascadia Code", "Fira Code", "JetBrains Mono", monospace;
      font-size: 13.5px;
      line-height: 1.45;
      white-space: pre;
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="header-card">
      <div class="header-title">{header_title}</div>
      <button class="copy-btn" onclick="copyTerminalText()">{copy_btn_lbl}</button>
      <div class="metadata-grid">
        <div class="metadata-item">{lbl_session}: <span>{session_esc}</span></div>
        <div class="metadata-item">{lbl_host}: <span>{host_esc}</span></div>
        <div class="metadata-item">{lbl_dir}: <span>{cwd_esc}</span></div>
        <div class="metadata-item">{lbl_date}: <span>{date_esc}</span></div>
        <div class="metadata-item">{lbl_dims}: <span>{cols} x {rows}</span></div>
      </div>
    </div>
    <div class="terminal-container">
      <div class="terminal-header">
        <span class="circle red"></span>
        <span class="circle yellow"></span>
        <span class="circle green"></span>
        <span class="terminal-title">{host_esc} — {cwd_esc}</span>
      </div>
      <pre id="terminal-output" class="terminal-content">{escaped}</pre>
    </div>
  </div>
  <script>
    function copyTerminalText() {{
      const el = document.getElementById('terminal-output');
      navigator.clipboard.writeText(el.innerText).then(() => {{
        const btn = document.querySelector('.copy-btn');
        btn.innerText = '{copied_js}';
        setTimeout(() => {{ btn.innerText = '{copy_btn_js}'; }}, 2000);
      }});
    }}
  </script>
</body>
</html>
"#,
        cols = meta.cols,
        rows = meta.rows
    )
}

/// Formats direct terminal export content by format ID (`txt`, `log`, `md`, `html`, `cast`)
/// and returns `(formatted_content, extension, mime_type)`.
pub fn format_direct_export(
    raw_text: &str,
    format_id: &str,
    meta: &ExportMetadata,
) -> (String, &'static str, &'static str) {
    match format_id.trim().to_lowercase().trim_start_matches('.') {
        "txt" => (export_plain_text(raw_text), ".txt", "text/plain"),
        "log" => (export_log(raw_text, meta), ".log", "text/plain"),
        "md" | "markdown" => (export_markdown(raw_text, meta), ".md", "text/markdown"),
        "html" => (export_html(raw_text, meta), ".html", "text/html"),
        "cast" | "asciinema" => (export_asciinema_v2(raw_text, meta), ".cast", "application/json"),
        _ => (export_plain_text(raw_text), ".txt", "text/plain"),
    }
}

/// Exports terminal content in Asciinema v2 (`.cast`) JSONL format.
pub fn export_asciinema_v2(raw_text: &str, meta: &ExportMetadata) -> String {
    let header = serde_json::json!({
        "version": 2,
        "width": meta.cols.max(20),
        "height": meta.rows.max(5),
        "timestamp": meta.unix_timestamp,
        "title": format!("{} ({})", meta.session_name, meta.host),
        "env": { "TERM": "xterm-256color", "SHELL": "/bin/bash" }
    });

    let mut out = format!("{}\n", header);
    let mut time_offset = 0.0f64;
    for line in export_plain_text(raw_text).lines() {
        let payload = format!("{line}\r\n");
        let event = serde_json::json!([
            (time_offset * 1000.0).round() / 1000.0,
            "o",
            payload
        ]);
        out.push_str(&event.to_string());
        out.push('\n');
        time_offset += 0.05;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_formats() {
        let meta = ExportMetadata::default();
        let raw = "$ uname -a\nLinux onyx 6.12\n\n\n";

        assert_eq!(export_plain_text(raw), "$ uname -a\nLinux onyx 6.12\n");
        let md = export_markdown(raw, &meta);
        assert!(md.contains(&tr("# 🖥️ OnyxSH Terminal Export")));
        assert!(md.contains("```bash\n$ uname -a\nLinux onyx 6.12\n```"));

        let html = export_html(raw, &meta);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Linux onyx 6.12"));

        let cast = export_asciinema_v2(raw, &meta);
        assert!(cast.starts_with("{\"env\":"));
        assert!(cast.contains("\"version\":2"));
    }
}
