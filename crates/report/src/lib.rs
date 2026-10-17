//! Renderização do relatório. Cada formato é uma vista sobre `Report`.
use anyhow::Result;
use depsandbox_core::*;
use std::fmt::Write;

pub fn to_json(r: &Report) -> Result<String> {
    Ok(serde_json::to_string_pretty(r)?)
}

pub fn to_markdown(r: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Relatório de upgrade: `{}`\n", r.target.spec());
    let _ = writeln!(s, "- **Projeto:** {}", r.project);
    let _ = writeln!(s, "- **Ecossistema:** {:?}", r.ecosystem);
    let _ = writeln!(s, "- **Veredito:** {}\n", verdict_text(r.verdict));

    if r.verdict == Verdict::NoIssuesDetected {
        let _ = writeln!(
            s,
            "> \"Passou\" significa apenas que nenhuma verificação detectou problema. \
             Cobertura de testes fraca = confiança fraca.\n"
        );
    }

    let _ = writeln!(s, "## Verificações\n");
    let _ = writeln!(s, "| Verificação | Antes | Depois |");
    let _ = writeln!(s, "|---|---|---|");
    for b in &r.baseline {
        let after = r.after.iter().find(|a| a.name == b.name);
        let _ = writeln!(
            s,
            "| {} | {} | {} |",
            b.name,
            icon(b.status),
            after.map_or("—".into(), |a| icon(a.status))
        );
    }
    if r.baseline.is_empty() {
        for a in &r.after {
            let _ = writeln!(s, "| {} | — | {} |", a.name, icon(a.status));
        }
    }

    let _ = writeln!(s, "\n## Achados\n");
    if r.findings.is_empty() {
        let _ = writeln!(s, "Nenhum.");
    }
    let mut findings = r.findings.clone();
    findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    for f in &findings {
        let loc = match (&f.file, f.line) {
            (Some(file), Some(l)) => format!(" (`{file}:{l}`)"),
            (Some(file), None) => format!(" (`{file}`)"),
            _ => String::new(),
        };
        let _ = writeln!(s, "- **{:?}**: {}{}", f.severity, f.message, loc);
    }

    for a in r.after.iter().filter(|a| a.status != CheckStatus::Passed) {
        let _ = writeln!(s, "\n### Saída de `{}` (`{}`)\n\n```\n{}\n```", a.name, a.command, a.output_tail);
    }

    let _ = writeln!(s, "\n## Manifest (reprodutibilidade)\n");
    let _ = writeln!(s, "- depsandbox {} em {}/{}", r.manifest.depsandbox_version, r.manifest.os, r.manifest.arch);
    for (k, v) in &r.manifest.tool_versions {
        let _ = writeln!(s, "- {k}: {v}");
    }
    if let Some(sha) = &r.manifest.lockfile_sha {
        let _ = writeln!(s, "- lockfile sha256: `{sha}`");
    }
    s
}

fn icon(s: CheckStatus) -> String {
    match s {
        CheckStatus::Passed => "✔ passou",
        CheckStatus::Failed => "✖ falhou",
        CheckStatus::Errored => "✖ erro",
        CheckStatus::Flaky => "⚠ instável",
        CheckStatus::Skipped => "– ignorado",
    }
    .to_string()
}

fn verdict_text(v: Verdict) -> &'static str {
    match v {
        Verdict::NoIssuesDetected => "✔ nenhum problema detectado",
        Verdict::BreakingChangesDetected => "✖ breaking changes detectadas",
        Verdict::BaselineBroken => "⚠ baseline já falhava (comparação pouco fiável)",
        Verdict::InstallFailed => "✖ o upgrade não instalou",
    }
}
