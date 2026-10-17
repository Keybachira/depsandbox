//! Schema do relatório. Tudo o resto (md, html, sarif, MCP) é uma "vista" sobre isto.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    Pip,
    Cargo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    /// O comando nem chegou a correr (ex.: binário em falta).
    Errored,
    /// Passou numa execução e falhou noutra.
    Flaky,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub name: String,
    pub command: String,
    pub status: CheckStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    /// Últimas linhas de stdout+stderr (truncado).
    pub output_tail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// Passava antes do upgrade e falha depois.
    Regression,
    /// Instável já no baseline; não é culpa do upgrade.
    Flaky,
    /// Falhava antes e passa depois.
    Fixed,
    /// Falhava antes e continua a falhar.
    PreExistingFailure,
    /// A instalação do upgrade falhou (peers, resolução, rede...).
    InstallFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub check: Option<String>,
    pub message: String,
    /// Reservado para a análise de uso no código (fase seguinte).
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// Uma dependência declarada no projeto.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Dependency {
    pub name: String,
    /// Especificador declarado (p.ex. `^4.1.2`).
    pub current: String,
    /// Versão mais recente, quando conhecida e diferente da atual.
    pub latest: Option<String>,
    pub dev: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpgradeTarget {
    pub package: String,
    /// `None` = `latest`.
    pub version: Option<String>,
}

impl UpgradeTarget {
    /// Aceita `lodash`, `lodash@5`, `@scope/pkg@1.2.3`.
    pub fn parse(spec: &str) -> Self {
        match spec.rfind('@') {
            Some(i) if i > 0 => Self {
                package: spec[..i].to_string(),
                version: Some(spec[i + 1..].to_string()),
            },
            _ => Self { package: spec.to_string(), version: None },
        }
    }

    pub fn spec(&self) -> String {
        format!("{}@{}", self.package, self.version.as_deref().unwrap_or("latest"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Nenhuma verificação detectou problema (não prova ausência de breaking changes).
    NoIssuesDetected,
    BreakingChangesDetected,
    /// O baseline já falhava: comparação pouco fiável.
    BaselineBroken,
    InstallFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub depsandbox_version: String,
    pub os: String,
    pub arch: String,
    pub tool_versions: Vec<(String, String)>,
    pub lockfile_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Report {
    pub schema_version: u32,
    pub project: String,
    pub ecosystem: Ecosystem,
    pub target: UpgradeTarget,
    pub manifest: Manifest,
    pub baseline: Vec<CheckResult>,
    pub after: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub verdict: Verdict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target() {
        let t = UpgradeTarget::parse("lodash");
        assert_eq!((t.package.as_str(), t.version), ("lodash", None));
        let t = UpgradeTarget::parse("lodash@5");
        assert_eq!((t.package.as_str(), t.version.as_deref()), ("lodash", Some("5")));
        let t = UpgradeTarget::parse("@scope/pkg@1.2.3");
        assert_eq!((t.package.as_str(), t.version.as_deref()), ("@scope/pkg", Some("1.2.3")));
        let t = UpgradeTarget::parse("@scope/pkg");
        assert_eq!((t.package.as_str(), t.version), ("@scope/pkg", None));
        assert_eq!(UpgradeTarget::parse("lodash").spec(), "lodash@latest");
    }
}
