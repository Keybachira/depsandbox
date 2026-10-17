use anyhow::Result;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Diretórios nunca copiados para o sandbox (são regenerados pelo install).
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".venv", "venv", "__pycache__", ".git", "dist", ".next"];

/// Copia o projeto para um diretório temporário. O projeto real nunca é tocado.
/// (Versão simples; `reflink`/hardlink fica para otimizar depois.)
pub fn snapshot(src: &Path) -> Result<TempDir> {
    let tmp = tempfile::Builder::new().prefix("depsandbox-").tempdir()?;
    copy_dir(src, tmp.path())?;
    Ok(tmp)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let ft = entry.file_type()?;
        let to = dst.join(&name);
        if ft.is_dir() {
            if SKIP_DIRS.iter().any(|s| name == *s) {
                continue;
            }
            fs::create_dir_all(&to)?;
            copy_dir(&entry.path(), &to)?;
        } else if ft.is_file() {
            fs::copy(entry.path(), &to)?;
        }
        // symlinks ignorados de propósito no MVP
    }
    Ok(())
}
