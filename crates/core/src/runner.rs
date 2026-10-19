use crate::model::{CheckResult, CheckStatus};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct Output {
    pub ok: bool,
    pub code: Option<i32>,
    pub text: String,
    /// Só stdout (útil para parsear JSON sem o ruído do stderr).
    pub stdout: String,
    pub spawn_error: bool,
    pub duration_ms: u128,
}

/// Corre um comando de shell (`cmd /C` no Windows, `sh -c` nos restantes).
pub fn run_shell(command: &str, cwd: &Path) -> Output {
    let start = Instant::now();
    #[cfg(windows)]
    let mut cmd = {
        // `cmd /C` é preciso para aceitar npm.cmd/pnpm.cmd; e o escapamento de
        // args do std (com `\"`) é re-interpretado mal pelo cmd — o npm passa a
        // ver `""pkg@5""` e rejeita o nome. `raw_arg` repassa o comando sem alterações.
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        c.raw_arg("/C").raw_arg(command);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    // Ambiente neutro: sem cores do gerenciador poluindo o output parseado
    // (helpful em terminais limitados, CI e Windows desconhecidos).
    cmd.current_dir(cwd)
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .env("DO_NOT_TRACK", "1");
    match cmd.output() {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let mut text = stdout.clone();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            Output {
                ok: o.status.success(),
                code: o.status.code(),
                text,
                stdout,
                spawn_error: false,
                duration_ms: start.elapsed().as_millis(),
            }
        }
        Err(e) => Output {
            ok: false,
            code: None,
            text: e.to_string(),
            stdout: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn quotes_survive_cmd_c() {
        // Regressão do EINVALIDPACKAGENAME: o escapamento do std (") era
        // re-interpretado pelo cmd e o npm recebia ""pkg@5"". As aspas têm de
        // chegar intactas ao comando; com o bug, o node veria "''ok''" e
        // falharia com SyntaxError.
        let o = run_shell("node -p \"'ok'\"", &std::env::temp_dir());
        assert!(o.ok, "o comando falhou (stderr): {}", o.text.trim());
        assert_eq!(o.stdout.trim(), "ok");
    }
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
