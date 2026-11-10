//! Pinned projects: the ones the user is driving, ranked by the user. The
//! daemon holds the rank (`Project::pin_order`) so every client shows the
//! same order and it survives restarts.

use std::collections::HashMap;

use anyhow::Result;
use berth_core::{Entity, ProjectId, ServerEvent};

use crate::registry::Daemon;

impl Daemon {
    /// Replace the pinned list with `order`, highest priority first, and
    /// broadcast every project whose rank changed.
    pub fn set_project_pins(&self, order: &[ProjectId]) -> Result<()> {
        let before: HashMap<ProjectId, Option<i64>> = self
            .store
            .load_tree()?
            .0
            .into_iter()
            .map(|p| (p.id, p.pin_order))
            .collect();
        self.store.set_project_pins(order)?;
        for project in self.store.load_tree()?.0 {
            if before.get(&project.id) != Some(&project.pin_order) {
                self.broadcast(ServerEvent::EntityUpserted {
                    entity: Entity::Project(project),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use berth_core::{Project, WorkspaceId};

    use super::*;
    use crate::hooks::HookEnv;
    use crate::store::Store;

    fn daemon(store: Store) -> Arc<Daemon> {
        Daemon::new(
            Arc::new(store),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        )
    }

    fn project(id: &str, sort_order: i64) -> Project {
        Project {
            id: ProjectId(id.into()),
            name: id.into(),
            workspace_id: WorkspaceId::default(),
            repo_path: PathBuf::from(format!("/repos/{id}")),
            sort_order,
            pin_order: None,
        }
    }

    fn pins(store: &Store) -> Vec<(String, Option<i64>)> {
        store
            .load_tree()
            .unwrap()
            .0
            .into_iter()
            .map(|p| (p.id.0, p.pin_order))
            .collect()
    }

    #[test]
    fn set_project_pins_persists_across_reopen_and_reaches_other_clients() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("berth.db");
        {
            let store = Store::open(&db).unwrap();
            for (i, id) in ["a", "b", "c"].into_iter().enumerate() {
                store.insert_project(&project(id, i as i64)).unwrap();
            }
            let daemon = daemon(store);
            let mut other_client = daemon.events.subscribe();

            let order = |ids: &[&str]| {
                ids.iter()
                    .map(|id| ProjectId((*id).into()))
                    .collect::<Vec<_>>()
            };
            daemon
                .set_project_pins(&order(&["c", "a", "ghost"]))
                .unwrap();

            let mut seen = Vec::new();
            while let Ok(ServerEvent::EntityUpserted {
                entity: Entity::Project(p),
            }) = other_client.try_recv()
            {
                seen.push((p.id.0, p.pin_order));
            }
            seen.sort();
            assert_eq!(
                seen,
                vec![("a".into(), Some(1)), ("c".into(), Some(0))],
                "only the projects whose rank changed are broadcast"
            );

            daemon.set_project_pins(&order(&["a"])).unwrap();
            let mut seen = Vec::new();
            while let Ok(ServerEvent::EntityUpserted {
                entity: Entity::Project(p),
            }) = other_client.try_recv()
            {
                seen.push((p.id.0, p.pin_order));
            }
            seen.sort();
            assert_eq!(
                seen,
                vec![("a".into(), Some(0)), ("c".into(), None)],
                "a project left out of the list is unpinned"
            );
        }

        let reopened = Store::open(&db).unwrap();
        assert_eq!(
            pins(&reopened),
            vec![
                ("a".into(), Some(0)),
                ("b".into(), None),
                ("c".into(), None)
            ],
            "the ranks survive a daemon restart"
        );
    }
}
