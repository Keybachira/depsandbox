//! Um adapter por ecossistema, todos implementam `depsandbox_core::Adapter`.
pub mod node;

use depsandbox_core::Adapter;
use depsandbox_core::Ecosystem;
use std::path::Path;
use std::sync::Arc;

pub use node::{NodeAdapter, NPM, PNPM, YARN, Pm};

/// Todos os adapters disponíveis (cargo e pip entram nas fases seguintes).
/// A ordem importada: ver `detect` — npm é o fallback de qualquer projeto Node
/// sem pnpm/yarn.
pub fn all() -> Vec<Arc<dyn Adapter>> {
    vec![Arc::new(NPM), Arc::new(PNPM), Arc::new(YARN)]
}

pub fn detect(dir: &Path) -> Vec<Arc<dyn Adapter>> {
    all().into_iter().filter(|a| a.detect(dir)).collect()
}

/// Encontra um adapter pelo nome do ecossistema (como está no depsandbox.toml).
pub fn by_ecosystem(name: &str) -> Option<Arc<dyn Adapter>> {
    all().into_iter().find(|a| match a.ecosystem() {
        Ecosystem::Npm => name == "npm",
        Ecosystem::Pnpm => name == "pnpm",
        Ecosystem::Yarn => name == "yarn",
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_ecosystem_finds_node_managers() {
        assert!(by_ecosystem("npm").is_some());
        assert!(by_ecosystem("pnpm").is_some());
        assert!(by_ecosystem("yarn").is_some());
        assert!(by_ecosystem("pip").is_none());
        assert!(by_ecosystem("npm2").is_none());
    }
}
