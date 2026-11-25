mod acp;
mod blocks;
mod claude;
mod codex;
mod options;
mod shape;
mod translator;

pub use translator::{MCP_STATUS, REWIND};

use super::providers::AcpProvider;
use super::wire::{Host, Wire};
use anyhow::{Context, Result};
use berth_core::acp::ACP_DRIVER;
use std::path::Path;
use std::sync::Arc;

pub trait Driver: Sync {
    fn open(&self, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire>;
}

static DRIVERS: &[(&str, &dyn Driver)] = &[
    (ACP_DRIVER, &acp::PassThrough),
    ("claude", &claude::Claude),
    ("codex", &codex::Codex),
];

pub fn open(provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire> {
    let driver = DRIVERS
        .iter()
        .find(|(known, _)| *known == provider.driver)
        .map(|(_, driver)| *driver)
        .with_context(|| format!("{}: unknown driver `{}`", provider.name, provider.driver))?;
    driver.open(provider, cwd, host)
}
