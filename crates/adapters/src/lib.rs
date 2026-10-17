//! Um adapter por ecossistema, todos implementam `depsandbox_core::Adapter`.
pub mod npm;

use depsandbox_core::Adapter;
use std::path::Path;
use std::sync::Arc;

pub use npm::NpmAdapter;

/// Todos os adapters disponíveis (cargo e pip entram nas fases seguintes).
pub fn all() -> Vec<Arc<dyn Adapter>> {
    vec![Arc::new(NpmAdapter)]
}

pub fn detect(dir: &Path) -> Vec<Arc<dyn Adapter>> {
    all().into_iter().filter(|a| a.detect(dir)).collect()
}
