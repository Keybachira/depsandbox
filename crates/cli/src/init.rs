use anyhow::{bail, Result};
use depsandbox_core::config::{ChecksConfig, CONFIG_FILE};
use depsandbox_core::Config;
use inquire::{Confirm, Text};
use std::io::IsTerminal;
use std::path::Path;

/// Perguntas -> Respostas -> Config. As respostas vêm de flags, de prompts ou dos padrões detectados.
pub fn run(
    dir: &Path,
    yes: bool,
    test_flag: Option<String>,
    build_flag: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let adapters = depsandbox_adapters::detect(dir);
    if adapters.is_empty() {
        bail!("nenhum ecossistema suportado detectado em {}", dir.display());
    }
    let names: Vec<_> = adapters.iter().map(|a| format!("{:?}", a.ecosystem())).collect();
    eprintln!("✔ Detectado: {}", names.join(", "));

    // Padrões inferidos do primeiro adapter.
    let detected = adapters[0].checks(dir)?;
    let find = |n: &str| detected.iter().find(|c| c.name == n).map(|c| c.command.clone());
    let (def_test, def_build, def_type) = (find("test"), find("build"), find("typecheck"));

    let interactive = !yes && std::io::stdin().is_terminal();
    let needs_prompt = test_flag.is_none() || build_flag.is_none();
    if !yes && !interactive && needs_prompt {
        bail!("sem terminal interativo: passe --yes ou as flags --test-cmd / --build-cmd");
    }

    let ask = |label: &str, flag: Option<String>, default: Option<String>| -> Result<Option<String>> {
        if let Some(v) = flag {
            return Ok(Some(v));
        }
        if !interactive {
            return Ok(default);
        }
        let mut p = Text::new(label);
        if let Some(d) = &default {
            p = p.with_default(d);
        }
        let v = p.with_help_message("vazio para não executar esta verificação").prompt()?;
        Ok(if v.trim().is_empty() { None } else { Some(v) })
    };

    let test = ask("Comando de testes", test_flag, def_test)?;
    let build = ask("Comando de build", build_flag, def_build)?;
    let ignore: Vec<String> = if interactive {
        Text::new("Pacotes a ignorar (separados por vírgula)")
            .with_default("")
            .prompt()?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![]
    };

    let cfg = Config {
        ecosystem: Some(names[0].to_lowercase()),
        checks: ChecksConfig { test, build, typecheck: def_type },
        ignore,
    };

    if dry_run {
        println!("{}", cfg.to_toml()?);
        return Ok(());
    }

    let target = dir.join(CONFIG_FILE);
    if target.exists() && interactive && !Confirm::new(&format!("{CONFIG_FILE} já existe. Sobrescrever?")).with_default(false).prompt()? {
        eprintln!("cancelado");
        return Ok(());
    }
    cfg.save(dir)?;
    eprintln!("✔ Criado {}", target.display());
    Ok(())
}
