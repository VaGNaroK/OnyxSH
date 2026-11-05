//! Runbook / Caderno de Bordo Engine for OnyxSH.
//! Port of `src/onyxsh/terminal/runbook.py`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookStep {
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub duration_str: String,
    pub timestamp_str: String,
    pub cwd: String,
    pub annotation: String,
    pub included: bool,
}

impl RunbookStep {
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0) || self.exit_code.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookMetadata {
    pub title: String,
    pub author: String,
    pub objective: String,
    pub general_notes: String,
    pub conclusion: String,
    pub status: String,
    pub session_name: String,
    pub host: String,
    pub date_str: String,
    pub total_commands: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookData {
    pub metadata: RunbookMetadata,
    pub steps: Vec<RunbookStep>,
}

static PROMPT_FALLBACK_RE: OnceLock<Regex> = OnceLock::new();

fn prompt_fallback_regex() -> &'static Regex {
    PROMPT_FALLBACK_RE.get_or_init(|| {
        Regex::new(r"(?m)^(?:\[.*?\]|[\w\.\-]+@[\w\.\-]+:[^\$#\n]*[\$#]|[\w\.\-]+:[^\$#\n]*[\$#]|bash-[0-9\.]+[\$#]|[\$#%>])\s+(.+)$")
            .expect("valid prompt fallback regex")
    })
}

impl RunbookData {
    /// Parses fallback steps from raw terminal scrollback when semantic prompts are unavailable.
    pub fn parse_fallback_steps(raw_text: &str) -> Vec<RunbookStep> {
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let re = prompt_fallback_regex();
        let matches: Vec<_> = re.captures_iter(raw_text).collect();
        if matches.is_empty() {
            return vec![RunbookStep {
                command: "Terminal Output".to_string(),
                output: trimmed.to_string(),
                exit_code: Some(0),
                duration_str: String::new(),
                timestamp_str: String::new(),
                cwd: "~".to_string(),
                annotation: String::new(),
                included: true,
            }];
        }

        let mut steps = Vec::new();
        for (idx, cap) in matches.iter().enumerate() {
            let Some(full_match) = cap.get(0) else {
                continue;
            };
            let cmd_line = cap.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
            let start_out = full_match.end();
            let end_out = matches
                .get(idx + 1)
                .and_then(|next| next.get(0))
                .map(|m| m.start())
                .unwrap_or(raw_text.len());
            let out_segment = raw_text[start_out..end_out].trim();

            steps.push(RunbookStep {
                command: cmd_line.to_string(),
                output: out_segment.to_string(),
                exit_code: Some(0),
                duration_str: String::new(),
                timestamp_str: String::new(),
                cwd: "~".to_string(),
                annotation: String::new(),
                included: true,
            });
        }
        steps
    }

    /// Renders the Runbook into structured Markdown documentation (`render_markdown` in Python).
    pub fn render_markdown(&self, collapse_long_outputs: bool) -> String {
        let max_preview_lines = 15usize;
        let meta = &self.metadata;
        let included_steps: Vec<&RunbookStep> = self.steps.iter().filter(|s| s.included).collect();

        let title = if meta.title.trim().is_empty() {
            "Caderno de Bordo / Runbook"
        } else {
            meta.title.trim()
        };
        let status_badge = if meta.status.trim().is_empty() {
            "Concluído"
        } else {
            meta.status.trim()
        };

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("# 📘 {title}\n"));
        lines.push("### 📌 Informações da Sessão".to_string());
        lines.push(format!("- **Status:** `{status_badge}`"));
        if !meta.author.trim().is_empty() {
            lines.push(format!("- **Operador / Autor:** `{}`", meta.author.trim()));
        }
        lines.push(format!(
            "- **Host:** `{}`",
            if meta.host.is_empty() { "localhost" } else { &meta.host }
        ));
        if !meta.session_name.trim().is_empty() {
            lines.push(format!("- **Sessão:** `{}`", meta.session_name.trim()));
        }
        lines.push(format!("- **Data / Hora:** `{}`", meta.date_str));
        lines.push(format!(
            "- **Total de Etapas Registradas:** `{}`\n",
            included_steps.len()
        ));

        if !meta.objective.trim().is_empty() {
            lines.push("## 🎯 Objetivo / Contexto".to_string());
            lines.push(format!("{}\n", meta.objective.trim()));
        }

        if !meta.general_notes.trim().is_empty() {
            lines.push("## 📝 Observações Preliminares".to_string());
            lines.push(format!("{}\n", meta.general_notes.trim()));
        }

        lines.push("## 🚀 Passo a Passo da Execução".to_string());

        if included_steps.is_empty() {
            lines.push("*Nenhuma etapa selecionada para este documento.*\n".to_string());
        } else {
            for (idx, step) in included_steps.iter().enumerate() {
                let step_num = idx + 1;
                let status_icon = if step.is_success() { "🟢" } else { "🔴" };
                let mut badges = Vec::new();
                if let Some(code) = step.exit_code {
                    badges.push(format!("`exit {code}`"));
                }
                if !step.duration_str.is_empty() {
                    badges.push(format!("`⏱ {}`", step.duration_str));
                }
                if !step.timestamp_str.is_empty() {
                    badges.push(format!("`{}`", step.timestamp_str));
                }

                lines.push(format!("### {step_num}. `{}` {status_icon}", step.command));
                if !badges.is_empty() {
                    lines.push(format!("> {}\n", badges.join(" ")));
                }
                if !step.annotation.trim().is_empty() {
                    lines.push(format!("> 💬 **Anotação:** {}\n", step.annotation.trim()));
                }

                let out_content = step.output.trim();
                if !out_content.is_empty() {
                    let out_lines_count = out_content.lines().count();
                    if collapse_long_outputs && out_lines_count > max_preview_lines {
                        lines.push(format!(
                            "<details>\n<summary>📄 <b>Saída do Comando</b> ({out_lines_count} linhas)</summary>\n"
                        ));
                        lines.push("```bash".to_string());
                        lines.push(out_content.to_string());
                        lines.push("```\n</details>\n".to_string());
                    } else {
                        lines.push("```bash".to_string());
                        lines.push(out_content.to_string());
                        lines.push("```\n".to_string());
                    }
                } else {
                    lines.push("*(Sem saída gerada)*\n".to_string());
                }
            }
        }

        if !meta.conclusion.trim().is_empty() {
            lines.push("## 🏁 Conclusão & Lições Aprendidas".to_string());
            lines.push(format!("{}\n", meta.conclusion.trim()));
        }

        lines.push("---\n*Documento gerado automaticamente pelo OnyxSH Caderno de Bordo.*".to_string());
        format!("{}\n", lines.join("\n"))
    }

    /// Backwards-compatible alias for [`Self::render_markdown`] with `collapse_long_outputs = true`.
    pub fn to_markdown(&self) -> String {
        self.render_markdown(true)
    }

    /// Renders the Runbook as a self-contained dark-mode HTML report with `@media print` support (`render_html` in Python).
    pub fn render_html(&self, collapse_long_outputs: bool) -> String {
        use crate::exporter::escape_html;

        let max_preview_lines = 15usize;
        let meta = &self.metadata;
        let included_steps: Vec<&RunbookStep> = self.steps.iter().filter(|s| s.included).collect();

        let title = escape_html(if meta.title.trim().is_empty() {
            "Caderno de Bordo / Runbook"
        } else {
            meta.title.trim()
        });
        let status_text = escape_html(if meta.status.trim().is_empty() {
            "Concluído"
        } else {
            meta.status.trim()
        });
        let author = escape_html(if meta.author.trim().is_empty() {
            "N/A"
        } else {
            meta.author.trim()
        });
        let host = escape_html(if meta.host.trim().is_empty() {
            "localhost"
        } else {
            meta.host.trim()
        });
        let date_str = escape_html(&meta.date_str);

        let mut steps_html = Vec::new();
        for (idx, step) in included_steps.iter().enumerate() {
            let step_num = idx + 1;
            let cmd_escaped = escape_html(&step.command);
            let status_color = if step.is_success() { "#27c93f" } else { "#ff5f56" };
            let status_tag = step
                .exit_code
                .map(|c| format!("exit {c}"))
                .unwrap_or_else(|| "OK".to_string());
            let duration_badge = if !step.duration_str.is_empty() {
                format!(
                    r#"<span class="badge">⏱ {}</span>"#,
                    escape_html(&step.duration_str)
                )
            } else {
                String::new()
            };
            let time_badge = if !step.timestamp_str.is_empty() {
                format!(
                    r#"<span class="badge">🕒 {}</span>"#,
                    escape_html(&step.timestamp_str)
                )
            } else {
                String::new()
            };

            let annotation_box = if !step.annotation.trim().is_empty() {
                let ann_escaped = escape_html(step.annotation.trim()).replace('\n', "<br>");
                format!(
                    r#"<div class="annotation-box"><strong>💬 Anotação:</strong> {ann_escaped}</div>"#
                )
            } else {
                String::new()
            };

            let out_trimmed = step.output.trim();
            let output_escaped = escape_html(out_trimmed);
            let out_lines_count = if out_trimmed.is_empty() {
                0
            } else {
                out_trimmed.lines().count()
            };

            let out_block = if output_escaped.is_empty() {
                r#"<div class="no-output"><em>(Sem saída gerada)</em></div>"#.to_string()
            } else if collapse_long_outputs && out_lines_count > max_preview_lines {
                format!(
                    r#"<details class="collapsible-output" open>
  <summary>📄 Saída do Comando ({out_lines_count} linhas)</summary>
  <pre class="terminal-box"><code>{output_escaped}</code></pre>
</details>"#
                )
            } else {
                format!(r#"<pre class="terminal-box"><code>{output_escaped}</code></pre>"#)
            };

            steps_html.push(format!(
                r#"<div class="step-card">
  <div class="step-header">
    <span class="step-num">{step_num}</span>
    <span class="step-cmd"><code>{cmd_escaped}</code></span>
    <span class="badge" style="background-color: {status_color}; color: #000; font-weight: bold;">{status_tag}</span>
    {duration_badge}
    {time_badge}
  </div>
  {annotation_box}
  {out_block}
</div>"#
            ));
        }

        let steps_joined = if steps_html.is_empty() {
            "<p><em>Nenhuma etapa selecionada.</em></p>".to_string()
        } else {
            steps_html.join("\n")
        };

        let objective_section = if !meta.objective.trim().is_empty() {
            let obj_escaped = escape_html(meta.objective.trim()).replace('\n', "<br>");
            format!(
                r#"<div class="card section-card"><h2>🎯 Objetivo / Contexto</h2><p>{obj_escaped}</p></div>"#
            )
        } else {
            String::new()
        };

        let notes_section = if !meta.general_notes.trim().is_empty() {
            let notes_escaped = escape_html(meta.general_notes.trim()).replace('\n', "<br>");
            format!(
                r#"<div class="card section-card"><h2>📝 Observações Preliminares</h2><p>{notes_escaped}</p></div>"#
            )
        } else {
            String::new()
        };

        let conclusion_section = if !meta.conclusion.trim().is_empty() {
            let conc_escaped = escape_html(meta.conclusion.trim()).replace('\n', "<br>");
            format!(
                r#"<div class="card section-card"><h2>🏁 Conclusão &amp; Lições Aprendidas</h2><p>{conc_escaped}</p></div>"#
            )
        } else {
            String::new()
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} - OnyxSH</title>
  <style>
    :root {{
      --bg: #111318;
      --card-bg: #1b1e26;
      --border: #2d3342;
      --text: #e6edf3;
      --text-muted: #8b949e;
      --accent: #3584e4;
      --accent-badge: #263852;
      --annotation-bg: #1c2a38;
      --annotation-border: #3584e4;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      background-color: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      padding: 32px 20px;
      line-height: 1.6;
    }}
    .container {{ max-width: 1040px; margin: 0 auto; }}
    .card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 24px;
      margin-bottom: 20px;
      box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    }}
    .header-card {{ display: flex; flex-direction: column; gap: 16px; }}
    .title-row {{ display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px; }}
    h1 {{ font-size: 1.6rem; color: #fff; font-weight: 700; }}
    h2 {{ font-size: 1.25rem; color: #fff; margin-bottom: 12px; font-weight: 600; }}
    .meta-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 12px;
      font-size: 0.9rem;
      border-top: 1px solid var(--border);
      padding-top: 16px;
    }}
    .meta-item {{ color: var(--text-muted); }}
    .meta-item strong {{ color: var(--text); }}
    .badge {{
      display: inline-block;
      padding: 3px 8px;
      border-radius: 6px;
      font-size: 0.78rem;
      background-color: var(--accent-badge);
      color: #90c2ff;
    }}
    .step-card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      margin-bottom: 16px;
      overflow: hidden;
    }}
    .step-header {{
      background-color: #20242e;
      padding: 12px 18px;
      display: flex;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
      border-bottom: 1px solid var(--border);
    }}
    .step-num {{
      font-weight: 700;
      color: var(--accent);
      background-color: rgba(53, 132, 228, 0.15);
      border-radius: 50%;
      width: 26px;
      height: 26px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 0.85rem;
    }}
    .step-cmd code {{ font-family: "Cascadia Code", "Fira Code", monospace; font-size: 0.95rem; color: #79c0ff; }}
    .annotation-box {{
      background-color: var(--annotation-bg);
      border-left: 4px solid var(--annotation-border);
      padding: 12px 16px;
      font-size: 0.9rem;
      color: #d2e7ff;
      border-bottom: 1px solid var(--border);
    }}
    pre.terminal-box {{
      background-color: #0b0d13;
      padding: 16px;
      overflow-x: auto;
      font-family: "Cascadia Code", "Fira Code", monospace;
      font-size: 0.85rem;
      line-height: 1.45;
      color: #c9d1d9;
    }}
    details.collapsible-output summary {{
      background-color: #141720;
      padding: 8px 16px;
      cursor: pointer;
      font-size: 0.85rem;
      color: var(--text-muted);
      border-bottom: 1px solid var(--border);
    }}
    .no-output {{ padding: 12px 16px; font-size: 0.85rem; color: var(--text-muted); }}
    .footer {{
      text-align: center;
      font-size: 0.8rem;
      color: var(--text-muted);
      margin-top: 32px;
      padding-top: 16px;
      border-top: 1px solid var(--border);
    }}
    @media print {{
      body {{ background-color: #fff; color: #000; padding: 0; }}
      .card, .step-card {{ border: 1px solid #ccc; box-shadow: none; background: #fff; color: #000; }}
      .step-header {{ background-color: #f2f2f2; border-bottom: 1px solid #ccc; }}
      pre.terminal-box {{ background-color: #fafafa; color: #000; border: 1px solid #eee; }}
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="card header-card">
      <div class="title-row">
        <h1>📘 {title}</h1>
        <span class="badge" style="background-color: #27c93f; color: #000; font-weight: bold; font-size: 0.85rem;">{status_text}</span>
      </div>
      <div class="meta-grid">
        <div class="meta-item"><strong>Operador:</strong> {author}</div>
        <div class="meta-item"><strong>Host:</strong> {host}</div>
        <div class="meta-item"><strong>Data:</strong> {date_str}</div>
        <div class="meta-item"><strong>Etapas:</strong> {steps_len}</div>
      </div>
    </div>
    {objective_section}
    {notes_section}
    <div class="card">
      <h2>🚀 Passo a Passo da Execução</h2>
      {steps_joined}
    </div>
    {conclusion_section}
    <div class="footer">OnyxSH — Caderno de Bordo &amp; Runbook Operacional</div>
  </div>
</body>
</html>
"#,
            steps_len = included_steps.len()
        )
    }

    /// Renders the Runbook as a structured plain-text log report (`render_log` in Python).
    pub fn render_log(&self) -> String {
        let meta = &self.metadata;
        let included_steps: Vec<&RunbookStep> = self.steps.iter().filter(|s| s.included).collect();

        let title = if meta.title.trim().is_empty() {
            "Caderno de Bordo / Runbook"
        } else {
            meta.title.trim()
        };
        let status = if meta.status.trim().is_empty() {
            "Concluído"
        } else {
            meta.status.trim()
        };

        let mut lines = vec![
            "=".repeat(80),
            format!(" {title} [{status}]"),
            format!(
                " Operador: {} | Host: {} | Data: {}",
                meta.author, meta.host, meta.date_str
            ),
            "=".repeat(80),
            String::new(),
        ];

        if !meta.objective.trim().is_empty() {
            lines.push("[OBJETIVO / CONTEXTO]".to_string());
            lines.push(meta.objective.trim().to_string());
            lines.push(String::new());
        }

        lines.push(format!("[PASSOS EXECUTADOS ({})]", included_steps.len()));
        lines.push("-".repeat(80));

        for (idx, step) in included_steps.iter().enumerate() {
            let step_num = idx + 1;
            let status_str = step
                .exit_code
                .map(|c| format!("exit {c}"))
                .unwrap_or_else(|| "OK".to_string());
            let dur = if step.duration_str.is_empty() {
                String::new()
            } else {
                format!(" ({})", step.duration_str)
            };
            lines.push(format!("#{step_num} $ {} [{status_str}]{dur}", step.command));

            if !step.annotation.trim().is_empty() {
                lines.push(format!("   💬 Nota: {}", step.annotation.trim()));
            }

            if !step.output.trim().is_empty() {
                lines.push("   --- Saída ---".to_string());
                for out_line in step.output.trim().lines() {
                    lines.push(format!("   | {out_line}"));
                }
                lines.push("   -------------\n".to_string());
            } else {
                lines.push("   (Sem saída)\n".to_string());
            }
        }

        if !meta.conclusion.trim().is_empty() {
            lines.push("[CONCLUSÃO & OBSERVAÇÕES]".to_string());
            lines.push(meta.conclusion.trim().to_string());
            lines.push(String::new());
        }

        lines.push("=".repeat(80));
        format!("{}\n", lines.join("\n"))
    }

    /// Formats the Runbook by format ID (`runbook_md`, `runbook_html`, `runbook_log`)
    /// and returns `(content, extension, mime_type)`.
    pub fn format_runbook(
        &self,
        format_id: &str,
        collapse_long_outputs: bool,
    ) -> (String, &'static str, &'static str) {
        match format_id.trim().to_lowercase().as_str() {
            "runbook_html" => (self.render_html(collapse_long_outputs), ".html", "text/html"),
            "runbook_log" => (self.render_log(), ".log", "text/plain"),
            _ => (
                self.render_markdown(collapse_long_outputs),
                ".md",
                "text/markdown",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runbook_fallback_parsing_and_markdown_export() {
        let raw = "vagnarok@onyx:~$ systemctl status nginx\n● nginx.service - active (running)\nvagnarok@onyx:~$ uptime\n12:00:00 up 5 days";
        let steps = RunbookData::parse_fallback_steps(raw);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].command, "systemctl status nginx");
        assert!(steps[0].output.contains("active (running)"));
        assert_eq!(steps[1].command, "uptime");

        let rb = RunbookData {
            metadata: RunbookMetadata {
                title: "Manutenção Nginx".to_string(),
                author: "vagnarok".to_string(),
                session_name: "Prod Web".to_string(),
                host: "10.0.0.10".to_string(),
                status: "Concluído".to_string(),
                ..Default::default()
            },
            steps,
        };
        let md = rb.to_markdown();
        assert!(md.contains("# 📘 Manutenção Nginx"));
        assert!(md.contains("### 1. `systemctl status nginx` 🟢"));

        let html = rb.render_html(true);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Manutenção Nginx"));
        assert!(html.contains("systemctl status nginx"));

        let log = rb.render_log();
        assert!(log.contains("Manutenção Nginx [Concluído]"));
        assert!(log.contains("#1 $ systemctl status nginx [exit 0]"));
    }
}
