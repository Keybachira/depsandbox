mod init;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use depsandbox_core::engine::RunOptions;
use depsandbox_core::{engine, Config, Report, UpgradeTarget, Verdict};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "depsandbox", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Cria o depsandbox.toml (interativo; use --yes para aceitar os padrões detectados)
    Init {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Aceita todos os padrões sem perguntar
        #[arg(short, long)]
        yes: bool,
        /// Comando de testes
        #[arg(long)]
        test_cmd: Option<String>,
        /// Comando de build
        #[arg(long)]
        build_cmd: Option<String>,
        /// Mostra o ficheiro que seria criado, sem escrever
        #[arg(long)]
        dry_run: bool,
    },
    /// Testa um upgrade num sandbox e gera o relatório
    Try {
        /// Pacote, p.ex. `lodash`, `lodash@5`, `@scope/pkg@1.2.3`
        package: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Md)]
        format: Format,
        /// Escreve o relatório num ficheiro em vez do stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Abre a interface interativa (TUI): escolhe pacotes, testa e vê os resultados
    Tui {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Imprime o JSON Schema do relatório
    Schema,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Md,
    Json,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erro: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    match Cli::parse().command {
        Cmd::Init { path, yes, test_cmd, build_cmd, dry_run } => {
            init::run(&path, yes, test_cmd, build_cmd, dry_run)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Try { package, path, format, output } => {
            let cfg = Config::load(&path)?;
            try_cmd(&package, &path, &cfg, format, output)
        }
        Cmd::Tui { path } => {
            let cfg = Config::load(&path)?;
            let adapter = pick_adapter(&path, &cfg)?;
            depsandbox_tui::run(path, adapter, cfg)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Schema => {
            let schema = schemars::schema_for!(Report);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn try_cmd(package: &str, path: &Path, cfg: &Config, format: Format, output: Option<PathBuf>) -> Result<ExitCode> {
    let adapter = pick_adapter(path, cfg)?;

    let target = UpgradeTarget::parse(package);
    if cfg.ignore.iter().any(|i| i == &target.package) {
        bail!("'{}' está em `ignore` no depsandbox.toml", target.package);
    }

    let report = engine::try_upgrade(adapter.as_ref(), path, &target, cfg, &RunOptions::default(), &|m| eprintln!("• {m}"))?;

    let text = match format {
        Format::Md => depsandbox_report::to_markdown(&report),
        Format::Json => depsandbox_report::to_json(&report)?,
    };
    match output {
        Some(p) => {
            std::fs::write(&p, &text).with_context(|| format!("a escrever {}", p.display()))?;
            eprintln!("relatório escrito em {}", p.display());
        }
        None => println!("{text}"),
    }

    Ok(match report.verdict {
        Verdict::NoIssuesDetected => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    })
}

fn pick_adapter(path: &Path, cfg: &Config) -> Result<Arc<dyn depsandbox_core::Adapter>> {
    // O depsandbox.toml pode fixar o ecossistema (ex.: monorepos com dois lockfiles).
    if let Some(name) = &cfg.ecosystem {
        return match depsandbox_adapters::by_ecosystem(name) {
            Some(a) => Ok(a),
            None => bail!(
                "ecossistema '{}' no depsandbox.toml não é suportado (opções: npm, pnpm, yarn)",
                name
            ),
        };
    }
    let mut adapters = depsandbox_adapters::detect(path);
    match adapters.len() {
        0 => bail!("nenhum ecossistema suportado detectado em {}", path.display()),
        1 => Ok(adapters.remove(0)),
        n => bail!(
            "{n} ecossistemas detectados ({:?}); fixe `ecosystem` no depsandbox.toml",
            adapters.iter().map(|a| a.ecosystem().name()).collect::<Vec<_>>()
        ),
    }
}
