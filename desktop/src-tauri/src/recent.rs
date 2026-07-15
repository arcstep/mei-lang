use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentFile {
    paths: Vec<String>,
}

#[derive(Debug, Default)]
pub struct RecentStore {
    paths: Vec<PathBuf>,
}

impl RecentStore {
    pub fn load() -> anyhow::Result<Self> {
        let path = paths::recent_file()?;
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)?;
        let file: RecentFile = serde_json::from_str(&raw).unwrap_or_default();
        Ok(Self {
            paths: file.paths.into_iter().map(PathBuf::from).collect(),
        })
    }

    pub fn list(&self) -> Vec<PathBuf> {
        self.paths.clone()
    }

    pub fn push(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.paths.retain(|p| p != &path);
        self.paths.insert(0, path);
        self.paths.truncate(12);
        self.save()
    }

    fn save(&self) -> anyhow::Result<()> {
        let path = paths::recent_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = RecentFile {
            paths: self
                .paths
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        };
        fs::write(path, serde_json::to_vec_pretty(&file)?)?;
        Ok(())
    }
}
