use anyhow::Result;
use depsandbox_core::{Adapter, CheckSpec, Ecosystem, UpgradeTarget};
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
        format!("npm install {} --no-audit --no-fund", target.spec())
    }

    fn checks(&self, dir: &Path) -> Result<Vec<CheckSpec>> {
        let raw = std::fs::read_to_string(dir.join("package.json"))?;
        let pkg: serde_json::Value = serde_json::from_str(&raw)?;
        let scripts = pkg.get("scripts").and_then(|s| s.as_object());
        let has = |n: &str| scripts.map_or(false, |s| s.contains_key(n));

        let mut out = vec![];
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

    fn tool_versions_cmds(&self) -> Vec<(&'static str, &'static str)> {
        vec![("node", "node --version"), ("npm", "npm --version")]
    }
}

/// O `npm init` gera "echo \"Error: no test specified\" && exit 1"; não é um teste real.
fn is_placeholder_test(cmd: Option<&str>) -> bool {
    cmd.map_or(false, |c| c.contains("no test specified"))
}
