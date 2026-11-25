use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Blobs {
    dir: Mutex<PathBuf>,
}

impl Default for Blobs {
    fn default() -> Self {
        Self {
            dir: Mutex::new(berth_core::paths::acp_blob_dir()),
        }
    }
}

impl Blobs {
    #[cfg(test)]
    pub fn move_to(&self, dir: PathBuf) {
        *self.dir.lock().unwrap() = dir;
    }

    fn path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("no such attachment");
        }
        Ok(self.dir.lock().unwrap().join(id))
    }

    pub async fn put(&self, bytes: &[u8]) -> Result<String> {
        let id = ulid::Ulid::generate().to_string();
        let path = self.path(&id)?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .context("store the attachment")?;
        Ok(id)
    }

    pub async fn get(&self, id: &str) -> Result<Vec<u8>> {
        tokio::fs::read(self.path(id)?)
            .await
            .context("read the attachment")
    }
}
