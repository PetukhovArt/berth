use anyhow::{bail, Result};
use berth_daemon::shared_setup::SharedSetup;

use crate::client::{connect_or_spawn, runtime};
use crate::sessions::snapshot;

pub fn run_sync() -> Result<()> {
    runtime()?.block_on(async {
        let setup = match SharedSetup::fetch().await? {
            Some(setup) => setup,
            None => {
                println!("no shared_setup in the config: removing what berth installed before");
                SharedSetup::default()
            }
        };
        let mut conn = connect_or_spawn().await?;
        let tree = snapshot(&mut conn).await?;
        let mut failed = 0;
        for worktree in &tree.worktrees {
            let Some(project) = tree.projects.iter().find(|p| p.id == worktree.project_id) else {
                continue;
            };
            if !worktree.path.is_dir() {
                continue;
            }
            let at = format!("{} {}", project.name, worktree.path.display());
            match setup.sync(project, &worktree.path) {
                Ok(report) => {
                    println!(
                        "{at}: {} written, {} removed",
                        report.written.len(),
                        report.removed.len()
                    );
                    for path in report.kept_as_found {
                        println!("  kept {path}: not written by berth, or changed since");
                    }
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("{at}: {e:#}");
                }
            }
        }
        if failed > 0 {
            bail!("the shared setup failed in {failed} checkout(s)");
        }
        Ok(())
    })
}
