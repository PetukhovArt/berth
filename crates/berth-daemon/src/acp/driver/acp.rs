use super::Driver;
use crate::acp::providers::AcpProvider;
use crate::acp::wire::{Host, LaunchSpec, Wire};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

pub struct PassThrough;

impl Driver for PassThrough {
    fn open(&self, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire> {
        host.spawn(LaunchSpec {
            program: provider.command.clone(),
            args: provider.args.clone(),
            cwd: cwd.to_path_buf(),
        })
    }
}
