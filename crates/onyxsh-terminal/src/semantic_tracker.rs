//! Semantic Prompt Tracker (OSC 133 A/B/C/D) for OnyxSH.
//! Port of `src/onyxsh/terminal/semantic_tracker.py`.

use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticCommand {
    pub command_id: String,
    pub command_text: String,
    pub prompt_row: i64,
    pub command_row: i64,
    pub output_start_row: i64,
    pub output_end_row: Option<i64>,
    pub duration_secs: Option<f64>,
    pub exit_code: Option<i32>,
    pub cwd: String,
}

impl SemanticCommand {
    pub fn is_finished(&self) -> bool {
        self.exit_code.is_some()
    }

    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }

    pub fn formatted_duration(&self) -> String {
        let Some(d) = self.duration_secs else {
            return String::new();
        };
        if d < 1.0 {
            return format!("{}ms", (d * 1000.0) as u64);
        }
        if d < 60.0 {
            return format!("{d:.1}s");
        }
        let mins = (d / 60.0).floor() as u64;
        let secs = (d % 60.0).round() as u64;
        format!("{mins}m {secs}s")
    }
}

#[derive(Debug, Default)]
pub struct SemanticTerminalState {
    pub commands: VecDeque<SemanticCommand>,
    pub current_command: Option<SemanticCommand>,
    pub current_prompt_row: i64,
    pub current_command_row: i64,
    pub prompt_rows: VecDeque<i64>,
    cmd_counter: u64,
}

impl SemanticTerminalState {
    pub fn new() -> Self {
        Self::default()
    }

    /// OSC 133;A — Prompt start.
    pub fn start_prompt(&mut self, row: i64) {
        self.current_prompt_row = row;
        if self.prompt_rows.back().copied() != Some(row) {
            self.prompt_rows.push_back(row);
            if self.prompt_rows.len() > 200 {
                self.prompt_rows.pop_front();
            }
        }
    }

    /// OSC 133;B — Command input start.
    pub fn start_command(&mut self, row: i64) {
        self.current_command_row = row;
    }

    /// OSC 133;C — Command execution / output start.
    pub fn execute_command(&mut self, row: i64, command_text: &str, cwd: &str) {
        self.cmd_counter += 1;
        let command_row = if self.current_command_row != 0 {
            self.current_command_row
        } else {
            self.current_prompt_row
        };
        self.current_command = Some(SemanticCommand {
            command_id: format!("cmd_{}", self.cmd_counter),
            command_text: command_text.trim().to_string(),
            prompt_row: self.current_prompt_row,
            command_row,
            output_start_row: row,
            output_end_row: None,
            duration_secs: None,
            exit_code: None,
            cwd: cwd.to_string(),
        });
    }

    /// OSC 133;D;exit_code — Command finished.
    pub fn finish_command(
        &mut self,
        row: i64,
        exit_code: i32,
        duration_secs: f64,
    ) -> SemanticCommand {
        let mut cmd = self.current_command.take().unwrap_or_else(|| {
            self.cmd_counter += 1;
            SemanticCommand {
                command_id: format!("cmd_{}", self.cmd_counter),
                command_text: String::new(),
                prompt_row: self.current_prompt_row,
                command_row: self.current_command_row.max(self.current_prompt_row),
                output_start_row: self.current_prompt_row,
                output_end_row: None,
                duration_secs: None,
                exit_code: None,
                cwd: String::new(),
            }
        });
        cmd.output_end_row = Some(row);
        cmd.exit_code = Some(exit_code);
        cmd.duration_secs = Some(duration_secs.max(0.0));
        self.commands.push_back(cmd.clone());
        if self.commands.len() > 100 {
            self.commands.pop_front();
        }
        cmd
    }

    /// Returns the previous prompt row strictly before `current_row`.
    pub fn previous_prompt_row(&self, current_row: i64) -> Option<i64> {
        self.prompt_rows
            .iter()
            .rev()
            .copied()
            .find(|&r| r < current_row)
    }

    /// Returns the next prompt row strictly after `current_row`.
    pub fn next_prompt_row(&self, current_row: i64) -> Option<i64> {
        self.prompt_rows.iter().copied().find(|&r| r > current_row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc133_lifecycle_and_navigation() {
        let mut state = SemanticTerminalState::new();
        state.start_prompt(10);
        state.start_command(10);
        state.execute_command(11, "cargo test", "/home/vagnarok/OnyxSH");
        let finished = state.finish_command(25, 0, 1.42);

        assert!(finished.is_finished());
        assert!(finished.is_success());
        assert_eq!(finished.formatted_duration(), "1.4s");

        state.start_prompt(26);
        state.start_prompt(50);

        assert_eq!(state.previous_prompt_row(50), Some(26));
        assert_eq!(state.previous_prompt_row(26), Some(10));
        assert_eq!(state.next_prompt_row(10), Some(26));
    }
}
