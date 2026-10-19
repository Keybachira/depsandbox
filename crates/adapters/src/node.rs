//! Um adapter para cada gerenciador do ecossistema Node (npm, pnpm e yarn),
//! por cima da mesma lógica: o formato do `package.json` e do registo é comum.
use anyhow::{bail, Result};
use depsandbox_core::runner::{run_shell, tail};
use depsandbox_core::{Adapter, CheckSpec, Dependency, Ecosystem, UpgradeTarget};
use std::collections::HashMap;
use std::path::Path;

/// Gerenciadores suportados neste módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pm {
    Npm,
    Pnpm,
    Yarn,
}

impl Pm {
    pub fn name(self) -> &'static str {
        match self {
            Pm::Npm => "npm",
            Pm::Pnpm => "pnpm",
            Pm::Yarn => "yarn",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeAdapter {
    pub kind: Pm,
}

pub const NPM: NodeAdapter = NodeAdapter { kind: Pm::Npm };
pub const PNPM: NodeAdapter = NodeAdapter { kind: Pm::Pnpm };
pub const YARN: NodeAdapter = NodeAdapter { kind: Pm::Yarn };

impl NodeAdapter {
    fn cmd(self, command: &str) -> String {
        format!("{} {command}", self.kind.name())
    }

    fn read_pkg(dir: &Path) -> Result<serde_json::Value> {
        let raw = std::fs::read_to_string(dir.join("package.json"))?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// O projeto casa com ESTE gerenciador (sem considerar o `package.json` como
    /// prova de npm — isso é o fallback do npm).
    fn detect_inner(self, dir: &Path) -> bool {
        let package_manager = Self::read_pkg(dir)
            .ok()
            .and_then(|pkg| pkg.get("packageManager").and_then(|v| v.as_str()).map(String::from))
            .unwrap_or_default();
        let pm = |p: &str| package_manager.starts_with(&format!("{p}@"));
        match self.kind {
            Pm::Npm => true,
            Pm::Pnpm => {
                dir.join("pnpm-lock.yaml").is_file()
                    || dir.join("pnpm-workspace.yaml").is_file()
                    || pm("pnpm")
            }
            Pm::Yarn => {
                dir.join("yarn.lock").is_file()
                    || dir.join(".yarnrc.yml").is_file()
                    || dir.join(".yarn").is_dir()
                    || pm("yarn")
            }
        }
    }
}

impl Adapter for NodeAdapter {
    fn ecosystem(&self) -> Ecosystem {
        match self.kind {
            Pm::Npm => Ecosystem::Npm,
            Pm::Pnpm => Ecosystem::Pnpm,
            Pm::Yarn => Ecosystem::Yarn,
        }
    }

    /// Detecta com exclusividade: `package.json` presente e o gerenciador dono
    /// do projeto. Nenhum projeto casa com dois adapters (critical para o CLI/TUI).
    fn detect(&self, dir: &Path) -> bool {
        if Self::read_pkg(dir).is_err() {
            return false;
        }
        match self.kind {
            // npm é o fallback: só se ninguém mais reclamar do projeto.
            Pm::Npm => !YARN.detect_inner(dir) && !PNPM.detect_inner(dir),
            _ => self.detect_inner(dir),
        }
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        match self.kind {
            Pm::Npm => &["package-lock.json", "npm-shrinkwrap.json"],
            Pm::Pnpm => &["pnpm-lock.yaml"],
            Pm::Yarn => &["yarn.lock"],
        }
    }

    fn install_cmd(&self, dir: &Path) -> String {
        let has_lock = self.lockfiles().iter().any(|f| dir.join(f).is_file());
        match self.kind {
            Pm::Npm => {
                if has_lock {
                    "npm ci --no-audit --no-fund".into()
                } else {
                    "npm install --no-audit --no-fund".into()
                }
            }
            Pm::Pnpm => {
                if has_lock {
                    "pnpm install --frozen-lockfile --no-audit".into()
                } else {
                    "pnpm install --no-audit".into()
                }
            }
            Pm::Yarn => {
                let berry = dir.join(".yarnrc.yml").is_file() || dir.join(".yarn").is_dir();
                if !has_lock {
                    return "yarn install".into();
                }
                if berry {
                    "yarn install --immutable".into()
                } else {
                    "yarn install --frozen-lockfile".into()
                }
            }
        }
    }

    fn upgrade_cmd(&self, target: &UpgradeTarget) -> String {
        let spec = target.spec();
        match self.kind {
            Pm::Npm => format!("npm install \"{spec}\" --no-audit --no-fund"),
            Pm::Pnpm => format!("pnpm add \"{spec}\" --no-audit"),
            Pm::Yarn => format!("yarn add \"{spec}\" --exact"),
        }
    }

    fn checks(&self, dir: &Path) -> Result<Vec<CheckSpec>> {
        let pkg = Self::read_pkg(dir)?;
        let scripts = pkg.get("scripts").and_then(|s| s.as_object());
        let has = |n: &str| scripts.is_some_and(|s| s.contains_key(n));

        let mut out = vec![];
        // Sem script `typecheck`, mas com tsconfig e tsc instalado: usa `tsc --noEmit`.
        if !has("typecheck") && dir.join("tsconfig.json").is_file() && has_tsc(dir) {
            out.push(CheckSpec { name: "typecheck".into(), command: "npx --no-install tsc --noEmit".into() });
        }
        // Ordem: tipos -> build -> testes (falha rápida e mais barata primeiro).
        for name in ["typecheck", "build"] {
            // Rodando com o gerenciador do projeto, respeita workspaces e rc-files.
            if has(name) {
                out.push(CheckSpec { name: name.into(), command: self.cmd(&format!("run {name}")) });
            }
        }
        if has("test") && !is_placeholder_test(scripts.and_then(|s| s["test"].as_str())) {
            out.push(CheckSpec { name: "test".into(), command: self.cmd("test") });
        }
        Ok(out)
    }

    fn dependencies(&self, dir: &Path) -> Result<Vec<Dependency>> {
        let pkg = Self::read_pkg(dir)?;
        let mut out = vec![];
        for (key, dev) in [("dependencies", false), ("devDependencies", true)] {
            if let Some(map) = pkg.get(key).and_then(|v| v.as_object()) {
                for (name, spec) in map {
                    out.push(Dependency {
                        name: name.clone(),
                        current: spec.as_str().unwrap_or("?").to_string(),
                        latest: None,
                        dev,
                    });
                }
            }
        }
        Ok(out)
    }

    fn latest_versions(&self, dir: &Path) -> Result<HashMap<String, String>> {
        match self.kind {
            Pm::Npm | Pm::Pnpm => self.outdated_json(dir),
            Pm::Yarn => self.yarn_latest(dir),
        }
    }

    fn available_versions(&self, dir: &Path, package: &str) -> Result<Vec<String>> {
        let cmd = match self.kind {
            // `npm view` e `pnpm view` são equivalentes: array JSON de versões.
            Pm::Npm | Pm::Pnpm => {
                validate_package_name(package)?;
                self.cmd(&format!("view \"{package}\" versions --json"))
            }
            // yarn v4: `yarn npm info`; o v1 clássico também tem `yarn info --json`,
            // tratado no parser tolerante abaixo.
            Pm::Yarn => {
                validate_package_name(package)?;
                self.cmd(&format!("info \"{package}\" versions --json"))
            }
        };
        let o = run_shell(&cmd, dir);
        if o.spawn_error {
            bail!("{} não está disponível: {}", self.kind.name(), o.text.trim());
        }
        if !o.ok {
            bail!("{}", tail(&o.text, 3));
        }
        // Tolerante: procura o primeiro JSON completo no stdout — o gerenciador
        // pode imprimir avisos antes do JSON.
        match extract_json(&o.stdout) {
            Some(serde_json::Value::Array(a)) => {
                Ok(a.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
            }
            Some(serde_json::Value::Object(m)) => {
                // yarn v1: {"type":"inspect","data":[...]}
                // yarn v4:  {"data":{"versions":[...],...},...}
                let list = m
                    .get("versions")
                    .or_else(|| m.get("data"))
                    .or_else(|| m.get("data").and_then(|d| d.get("versions")));
                match list {
                    Some(serde_json::Value::Array(a)) => {
                        Ok(a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    }
                    _ => Ok(vec![]),
                }
            }
            _ => Ok(vec![]),
        }
    }

    fn tool_versions_cmds(&self) -> Vec<(&'static str, &'static str)> {
        let pm_cmd = match self.kind {
            Pm::Npm => "npm --version",
            Pm::Pnpm => "pnpm --version",
            Pm::Yarn => "yarn --version",
        };
        vec![("node", "node --version"), (self.kind.name(), pm_cmd)]
    }
}

impl NodeAdapter {
    /// `npm outdated --json` / `pnpm outdated --json`: sai com código 1 quando há
    /// pacotes desatualizados (não é erro) e imprime `{pacote: {latest: ...}}`.
    fn outdated_json(&self, dir: &Path) -> Result<HashMap<String, String>> {
        let o = run_shell(&self.cmd("outdated --json"), dir);
        if o.spawn_error {
            bail!("{} não está disponível: {}", self.kind.name(), o.text.trim());
        }
        let Some(start) = o.stdout.find('{') else {
            if o.ok {
                // "All of your dependencies are up to date..."
                return Ok(HashMap::new());
            }
            bail!("{}", tail(&o.text, 5));
        };
        let v: serde_json::Value = serde_json::from_str(&o.stdout[start..])?;
        let mut out = HashMap::new();
        if let Some(map) = v.as_object() {
            for (name, info) in map {
                if let Some(l) = info.get("latest").and_then(|l| l.as_str()) {
                    out.insert(name.clone(), l.to_string());
                }
            }
        }
        Ok(out)
    }

    /// O yarn v4 não tem `outdated`: consulta o registo pacote a pacote com
    /// `yarn npm info`. Para em 60 consultas e falhas individuais não derrubam o resto.
    fn yarn_latest(&self, dir: &Path) -> Result<HashMap<String, String>> {
        let deps = self.dependencies(dir)?;
        let mut out = HashMap::new();
        for d in deps.iter().take(60) {
            let Some(latest) = self.yarn_field(dir, &d.name, "version").first().cloned() else { continue };
            if d.current != latest {
                out.insert(d.name.clone(), latest);
            }
        }
        Ok(out)
    }

    fn yarn_field(&self, dir: &Path, package: &str, field: &str) -> Vec<String> {
        let cmd = match self.kind {
            Pm::Npm | Pm::Pnpm => unreachable!(),
            Pm::Yarn => self.cmd(&format!("info \"{package}\" --fields {field} --json")),
        };
        let o = run_shell(&cmd, dir);
        if o.spawn_error || !o.ok {
            return vec![];
        }
        yarn_info_values(&o.stdout, field)
    }
}

/// Extrai os valores de um campo no JSON do `yarn (npm) info --json`,
/// tolerando a mistura de linhas (log JSON + resultado).
///
/// Formatos aceitos:
/// - v4: uma linha com `{"version": "...", ...}` ou `{"versions": [...], ...}`
/// - v1: NDJSON `{"type":"inspect","data":"1.0.0"}` ou `{"data":[...]}`
fn yarn_info_values(stdout: &str, field: &str) -> Vec<String> {
    let mut out = vec![];
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        // yarn v1: procurar em `data`.
        match v.get("data").filter(|_| v.get("type").and_then(|t| t.as_str()) == Some("inspect")) {
            Some(serde_json::Value::String(s)) => out.push(s.clone()),
            Some(serde_json::Value::Array(a)) => out.extend(a.iter().filter_map(|x| x.as_str().map(String::from))),
            // yarn v4 / genérico: procurar o campo na resposta.
            _ => match v.get(field) {
                Some(serde_json::Value::String(s)) => out.push(s.clone()),
                Some(serde_json::Value::Array(a)) => {
                    out.extend(a.iter().filter_map(|x| x.as_str().map(String::from)))
                }
                _ => {}
            },
        }
    }
    out
}

/// Procura o primeiro valor JSON (array ou objeto) completo no texto.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let bytes = text.as_bytes();
    let open = bytes.iter().position(|&b| b == b'{' || b == b'[')?;
    let mut depth = 0u32;
    let mut in_str = false;
    let mut esc = false;
    for i in open..bytes.len() {
        let b = bytes[i];
        if esc {
            esc = false;
            continue;
        }
        match b {
            b'\\' if in_str => esc = true,
            b'"' => in_str = !in_str,
            _ if in_str => {}
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str::<serde_json::Value>(&text[open..=i]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

fn validate_package_name(package: &str) -> Result<()> {
    if package.is_empty() || !package.chars().all(|c| c.is_ascii_alphanumeric() || "@/._-".contains(c)) {
        bail!("nome de pacote inválido: {package:?}");
    }
    Ok(())
}

/// O `npm init` gera "echo \"Error: no test specified\" && exit 1"; não é um teste real.
fn is_placeholder_test(cmd: Option<&str>) -> bool {
    cmd.is_some_and(|c| c.contains("no test specified"))
}

fn has_tsc(dir: &Path) -> bool {
    let bin = dir.join("node_modules").join(".bin");
    if !bin.is_dir() {
        return false;
    }
    ["tsc", "tsc.cmd", "tsc.exe", "tsc.ps1"].iter().any(|ext| bin.join(ext).exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("depsandbox-test-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn pkg(dir: &Path, json: &str) {
        fs::write(dir.join("package.json"), json).unwrap();
    }

    #[test]
    fn detection_is_exclusive_by_lockfile() {
        let dir = tmp("exclusive");
        // Só package.json: é npm.
        pkg(&dir, r#"{"name":"x","version":"1.0.0"}"#);
        assert!(NPM.detect(&dir));
        assert!(!PNPM.detect(&dir));
        assert!(!YARN.detect(&dir));

        // pnpm-lock.yaml: é pnpm (e já não é npm).
        fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();
        assert!(PNPM.detect(&dir));
        assert!(!NPM.detect(&dir));
        fs::remove_file(dir.join("pnpm-lock.yaml")).unwrap();

        // packageManager: "yarn@4.0.0": é yarn.
        pkg(&dir, r#"{"packageManager":"yarn@4.0.0"}"#);
        assert!(YARN.detect(&dir));
        assert!(!NPM.detect(&dir));
        assert!(!PNPM.detect(&dir));

        // packageManager: "pnpm@9": é pnpm.
        pkg(&dir, r#"{"packageManager":"pnpm@9.0.0"}"#);
        assert!(PNPM.detect(&dir));
        assert!(!NPM.detect(&dir));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_package_json_is_not_detected() {
        let dir = tmp("empty");
        for a in [NPM, PNPM, YARN] {
            assert!(!a.detect(&dir));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn install_and_upgrade_cmds_per_manager() {
        let dir = tmp("cmds");
        pkg(&dir, r#"{"name":"x"}"#);
        assert_eq!(NPM.install_cmd(&dir), "npm install --no-audit --no-fund");
        assert_eq!(PNPM.install_cmd(&dir), "pnpm install --no-audit");
        assert_eq!(YARN.install_cmd(&dir), "yarn install");

        fs::write(dir.join("yarn.lock"), "").unwrap();
        assert_eq!(YARN.install_cmd(&dir), "yarn install --frozen-lockfile");
        fs::write(dir.join(".yarnrc.yml"), "").unwrap();
        assert_eq!(YARN.install_cmd(&dir), "yarn install --immutable");

        let target = UpgradeTarget::parse("chalk@5");
        assert_eq!(NPM.upgrade_cmd(&target), "npm install \"chalk@5\" --no-audit --no-fund");
        assert_eq!(PNPM.upgrade_cmd(&target), "pnpm add \"chalk@5\" --no-audit");
        assert_eq!(YARN.upgrade_cmd(&target), "yarn add \"chalk@5\" --exact");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn checks_use_project_manager() {
        let dir = tmp("checks");
        pkg(&dir, r#"{"scripts":{"test":"jest","build":"tsc"}}"#);
        let npm = NPM.checks(&dir).unwrap();
        let cmds: Vec<&str> = npm.iter().map(|c| c.command.as_str()).collect();
        assert_eq!(cmds, ["npm run build", "npm test"]);
        assert_eq!(PNPM.checks(&dir).unwrap()[0].command, "pnpm run build");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dependencies_read_dev_and_prod() {
        let dir = tmp("deps");
        pkg(&dir, r#"{"dependencies":{"react":"^19"},"devDependencies":{"vitest":"^1"}}"#);
        let deps = NPM.dependencies(&dir).unwrap();
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "react" && !d.dev));
        assert!(deps.iter().any(|d| d.name == "vitest" && d.dev));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_json_finds_arrays_and_objects() {
        let v = extract_json("warning: coisa\n[{\"a\":1}, {\"b\":\"x\"}] fim").unwrap();
        assert!(v.is_array());

        let v = extract_json("{\"type\":\"inspect\",\"data\":{\"versions\":[\"1.0.0\"]}}").unwrap();
        assert!(v.is_object());

        assert!(extract_json("sem json aqui").is_none());
        assert!(extract_json("{ incompleto").is_none());
    }

    #[test]
    fn yarn_info_parses_berry_and_v1() {
        // v4: a linha do resultado tem o campo à solta.
        let berry = "{\"type\":\"log\",\"decoded\":\"x\"}\n{\"versions\":[\"1.0.0\",\"1.1.0\"],\"version\":\"1.1.0\"}";
        assert_eq!(yarn_info_values(berry, "versions"), vec!["1.0.0", "1.1.0"]);
        assert_eq!(yarn_info_values(berry, "version"), vec!["1.1.0"]);

        // v1: NDJSON {"type":"inspect","data":...}.
        let v1 = "{\"type\":\"step\"}\n{\"type\":\"inspect\",\"data\":[\"2.0.0\"]}";
        assert_eq!(yarn_info_values(v1, "versions"), vec!["2.0.0"]);
        let v1s = "{\"type\":\"inspect\",\"data\":\"2.1.0\"}";
        assert_eq!(yarn_info_values(v1s, "version"), vec!["2.1.0"]);

        assert!(yarn_info_values("não é json", "versions").is_empty());
    }

    #[test]
    fn invalid_package_name_is_refused() {
        assert!(validate_package_name("chalk").is_ok());
        assert!(validate_package_name("@scope/pkg").is_ok());
        assert!(validate_package_name("x; rm -rf /").is_err());
        assert!(validate_package_name("").is_err());
    }

    #[test]
    fn tool_versions_include_node_and_manager() {
        let cmds = YARN.tool_versions_cmds();
        assert_eq!(cmds[0].0, "node");
        assert_eq!(cmds[1].1, "yarn --version");
    }
}
