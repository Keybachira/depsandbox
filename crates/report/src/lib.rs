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

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Página HTML autocontida (sem recursos externos), tema escuro como a TUI.
pub fn to_html(r: &Report) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "<!doctype html><html lang=\"pt\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>depsandbox: {title}</title><style>\
:root{{--bg:#06121f;--panel:#0b2133;--line:#184e70;--text:#d6e2eb;--dim:#688696;--accent:#b8f23a;--ok:#78de60;--err:#ff5868;--warn:#ffd440}}\
@media (prefers-color-scheme: light){{:root{{--bg:#f6f9fb;--panel:#fff;--line:#c5d6e0;--text:#12212c;--dim:#5b7584;--accent:#3d6b00;--ok:#1a7f37;--err:#c62828;--warn:#9a6700}}}}\
body{{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 ui-monospace,Consolas,monospace;padding:24px;max-width:960px;margin:auto}}\
h1{{color:var(--accent);font-size:22px}}h2{{font-size:16px;border-bottom:1px solid var(--line);padding-bottom:4px;margin-top:28px}}\
table{{border-collapse:collapse;width:100%}}td,th{{padding:6px 10px;border-bottom:1px solid var(--line);text-align:left}}\
.ok{{color:var(--ok)}}.err{{color:var(--err)}}.warn{{color:var(--warn)}}.dim{{color:var(--dim)}}\
pre{{background:var(--panel);border:1px solid var(--line);padding:12px;overflow-x:auto}}</style></head><body>",
        title = esc(&r.target.spec())
    );
    let (vclass, vtext) = match r.verdict {
        Verdict::NoIssuesDetected => ("ok", "nenhum problema detectado"),
        Verdict::BreakingChangesDetected => ("err", "breaking changes detectadas"),
        Verdict::BaselineBroken => ("warn", "baseline já falhava (comparação pouco fiável)"),
        Verdict::InstallFailed => ("err", "o upgrade não instalou"),
    };
    let _ = write!(
        s,
        "<h1>Upgrade {}</h1><p>Projeto <b>{}</b> · {:?} · <span class=\"{}\">{}</span></p>",
        esc(&r.target.spec()),
        esc(&r.project),
        r.ecosystem,
        vclass,
        vtext
    );
    let cls = |st: CheckStatus| match st {
        CheckStatus::Passed => ("ok", "PASSED"),
        CheckStatus::Failed | CheckStatus::Errored => ("err", "FAILED"),
        CheckStatus::Flaky => ("warn", "INSTÁVEL"),
        CheckStatus::Skipped => ("dim", "IGNORADO"),
    };
    s.push_str("<h2>Verificações</h2><table><tr><th>Verificação</th><th>Antes</th><th>Depois</th></tr>");
    for b in &r.baseline {
        let (bc, bt) = cls(b.status);
        let after = r.after.iter().find(|a| a.name == b.name).map(|a| cls(a.status));
        let (ac, at) = after.unwrap_or(("dim", "—"));
        let _ = write!(s, "<tr><td>{}</td><td class=\"{bc}\">{bt}</td><td class=\"{ac}\">{at}</td></tr>", esc(&b.name));
    }
    s.push_str("</table><h2>Achados</h2>");
    if r.findings.is_empty() {
        s.push_str("<p class=\"dim\">Nenhum.</p>");
    }
    s.push_str("<ul>");
    for f in &r.findings {
        let c = match f.severity {
            Severity::Error => "err",
            Severity::Warning => "warn",
            Severity::Info => "ok",
        };
        let _ = write!(s, "<li class=\"{c}\">{}</li>", esc(&f.message));
    }
    s.push_str("</ul>");
    for a in r.after.iter().filter(|a| a.status != CheckStatus::Passed) {
        let _ = write!(s, "<h2>Saída de {}</h2><pre>{}</pre>", esc(&a.name), esc(&a.output_tail));
    }
    if !r.steps.is_empty() {
        s.push_str("<h2>Etapas</h2><table>");
        for st in &r.steps {
            let _ = write!(s, "<tr><td>{}</td><td>{:.1}s</td></tr>", esc(&st.name), st.duration_ms as f64 / 1000.0);
        }
        s.push_str("</table>");
    }
    s.push_str("<h2>Manifest</h2><ul class=\"dim\">");
    let _ = write!(s, "<li>depsandbox {} em {}/{}</li>", r.manifest.depsandbox_version, r.manifest.os, r.manifest.arch);
    for (k, v) in &r.manifest.tool_versions {
        let _ = write!(s, "<li>{}: {}</li>", esc(k), esc(v));
    }
    if let Some(sha) = &r.manifest.lockfile_sha {
        let _ = write!(s, "<li>lockfile sha256: {sha}</li>");
    }
    s.push_str("</ul></body></html>");
    s
}
