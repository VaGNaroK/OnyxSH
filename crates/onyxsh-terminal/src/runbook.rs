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

    /// Renders the Runbook into structured Markdown documentation.
    pub fn to_markdown(&self) -> String {
        let mut md = format!(
            "# 📘 {}\n\n\
             - **Autor:** {}\n\
             - **Sessão / Host:** {} (`{}`)\n\
             - **Data:** {}\n\
             - **Status:** {}\n\n",
            if self.metadata.title.is_empty() {
                "Procedimento Operacional / Runbook"
            } else {
                &self.metadata.title
            },
            self.metadata.author,
            self.metadata.session_name,
            self.metadata.host,
            self.metadata.date_str,
            self.metadata.status
        );

        if !self.metadata.objective.is_empty() {
            md.push_str(&format!("## 🎯 Objetivo\n{}\n\n", self.metadata.objective));
        }

        md.push_str("## 🛠️ Etapas Executadas\n\n");
        let mut step_num = 1;
        for step in self.steps.iter().filter(|s| s.included) {
            let status_icon = if step.is_success() { "✅" } else { "❌" };
            md.push_str(&format!(
                "### {}. {} `{}`\n\n",
                step_num, status_icon, step.command
            ));
            if !step.annotation.is_empty() {
                md.push_str(&format!("> {}\n\n", step.annotation));
            }
            if !step.output.is_empty() {
                md.push_str(&format!("```text\n{}\n```\n\n", step.output));
            }
            step_num += 1;
        }

        if !self.metadata.conclusion.is_empty() {
            md.push_str(&format!("## 🏁 Conclusão\n{}\n", self.metadata.conclusion));
        }

        md
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
        assert!(md.contains("### 1. ✅ `systemctl status nginx`"));
    }
}
