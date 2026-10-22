use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::cli::Cli;

#[derive(Clone, Debug)]
pub struct Paths {
    pub assets: PathBuf,
    pub scenarios: PathBuf,
    pub discovered_source: Option<PathBuf>,
}

impl Paths {
    pub fn resolve(cli: &Cli) -> Result<Self> {
        let executable = env::current_exe().context("cannot locate bv-sim executable")?;
        let root = cli
            .data_dir
            .clone()
            .or_else(|| discover_data_root(&executable));
        let fallback = executable.parent().unwrap_or(Path::new("."));
        let root = root.as_deref().unwrap_or(fallback);
        Ok(Self {
            assets: cli
                .assets_dir
                .clone()
                .unwrap_or_else(|| root.join("assets")),
            scenarios: cli
                .scenarios_dir
                .clone()
                .unwrap_or_else(|| root.join("scenarios")),
            discovered_source: discover_source_root(&executable).or_else(|| {
                env::current_dir()
                    .ok()
                    .and_then(|cwd| discover_source_root(&cwd))
            }),
        })
    }

    pub fn require_assets(&self) -> Result<()> {
        if !self.assets.is_dir() {
            bail!(
                "visual assets are missing at {}. Install the complete release bundle or set --assets-dir /path/to/assets",
                self.assets.display()
            );
        }
        Ok(())
    }
}

fn discover_data_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .take(7)
        .find(|candidate| candidate.join("assets").is_dir() && candidate.join("scenarios").is_dir())
        .map(Path::to_path_buf)
}

fn discover_source_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .take(7)
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("backend/Dockerfile").is_file()
        })
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn release_and_source_data_are_found_from_executable_without_cwd() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "bv-sim-paths-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::create_dir_all(root.join("scenarios")).unwrap();
        assert_eq!(
            discover_data_root(&root.join("bin/bv-sim")),
            Some(root.clone())
        );
        assert_eq!(
            discover_data_root(&root.join("target/debug/bv-sim")),
            Some(root.clone())
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
