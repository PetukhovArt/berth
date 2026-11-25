mod acp;

use super::providers::AcpProvider;
use super::wire::{Host, Wire};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

pub const ACP: &str = "acp";

pub trait Driver: Sync {
    fn open(&self, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire>;
}

static DRIVERS: &[(&str, &dyn Driver)] = &[(ACP, &acp::PassThrough)];

pub fn open(name: &str, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire> {
    let driver = DRIVERS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, driver)| *driver)
        .with_context(|| format!("{}: unknown driver `{name}`", provider.name))?;
    driver.open(provider, cwd, host)
}
