use crate::model::{Dependency, Ecosystem, UpgradeTarget};
use std::collections::HashMap;
use anyhow::Result;
use std::path::Path;

/// Uma verificação a correr dentro do sandbox (build, testes, typecheck...).
#[derive(Debug, Clone)]
pub struct CheckSpec {
    pub name: String,
    pub command: String,
}

/// Interface comum a todos os ecossistemas.
pub trait Adapter: Send + Sync {
    fn ecosystem(&self) -> Ecosystem;
    /// O diretório contém um projeto deste ecossistema?
    fn detect(&self, dir: &Path) -> bool;
    /// Ficheiros de lock usados para o hash do manifest.
    fn lockfiles(&self) -> &'static [&'static str];
    /// Comando de instalação (baseline), correr no sandbox.
    fn install_cmd(&self, dir: &Path) -> String;
    /// Comando que aplica o upgrade e atualiza o lockfile.
    fn upgrade_cmd(&self, target: &UpgradeTarget) -> String;
    /// Verificações a correr, inferidas do projeto.
    fn checks(&self, dir: &Path) -> Result<Vec<CheckSpec>>;
    /// Ferramentas cujas versões entram no manifest, p.ex. ("node", "node --version").
    fn tool_versions_cmds(&self) -> Vec<(&'static str, &'static str)>;
    /// Dependências declaradas (leitura local, rápida).
    fn dependencies(&self, dir: &Path) -> Result<Vec<Dependency>>;
    /// Versões mais recentes por pacote (consulta o registo; lenta). Só inclui pacotes desatualizados.
    fn latest_versions(&self, dir: &Path) -> Result<HashMap<String, String>>;
    /// Todas as versões publicadas de um pacote (consulta o registo).
    fn available_versions(&self, dir: &Path, package: &str) -> Result<Vec<String>>;
}
