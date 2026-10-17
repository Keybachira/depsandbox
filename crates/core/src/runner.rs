use crate::model::{CheckResult, CheckStatus};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct Output {
    pub ok: bool,
    pub code: Option<i32>,
    pub text: String,
    pub spawn_error: bool,
    pub duration_ms: u128,
}

/// Corre um comando de shell (`cmd /C` no Windows, `sh -c` nos restantes).
pub fn run_shell(command: &str, cwd: &Path) -> Output {
    let start = Instant::now();
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.current_dir(cwd).env("CI", "1");
    match cmd.output() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            Output {
                ok: o.status.success(),
                code: o.status.code(),
                text,
                spawn_error: false,
                duration_ms: start.elapsed().as_millis(),
            }
        }
        Err(e) => Output {
            ok: false,
            code: None,
            text: e.to_string(),
            spawn_error: true,
            duration_ms: start.elapsed().as_millis(),
        },
    }
}

pub fn tail(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

pub fn run_check(name: &str, command: &str, cwd: &Path) -> CheckResult {
    let o = run_shell(command, cwd);
    let status = if o.spawn_error {
        CheckStatus::Errored
    } else if o.ok {
        CheckStatus::Passed
    } else {
        CheckStatus::Failed
    };
    CheckResult {
        name: name.to_string(),
        command: command.to_string(),
        status,
        exit_code: o.code,
        duration_ms: o.duration_ms,
        output_tail: tail(&o.text, 30),
    }
}
