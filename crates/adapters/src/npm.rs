use anyhow::{bail, Result};
use depsandbox_core::runner::{run_shell, tail};
use depsandbox_core::{Adapter, CheckSpec, Dependency, Ecosystem, UpgradeTarget};
use std::collections::HashMap;
use std::path::Path;

pub struct NpmAdapter;

impl Adapter for NpmAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Npm
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("package.json").is_file()
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        &["package-lock.json", "npm-shrinkwrap.json"]
    }

    fn install_cmd(&self, dir: &Path) -> String {
        // `npm ci` é reproduzível quando há lockfile; senão `npm install`.
        if dir.join("package-lock.json").is_file() {
            "npm ci --no-audit --no-fund".into()
        } else {
            "npm install --no-audit --no-fund".into()
        }
    }

    fn upgrade_cmd(&self, target: &UpgradeTarget) -> String {
        format!("npm install \"{}\" --no-audit --no-fund", target.spec())
    }

    fn checks(&self, dir: &Path) -> Result<Vec<CheckSpec>> {
        let raw = std::fs::read_to_string(dir.join("package.json"))?;
        let pkg: serde_json::Value = serde_json::from_str(&raw)?;
        let scripts = pkg.get("scripts").and_then(|s| s.as_object());
        let has = |n: &str| scripts.map_or(false, |s| s.contains_key(n));

        let mut out = vec![];
        // Sem script `typecheck`, mas com tsconfig e tsc instalado: usa `tsc --noEmit`.
        if !has("typecheck") && dir.join("tsconfig.json").is_file() && has_tsc(dir) {
            out.push(CheckSpec { name: "typecheck".into(), command: "npx --no-install tsc --noEmit".into() });
        }
        // Ordem: tipos -> build -> testes (falha rápida e mais barata primeiro).
        for name in ["typecheck", "build"] {
            if has(name) {
                out.push(CheckSpec { name: name.into(), command: format!("npm run {name}") });
            }
        }
        if has("test") && !is_placeholder_test(scripts.and_then(|s| s["test"].as_str())) {
            out.push(CheckSpec { name: "test".into(), command: "npm test".into() });
        }
        Ok(out)
    }

    fn dependencies(&self, dir: &Path) -> Result<Vec<Dependency>> {
        let raw = std::fs::read_to_string(dir.join("package.json"))?;
        let pkg: serde_json::Value = serde_json::from_str(&raw)?;
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
        // `npm outdated` sai com código 1 quando há pacotes desatualizados: não é erro.
        let o = run_shell("npm outdated --json", dir);
        if o.spawn_error {
            bail!("{}", o.text);
        }
        let Some(start) = o.stdout.find('{') else {
            if o.ok {
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

    fn available_versions(&self, dir: &Path, package: &str) -> Result<Vec<String>> {
        if package.is_empty() || !package.chars().all(|c| c.is_ascii_alphanumeric() || "@/._-".contains(c)) {
            bail!("nome de pacote inválido: {package:?}");
        }
        let o = run_shell(&format!("npm view \"{package}\" versions --json"), dir);
        if !o.ok {
            bail!("{}", tail(&o.text, 3));
        }
        match serde_json::from_str::<serde_json::Value>(o.stdout.trim())? {
            serde_json::Value::Array(a) => Ok(a.into_iter().filter_map(|x| x.as_str().map(String::from)).collect()),
            serde_json::Value::String(s) => Ok(vec![s]),
            _ => Ok(vec![]),
        }
    }

    fn tool_versions_cmds(&self) -> Vec<(&'static str, &'static str)> {
        vec![("node", "node --version"), ("npm", "npm --version")]
    }
}

/// O `npm init` gera "echo \"Error: no test specified\" && exit 1"; não é um teste real.
fn is_placeholder_test(cmd: Option<&str>) -> bool {
    cmd.map_or(false, |c| c.contains("no test specified"))
}

fn has_tsc(dir: &Path) -> bool {
    let bin = dir.join("node_modules").join(".bin");
    bin.join("tsc").exists() || bin.join("tsc.cmd").exists()
}
