use crate::client::{
    connect_or_spawn, handshake, runtime, try_connect, Connection, ONE_SHOT_REQ_ID,
};
use anyhow::{bail, Result};
use berth_core::codec::{read_frame, write_frame};
use berth_core::{
    Agent, AgentId, AgentStatus, ClientRequest, Project, ServerEvent, SessionRef, Workspace,
    Worktree,
};
use serde_json::{json, Value};
use std::time::Duration;

const SUBMIT_DELAY: Duration = Duration::from_millis(150);

fn attention(agent: &Agent) -> Option<&'static str> {
    if agent.archived {
        return None;
    }
    match agent.status {
        AgentStatus::NeedsFeedback => Some("needs_feedback"),
        AgentStatus::Finished if agent.unseen => Some("unseen"),
        _ => None,
    }
}

fn session_json(
    agent: &Agent,
    workspaces: &[Workspace],
    projects: &[Project],
    worktrees: &[Worktree],
) -> Value {
    let worktree = worktrees.iter().find(|w| w.id == agent.worktree_id);
    let project = worktree.and_then(|w| projects.iter().find(|p| p.id == w.project_id));
    let workspace = project.and_then(|p| workspaces.iter().find(|w| w.id == p.workspace_id));
    json!({
        "id": agent.id.as_str(),
        "name": agent.name,
        "status": agent.status.as_str(),
        "attention": attention(agent),
        "alive": agent.alive,
        "archived": agent.archived,
        "kind": agent.kind.as_str(),
        "model": agent.model,
        "status_changed_at": agent.status_changed_at,
        "workspace": workspace.map(|w| w.name.as_str()),
        "project": project.map(|p| p.name.as_str()),
        "branch": worktree.map(|w| w.branch.as_str()),
        "path": worktree.map(|w| w.path.display().to_string()),
    })
}

pub fn sessions_document(
    workspaces: &[Workspace],
    projects: &[Project],
    worktrees: &[Worktree],
    agents: &[Agent],
    all: bool,
    only_attention: bool,
) -> Value {
    let rows: Vec<Value> = agents
        .iter()
        .filter(|a| all || !a.archived)
        .filter(|a| !only_attention || attention(a).is_some())
        .map(|a| session_json(a, workspaces, projects, worktrees))
        .collect();
    json!({ "ok": true, "sessions": rows })
}

pub fn resolve_session<'a>(agents: &'a [Agent], query: &str) -> Result<&'a Agent> {
    let live: Vec<&Agent> = agents.iter().filter(|a| !a.archived).collect();
    if let Some(a) = live.iter().find(|a| a.id.as_str() == query) {
        return Ok(a);
    }
    let unique = |matches: Vec<&'a Agent>, how: &str| -> Result<Option<&'a Agent>> {
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches[0])),
            _ => bail!(
                "{how} `{query}` matches several sessions: {}",
                matches
                    .iter()
                    .map(|a| a.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    };
    let by_name = live.iter().copied().filter(|a| a.name == query).collect();
    if let Some(a) = unique(by_name, "name")? {
        return Ok(a);
    }
    let by_prefix = live
        .iter()
        .copied()
        .filter(|a| a.id.as_str().starts_with(query))
        .collect();
    if let Some(a) = unique(by_prefix, "id prefix")? {
        return Ok(a);
    }
    bail!("no session matches `{query}`")
}

fn prompt_chunks(text: &str, submit: bool) -> Vec<Vec<u8>> {
    let clean: String = text.chars().filter(|c| *c != '\u{1b}').collect();
    let mut chunks = vec![format!("\u{1b}[200~{clean}\u{1b}[201~").into_bytes()];
    if submit {
        chunks.push(b"\r".to_vec());
    }
    chunks
}

fn failure_json(err: &anyhow::Error) -> Value {
    json!({ "ok": false, "error": format!("{err:#}") })
}

fn finish(result: Result<Value>) -> Result<()> {
    match result {
        Ok(doc) => {
            println!("{doc}");
            Ok(())
        }
        Err(err) => {
            println!("{}", failure_json(&err));
            std::process::exit(1);
        }
    }
}

struct Snapshot {
    workspaces: Vec<Workspace>,
    projects: Vec<Project>,
    worktrees: Vec<Worktree>,
    agents: Vec<Agent>,
}

async fn snapshot(conn: &mut Connection) -> Result<Snapshot> {
    write_frame(&mut conn.stream, &ClientRequest::Subscribe).await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Snapshot {
                workspaces,
                projects,
                worktrees,
                agents,
                ..
            }) => {
                return Ok(Snapshot {
                    workspaces,
                    projects,
                    worktrees,
                    agents,
                })
            }
            Some(_) => continue,
            None => bail!("daemon closed the connection before sending a snapshot"),
        }
    }
}

pub(crate) async fn list(all: bool, only_attention: bool) -> Result<Value> {
    let mut conn = connect_or_spawn().await?;
    let s = snapshot(&mut conn).await?;
    Ok(sessions_document(
        &s.workspaces,
        &s.projects,
        &s.worktrees,
        &s.agents,
        all,
        only_attention,
    ))
}

pub(crate) async fn task_note(session: Option<&str>) -> Result<Value> {
    let query = match session {
        Some(s) => s.to_string(),
        None => berth_core::env::non_empty(berth_core::env::AGENT_ID)
            .or_else(|| berth_core::env::non_empty(berth_core::legacy::AGENT_ID))
            .ok_or_else(|| anyhow::anyhow!("`session` is required outside a berth session"))?,
    };
    let mut conn = connect_or_spawn().await?;
    let s = snapshot(&mut conn).await?;
    let agent = resolve_session(&s.agents, &query)?;
    let worktree = agent.worktree_id.clone();
    write_frame(
        &mut conn.stream,
        &ClientRequest::GetTaskNote {
            req_id: ONE_SHOT_REQ_ID,
            worktree: worktree.clone(),
        },
    )
    .await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::TaskNote { text, .. }) => {
                let branch = s
                    .worktrees
                    .iter()
                    .find(|w| w.id == worktree)
                    .map(|w| w.branch.as_str());
                return Ok(json!({
                    "ok": true,
                    "session": { "id": agent.id.as_str(), "name": agent.name },
                    "branch": branch,
                    "note": if text.is_empty() { None } else { Some(text) },
                }));
            }
            Some(ServerEvent::Error { message, .. }) => bail!("{message}"),
            Some(_) => continue,
            None => bail!("daemon closed the connection before answering"),
        }
    }
}

pub(crate) async fn send(session: &str, text: &str, submit: bool) -> Result<Value> {
    if text.trim().is_empty() {
        bail!("the message is empty");
    }
    let Ok(stream) = try_connect().await else {
        bail!("no berth daemon is running");
    };
    let mut conn = handshake(stream).await?;
    let s = snapshot(&mut conn).await?;
    let agent = resolve_session(&s.agents, session)?;
    if !agent.alive {
        bail!(
            "session `{}` is not running (status {}), the message would be dropped",
            agent.name,
            agent.status.as_str()
        );
    }
    let target = SessionRef::Agent(AgentId(agent.id.0.clone()));
    let mut bytes = 0;
    for (i, data) in prompt_chunks(text, submit).into_iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(SUBMIT_DELAY).await;
        }
        bytes += data.len();
        write_frame(
            &mut conn.stream,
            &ClientRequest::Input {
                session: target.clone(),
                data,
            },
        )
        .await?;
    }
    write_frame(
        &mut conn.stream,
        &ClientRequest::GetMetrics {
            req_id: ONE_SHOT_REQ_ID,
        },
    )
    .await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Metrics { .. }) => break,
            Some(_) => continue,
            None => bail!("daemon closed the connection before confirming"),
        }
    }
    Ok(json!({
        "ok": true,
        "session": { "id": agent.id.as_str(), "name": agent.name },
        "submitted": submit,
        "bytes": bytes,
    }))
}

pub fn run_sessions(all: bool) -> Result<()> {
    finish(runtime()?.block_on(list(all, false)))
}

pub fn run_attention() -> Result<()> {
    finish(runtime()?.block_on(list(false, true)))
}

pub fn run_send(session: String, text: String, submit: bool) -> Result<()> {
    finish(runtime()?.block_on(send(&session, &text, submit)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::{AgentKind, ProjectId, WorkspaceId, WorktreeId};
    use std::path::PathBuf;

    fn agent(id: &str, name: &str, status: AgentStatus, unseen: bool, archived: bool) -> Agent {
        Agent {
            id: AgentId(id.into()),
            worktree_id: WorktreeId("wt".into()),
            name: name.into(),
            status,
            archived,
            archived_at: 0,
            unseen,
            status_changed_at: 7,
            kind: AgentKind::default(),
            model: None,
            effort: None,
            session_id: None,
            cloud_session_id: None,
            sort_order: 0,
            alive: true,
            cloud_mirroring: false,
            suggested_prompt: None,
            provider_error: None,
        }
    }

    fn world() -> (Vec<Workspace>, Vec<Project>, Vec<Worktree>) {
        (
            vec![Workspace {
                id: WorkspaceId("default".into()),
                name: "Main".into(),
            }],
            vec![Project {
                id: ProjectId("p".into()),
                name: "api".into(),
                workspace_id: WorkspaceId("default".into()),
                repo_path: PathBuf::from("/repo"),
                sort_order: 0,
                pin_order: None,
            }],
            vec![Worktree {
                id: WorktreeId("wt".into()),
                project_id: ProjectId("p".into()),
                path: PathBuf::from("/repo"),
                branch: "main".into(),
                is_main: true,
                sort_order: 0,
            }],
        )
    }

    #[test]
    fn session_row_has_the_stable_fields() {
        let (ws, ps, wts) = world();
        let agents = vec![agent(
            "a1",
            "fix login",
            AgentStatus::NeedsFeedback,
            false,
            false,
        )];
        let doc = sessions_document(&ws, &ps, &wts, &agents, false, false);
        assert_eq!(doc["ok"], true);
        assert_eq!(
            doc["sessions"][0],
            json!({
                "id": "a1",
                "name": "fix login",
                "status": "needs_feedback",
                "attention": "needs_feedback",
                "alive": true,
                "archived": false,
                "kind": AgentKind::default().as_str(),
                "model": null,
                "status_changed_at": 7,
                "workspace": "Main",
                "project": "api",
                "branch": "main",
                "path": PathBuf::from("/repo").display().to_string(),
            })
        );
    }

    #[test]
    fn attention_lists_waiting_and_unseen_but_not_archived_or_seen() {
        let (ws, ps, wts) = world();
        let agents = vec![
            agent("a1", "waiting", AgentStatus::NeedsFeedback, false, false),
            agent("a2", "unseen", AgentStatus::Finished, true, false),
            agent("a3", "seen", AgentStatus::Finished, false, false),
            agent("a4", "busy", AgentStatus::Running, false, false),
            agent("a5", "old", AgentStatus::NeedsFeedback, false, true),
        ];
        let doc = sessions_document(&ws, &ps, &wts, &agents, true, true);
        let rows = doc["sessions"].as_array().unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["a1", "a2"]);
        assert_eq!(rows[1]["attention"], "unseen");
    }

    #[test]
    fn sessions_hide_archived_unless_all() {
        let (ws, ps, wts) = world();
        let agents = vec![
            agent("a1", "x", AgentStatus::Fresh, false, false),
            agent("a2", "y", AgentStatus::Fresh, false, true),
        ];
        let live = sessions_document(&ws, &ps, &wts, &agents, false, false);
        assert_eq!(live["sessions"].as_array().unwrap().len(), 1);
        let all = sessions_document(&ws, &ps, &wts, &agents, true, false);
        assert_eq!(all["sessions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn resolve_prefers_id_then_name_then_prefix_and_rejects_ambiguity() {
        let agents = vec![
            agent("01AAA", "fix", AgentStatus::Fresh, false, false),
            agent("01AAB", "fix", AgentStatus::Fresh, false, false),
            agent("01BBB", "docs", AgentStatus::Fresh, false, false),
        ];
        assert_eq!(
            resolve_session(&agents, "01AAB").unwrap().id.as_str(),
            "01AAB"
        );
        assert_eq!(
            resolve_session(&agents, "docs").unwrap().id.as_str(),
            "01BBB"
        );
        assert_eq!(
            resolve_session(&agents, "01BB").unwrap().id.as_str(),
            "01BBB"
        );
        let err = resolve_session(&agents, "fix").unwrap_err().to_string();
        assert!(err.contains("01AAA") && err.contains("01AAB"), "{err}");
        assert!(resolve_session(&agents, "01A").is_err());
        assert!(resolve_session(&agents, "nope").is_err());
    }

    #[test]
    fn prompt_is_a_bracketed_paste_then_enter_and_cannot_break_out() {
        let chunks = prompt_chunks("hi\nthere\u{1b}[201~x", true);
        assert_eq!(chunks[0], b"\x1b[200~hi\nthere[201~x\x1b[201~".to_vec());
        assert_eq!(chunks[1], b"\r".to_vec());
        assert_eq!(prompt_chunks("hi", false).len(), 1);
    }

    #[test]
    fn failure_is_json_with_ok_false() {
        let v = failure_json(&anyhow::anyhow!("boom"));
        assert_eq!(v, json!({ "ok": false, "error": "boom" }));
    }
}
