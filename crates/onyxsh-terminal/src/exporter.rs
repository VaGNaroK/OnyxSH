//! Terminal Content Exporter (Plain Text, Log, Markdown, Dark HTML, Asciinema v2 `.cast`).
//! Port of `src/onyxsh/terminal/exporter.py`.

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
    format!(
        "================================================================================\n\
         \x20OnyxSH Terminal Output Log\n\
         \x20Session: {} | Host: {} | CWD: {}\n\
         \x20Date: {} | Dimensions: {}x{}\n\
         ================================================================================\n\n\
         {}",
        meta.session_name, meta.host, meta.cwd, meta.timestamp, meta.cols, meta.rows, content
    )
}

/// Exports terminal content as a structured Markdown document with metadata and bash fence.
pub fn export_markdown(raw_text: &str, meta: &ExportMetadata) -> String {
    let content = export_plain_text(raw_text);
    let line_count = content.lines().count();
    format!(
        "# 🖥️ OnyxSH Terminal Export\n\n\
         - **Session:** `{}`\n\
         - **Host:** `{}`\n\
         - **Directory:** `{}`\n\
         - **Date / Time:** `{}`\n\
         - **Dimensions:** `{} x {}`\n\
         - **Total Lines:** `{}`\n\n\
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

/// Exports terminal content as a standalone dark-themed HTML document.
pub fn export_html(raw_text: &str, meta: &ExportMetadata) -> String {
    let escaped = escape_html(&export_plain_text(raw_text));
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"pt-BR\">\n\
         <head>\n\
         <meta charset=\"UTF-8\">\n\
         <title>OnyxSH Terminal Export - {}</title>\n\
         <style>body {{ background: #121216; color: #e5e5ea; font-family: monospace; padding: 24px; }} pre {{ background: #1a1b26; padding: 16px; border-radius: 8px; overflow-x: auto; }}</style>\n\
         </head>\n\
         <body>\n\
         <h2>{} ({}) — {}</h2>\n\
         <pre id=\"terminal-output\">{}</pre>\n\
         </body>\n\
         </html>\n",
        escape_html(&meta.session_name),
        escape_html(&meta.session_name),
        escape_html(&meta.host),
        escape_html(&meta.cwd),
        escaped
    )
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
        assert!(md.contains("# 🖥️ OnyxSH Terminal Export"));
        assert!(md.contains("```bash\n$ uname -a\nLinux onyx 6.12\n```"));

        let html = export_html(raw, &meta);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Linux onyx 6.12"));

        let cast = export_asciinema_v2(raw, &meta);
        assert!(cast.starts_with("{\"env\":"));
        assert!(cast.contains("\"version\":2"));
    }
}
