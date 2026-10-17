use crate::adapter::{Adapter, CheckSpec};
use crate::config::Config;
use crate::model::*;
use crate::runner::{run_check, run_shell, tail};
use crate::sandbox;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Instant;

/// Que verificações correr.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub tests: bool,
    pub build: bool,
    pub typecheck: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { tests: true, build: true, typecheck: true }
    }
}

/// Anuncia cada etapa a `progress` e regista o tempo gasto nela.
struct Stepper<'a> {
    progress: &'a dyn Fn(&str),
    cur: Option<(String, Instant)>,
    done: Vec<StepTiming>,
}

impl<'a> Stepper<'a> {
    fn new(progress: &'a dyn Fn(&str)) -> Self {
        Self { progress, cur: None, done: vec![] }
    }
    fn begin(&mut self, name: &str) {
        self.finish();
        (self.progress)(name);
        self.cur = Some((name.to_string(), Instant::now()));
    }
    fn finish(&mut self) {
        if let Some((name, t)) = self.cur.take() {
            self.done.push(StepTiming { name, duration_ms: t.elapsed().as_millis() });
        }
    }
    fn into_steps(mut self) -> Vec<StepTiming> {
        self.finish();
        self.done
    }
}

/// Fluxo central: snapshot -> baseline (2x) -> upgrade -> checks -> comparação.
pub fn try_upgrade(
    adapter: &dyn Adapter,
    project: &Path,
    target: &UpgradeTarget,
    cfg: &Config,
    opts: &RunOptions,
    progress: &dyn Fn(&str),
) -> Result<Report> {
    target.validate()?;
    let project_name = project
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "projeto".into());

    let mut report = Report {
        schema_version: SCHEMA_VERSION,
        project: project_name,
        ecosystem: adapter.ecosystem(),
        target: target.clone(),
        manifest: manifest(adapter, project),
        baseline: vec![],
        after: vec![],
        findings: vec![],
        verdict: Verdict::NoIssuesDetected,
        steps: vec![],
    };
    let mut st = Stepper::new(progress);

    st.begin("Criando ambiente isolado");
    let sb = sandbox::snapshot(project)?;
    let dir = sb.path();

    st.begin("Instalando dependências atuais (baseline)");
    let inst = run_shell(&adapter.install_cmd(dir), dir);
    if !inst.ok {
        report.findings.push(Finding {
            kind: FindingKind::InstallFailure,
            severity: Severity::Error,
            check: Some("install (baseline)".into()),
            message: "A instalação do baseline falhou; o projeto não instala sem o upgrade.".into(),
            file: None,
            line: None,
        });
        report.baseline.push(failed_install("install (baseline)", &adapter.install_cmd(dir), &inst.text, inst.code));
        report.verdict = Verdict::BaselineBroken;
        report.steps = st.into_steps();
        return Ok(report);
    }

    let mut checks = apply_overrides(adapter.checks(dir)?, cfg);
    checks.retain(|c| match c.name.as_str() {
        "test" => opts.tests,
        "build" => opts.build,
        "typecheck" => opts.typecheck,
        _ => true,
    });
    if checks.is_empty() {
        report.findings.push(Finding {
            kind: FindingKind::PreExistingFailure,
            severity: Severity::Warning,
            check: None,
            message: "Nenhuma verificação ativa (sem scripts de build/test, ou todas desligadas).".into(),
            file: None,
            line: None,
        });
    }

    st.begin("Executando baseline (2 execuções)");
    let run1: Vec<_> = checks.iter().map(|c| run_check(&c.name, &c.command, dir)).collect();
    let run2: Vec<_> = checks.iter().map(|c| run_check(&c.name, &c.command, dir)).collect();
    report.baseline = run1.into_iter().zip(run2).map(|(a, b)| merge_runs(a, b)).collect();

    st.begin(&format!("Aplicando upgrade {}", target.spec()));
    let up_cmd = adapter.upgrade_cmd(target);
    let up = run_shell(&up_cmd, dir);
    if !up.ok {
        report.findings.push(Finding {
            kind: FindingKind::InstallFailure,
            severity: Severity::Error,
            check: Some("upgrade".into()),
            message: format!("O upgrade não instalou: {}", first_error_line(&up.text)),
            file: None,
            line: None,
        });
        report.after.push(failed_install("upgrade", &up_cmd, &up.text, up.code));
        report.verdict = Verdict::InstallFailed;
        report.steps = st.into_steps();
        return Ok(report);
    }

    st.begin("Verificando após o upgrade");
    report.after = checks.iter().map(|c| run_check(&c.name, &c.command, dir)).collect();

    st.begin("Analisando resultados");
    report.findings.extend(compare(&report.baseline, &report.after));
    report.verdict = verdict(&report);
    report.steps = st.into_steps();
    Ok(report)
}

fn apply_overrides(mut checks: Vec<CheckSpec>, cfg: &Config) -> Vec<CheckSpec> {
    for (name, cmd) in [
        ("typecheck", &cfg.checks.typecheck),
        ("build", &cfg.checks.build),
        ("test", &cfg.checks.test),
    ] {
        if let Some(cmd) = cmd {
            match checks.iter_mut().find(|c| c.name == name) {
                Some(c) => c.command = cmd.clone(),
                None => checks.push(CheckSpec { name: name.into(), command: cmd.clone() }),
            }
        }
    }
    checks
}

fn merge_runs(a: CheckResult, b: CheckResult) -> CheckResult {
    use CheckStatus::*;
    let status = match (a.status, b.status) {
        (Passed, Passed) => Passed,
        (Passed, _) | (_, Passed) => Flaky,
        (x, _) => x,
    };
    // Mantém o output da execução que falhou, se houver.
    let (mut keep, other) = if a.status == Passed { (b, a) } else { (a, b) };
    keep.status = status;
    keep.duration_ms = (keep.duration_ms + other.duration_ms) / 2;
    keep
}

fn compare(baseline: &[CheckResult], after: &[CheckResult]) -> Vec<Finding> {
    use CheckStatus::*;
    let mut out = vec![];
    for b in baseline {
        let Some(a) = after.iter().find(|a| a.name == b.name) else { continue };
        let (kind, severity, msg) = match (b.status, a.status) {
            (Flaky, _) => (
                FindingKind::Flaky,
                Severity::Warning,
                format!("'{}' é instável já no baseline; resultado inconclusivo.", b.name),
            ),
            (Passed, Failed | Errored) => (
                FindingKind::Regression,
                Severity::Error,
                format!("'{}' passava antes do upgrade e falha depois.", b.name),
            ),
            (Failed | Errored, Passed) => (
                FindingKind::Fixed,
                Severity::Info,
                format!("'{}' falhava antes e passa depois do upgrade.", b.name),
            ),
            (Failed | Errored, Failed | Errored) => (
                FindingKind::PreExistingFailure,
                Severity::Warning,
                format!("'{}' já falhava no baseline; não é possível atribuir ao upgrade.", b.name),
            ),
            _ => continue,
        };
        out.push(Finding { kind, severity, check: Some(b.name.clone()), message: msg, file: None, line: None });
    }
    out
}

fn verdict(r: &Report) -> Verdict {
    if r.findings.iter().any(|f| f.kind == FindingKind::Regression) {
        Verdict::BreakingChangesDetected
    } else if r.baseline.iter().any(|c| matches!(c.status, CheckStatus::Failed | CheckStatus::Errored)) {
        Verdict::BaselineBroken
    } else {
        Verdict::NoIssuesDetected
    }
}

fn failed_install(name: &str, cmd: &str, text: &str, code: Option<i32>) -> CheckResult {
    CheckResult {
        name: name.into(),
        command: cmd.into(),
        status: CheckStatus::Failed,
        exit_code: code,
        duration_ms: 0,
        output_tail: tail(text, 30),
    }
}

fn first_error_line(text: &str) -> String {
    text.lines()
        .find(|l| l.to_lowercase().contains("err"))
        .or_else(|| text.lines().next())
        .unwrap_or("sem detalhes")
        .trim()
        .to_string()
}

fn manifest(adapter: &dyn Adapter, project: &Path) -> Manifest {
    let tool_versions = adapter
        .tool_versions_cmds()
        .into_iter()
        .map(|(name, cmd)| {
            let o = run_shell(cmd, project);
            let v = o.text.lines().next().unwrap_or("?").trim().to_string();
            (name.to_string(), v)
        })
        .collect();
    let mut hasher = Sha256::new();
    let mut found = false;
    for lf in adapter.lockfiles() {
        if let Ok(bytes) = std::fs::read(project.join(lf)) {
            hasher.update(&bytes);
            found = true;
        }
    }
    let lockfile_sha = found.then(|| hasher.finalize().iter().map(|b| format!("{b:02x}")).collect());
    Manifest {
        depsandbox_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        tool_versions,
        lockfile_sha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(name: &str, status: CheckStatus) -> CheckResult {
        CheckResult {
            name: name.into(),
            command: String::new(),
            status,
            exit_code: None,
            duration_ms: 0,
            output_tail: String::new(),
        }
    }

    #[test]
    fn flaky_when_runs_differ() {
        let m = merge_runs(res("t", CheckStatus::Passed), res("t", CheckStatus::Failed));
        assert_eq!(m.status, CheckStatus::Flaky);
        let m = merge_runs(res("t", CheckStatus::Passed), res("t", CheckStatus::Passed));
        assert_eq!(m.status, CheckStatus::Passed);
    }

    #[test]
    fn regression_vs_preexisting() {
        let base = [res("a", CheckStatus::Passed), res("b", CheckStatus::Failed), res("c", CheckStatus::Flaky)];
        let after = [res("a", CheckStatus::Failed), res("b", CheckStatus::Failed), res("c", CheckStatus::Failed)];
        let f = compare(&base, &after);
        assert_eq!(f[0].kind, FindingKind::Regression);
        assert_eq!(f[1].kind, FindingKind::PreExistingFailure);
        assert_eq!(f[2].kind, FindingKind::Flaky);
    }
}
