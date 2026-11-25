use super::providers::AcpProvider;
use super::wire::{Host, LaunchSpec, Process, Wire, WireIn};
use crate::hooks::HookEnv;
use crate::registry::Daemon;
use crate::store::Store;
use anyhow::{bail, ensure, Context, Result};
use berth_core::acp::{
    AcpSessionState, Answers, AttachmentUpload, FeedbackAnswer, QueueId, ACP_DRIVER,
};
use berth_core::{AgentId, EntityId, ServerEvent, WorktreeId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const ANY: &str = "<any>";
const WORKSPACE: &str = "<workspace>";
const OUTBOUND_WAIT: Duration = Duration::from_secs(30);
const IDLE_PROBE: Duration = Duration::from_millis(1);
const BLESS_VAR: &str = "BERTH_REPLAY_BLESS";

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Step {
    TranscriptStart {
        provider: String,
        #[serde(default)]
        driver: Option<String>,
    },
    Berth {
        call: Call,
    },
    ExpectOutbound {
        frame: Value,
    },
    EmitInbound {
        frame: Value,
    },
    RuntimeExit {
        code: Option<i32>,
    },
    Wait {
        ms: u64,
    },
}

#[derive(Deserialize)]
#[serde(tag = "call", content = "args", rename_all = "snake_case")]
enum Call {
    Create {
        name: String,
    },
    Prompt {
        text: String,
        #[serde(default)]
        attachments: Vec<AttachmentUpload>,
    },
    Answer {
        option: Option<u8>,
        form: Option<Answers>,
    },
    SetOption {
        id: String,
        value: String,
    },
    EditQueued {
        id: u64,
        text: String,
    },
    RemoveQueued {
        id: u64,
    },
    Stop,
    Resume,
    RefreshMcp,
    Rewind {
        to: Option<String>,
    },
}

struct Peer {
    to_berth: mpsc::UnboundedSender<WireIn>,
    from_berth: mpsc::UnboundedReceiver<String>,
}

#[derive(Default)]
struct ReplayHost {
    peer: Mutex<Option<Peer>>,
}

struct ReplayProcess {
    to_berth: mpsc::UnboundedSender<WireIn>,
}

impl Process for ReplayProcess {
    fn kill(&self) {
        let _ = self.to_berth.send(WireIn::Exited(None));
    }

    fn stderr_tail(&self) -> Vec<String> {
        Vec::new()
    }
}

impl Host for ReplayHost {
    fn spawn(&self, _spec: LaunchSpec) -> Result<Wire> {
        let (to_berth, inbound) = mpsc::unbounded_channel();
        let (outbound, from_berth) = mpsc::unbounded_channel();
        *self.peer.lock().unwrap() = Some(Peer {
            to_berth: to_berth.clone(),
            from_berth,
        });
        Ok(Wire {
            inbound,
            outbound,
            process: Arc::new(ReplayProcess { to_berth }),
        })
    }
}

struct Run {
    daemon: Arc<Daemon>,
    host: Arc<ReplayHost>,
    events: broadcast::Receiver<ServerEvent>,
    worktree: WorktreeId,
    workspace: String,
    provider: Option<AcpProvider>,
    agent: Option<AgentId>,
    peer: Option<Peer>,
}

fn git(repo: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
}

fn hide_blobs(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(hide_blobs),
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                match field {
                    Value::String(_) if key == "blob" => *field = json!("<blob>"),
                    _ => hide_blobs(field),
                }
            }
        }
        _ => {}
    }
}

fn map_strings(value: &mut Value, change: &impl Fn(&str) -> String) {
    match value {
        Value::String(text) => *text = change(text),
        Value::Array(items) => items.iter_mut().for_each(|v| map_strings(v, change)),
        Value::Object(fields) => fields.values_mut().for_each(|v| map_strings(v, change)),
        _ => {}
    }
}

fn matches(expected: &Value, actual: &Value, workspace: &str) -> bool {
    match (expected, actual) {
        (Value::String(e), _) if e == ANY => true,
        (Value::String(e), Value::String(a)) => e.replace(WORKSPACE, workspace) == *a,
        (Value::Object(e), Value::Object(a)) => e
            .iter()
            .all(|(key, v)| a.get(key).is_some_and(|x| matches(v, x, workspace))),
        (Value::Array(e), Value::Array(a)) => {
            e.len() == a.len() && e.iter().zip(a).all(|(e, a)| matches(e, a, workspace))
        }
        _ => expected == actual,
    }
}

fn session_summary(state: &AcpSessionState) -> Value {
    let mut summary = json!({
        "caps": state.caps,
        "options": state.options.iter().map(|option| json!({
            "id": option.id,
            "category": option.category,
            "current": option.current,
            "choices": option.choices.len(),
        })).collect::<Vec<_>>(),
        "commands": state.commands.iter().map(|command| &command.name).collect::<Vec<_>>(),
        "queue": state.queue.iter().map(|queued| json!({"id": queued.id, "text": queued.prompt.text})).collect::<Vec<_>>(),
    });
    if state.mcp.iter().any(|server| server.status.is_some()) {
        summary["mcp"] = json!(state.mcp);
    }
    summary
}

fn item_seq(observed: &Value) -> Option<u64> {
    observed.pointer("/item/seq")?.as_u64()
}

pub(super) async fn daemon_in(tmp: &Path) -> Result<(Arc<Daemon>, WorktreeId, String)> {
    let repo = berth_core::paths::canonical_or_raw(tmp).join("repo");
    std::fs::create_dir_all(&repo)?;
    git(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n")?;
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let daemon = Daemon::new(
        Arc::new(Store::open_in_memory()?),
        HookEnv {
            port: 0,
            token: String::new(),
        },
    );
    let EntityId::Project(project) = daemon.add_project(&repo, None, false, None).await? else {
        bail!("expected a project id");
    };
    let (_, worktrees, _, _) = daemon.store.load_tree()?;
    let worktree = worktrees
        .into_iter()
        .find(|w| w.project_id == project)
        .context("the project has no worktree")?;
    Ok((daemon, worktree.id, worktree.path.display().to_string()))
}

impl Run {
    async fn new(tmp: &Path) -> Result<Run> {
        let (daemon, worktree, workspace) = daemon_in(tmp).await?;
        daemon.acp.blobs.move_to(tmp.join("blobs"));
        Ok(Run {
            events: daemon.events.subscribe(),
            workspace,
            worktree,
            daemon,
            host: Default::default(),
            provider: None,
            agent: None,
            peer: None,
        })
    }

    async fn settle(&self) {
        tokio::time::pause();
        tokio::time::sleep(IDLE_PROBE).await;
        tokio::time::resume();
    }

    fn agent(&self) -> Result<&AgentId> {
        self.agent.as_ref().context("no agent created yet")
    }

    fn take_peer(&mut self) {
        if let Some(peer) = self.host.peer.lock().unwrap().take() {
            self.peer = Some(peer);
        }
    }

    async fn berth(&mut self, call: Call) -> Result<()> {
        match call {
            Call::Create { name } => {
                let provider = self.provider.clone().context("no transcript_start")?;
                let id = AgentId::generate();
                self.daemon.create_acp_agent_with(
                    id.clone(),
                    self.worktree.clone(),
                    name,
                    provider,
                    false,
                    self.host.clone(),
                )?;
                self.agent = Some(id);
                self.take_peer();
            }
            Call::Prompt { text, attachments } => {
                self.daemon
                    .acp_prompt_with(self.agent()?, &text, attachments)
                    .await?
            }
            Call::Answer { option, form } => {
                let answer = match (option, form) {
                    (Some(option), _) => FeedbackAnswer::Option(option),
                    (None, Some(form)) => FeedbackAnswer::Form(form),
                    (None, None) => bail!("an answer needs an option or a form"),
                };
                self.daemon.answer_feedback(self.agent()?, answer).await?
            }
            Call::SetOption { id, value } => {
                let daemon = self.daemon.clone();
                let agent = self.agent()?.clone();
                tokio::spawn(async move { daemon.acp_set_option(&agent, &id, &value).await });
            }
            Call::EditQueued { id, text } => {
                self.daemon
                    .acp_edit_queued(self.agent()?, QueueId(id), &text)?
            }
            Call::RemoveQueued { id } => {
                self.daemon.acp_remove_queued(self.agent()?, QueueId(id))?
            }
            Call::Stop => {
                self.daemon
                    .stop_agent_or_its_acp_turn(self.agent()?)
                    .await?
            }
            Call::Rewind { to } => {
                self.daemon.acp_rewind(self.agent()?, to).await?;
            }
            Call::RefreshMcp => {
                let daemon = self.daemon.clone();
                let agent = self.agent()?.clone();
                tokio::spawn(async move { daemon.acp_refresh_mcp(&agent).await });
            }
            Call::Resume => {
                let agent = self.agent()?.clone();
                let session = self
                    .daemon
                    .store
                    .get_agent(&agent)?
                    .and_then(|a| a.session_id);
                let provider = self.provider.clone().context("no transcript_start")?;
                let cwd = PathBuf::from(&self.workspace);
                self.daemon
                    .acp_start(&agent, &cwd, &provider, session, self.host.clone())?;
                self.take_peer();
            }
        }
        Ok(())
    }

    async fn expect_outbound(&mut self, expected: &Value) -> Result<()> {
        let peer = self.peer.as_mut().context("no agent process")?;
        let line = tokio::time::timeout(OUTBOUND_WAIT, peer.from_berth.recv())
            .await
            .with_context(|| format!("berth sent nothing; expected {expected}"))?
            .context("berth closed the wire")?;
        let actual: Value = serde_json::from_str(&line)?;
        ensure!(
            matches(expected, &actual, &self.workspace),
            "outbound frame differs\n  expected {expected}\n  actual   {actual}"
        );
        Ok(())
    }

    fn emit_inbound(&self, mut frame: Value) -> Result<()> {
        map_strings(&mut frame, &|s| s.replace(WORKSPACE, &self.workspace));
        let peer = self.peer.as_ref().context("no agent process")?;
        peer.to_berth.send(WireIn::line(frame.to_string()))?;
        Ok(())
    }

    fn observed(&mut self) -> Result<Vec<Value>> {
        let agent = self.agent()?.clone();
        let mut out: Vec<Value> = Vec::new();
        loop {
            let event = match self.events.try_recv() {
                Ok(event) => event,
                Err(broadcast::error::TryRecvError::Lagged(missed)) => {
                    bail!("the event stream lagged by {missed}")
                }
                Err(_) => break,
            };
            let mut seen = match event {
                ServerEvent::AcpItem {
                    agent: a,
                    seq,
                    item,
                } if a == agent => json!({"item": {"seq": seq, "item": item}}),
                ServerEvent::StatusChanged {
                    agent: a, status, ..
                } if a == agent => json!({"status": status}),
                ServerEvent::FeedbackRequestChanged { agent: a, request } if a == agent => {
                    json!({"feedback": request})
                }
                ServerEvent::AcpSessionChanged { agent: a, state } if a == agent => {
                    json!({"session": session_summary(&state)})
                }
                _ => continue,
            };
            map_strings(&mut seen, &|s| s.replace(&self.workspace, WORKSPACE));
            hide_blobs(&mut seen);
            match (item_seq(&seen), out.last_mut()) {
                (Some(seq), Some(last)) if item_seq(last) == Some(seq) => *last = seen,
                _ => out.push(seen),
            }
        }
        Ok(out)
    }

    async fn play(mut self, steps: Vec<Step>) -> Result<Vec<Value>> {
        for step in steps {
            match step {
                Step::TranscriptStart { provider, driver } => {
                    self.provider = Some(AcpProvider {
                        id: provider.clone(),
                        name: provider.clone(),
                        command: provider,
                        args: Vec::new(),
                        driver: driver.unwrap_or_else(|| ACP_DRIVER.to_string()),
                        enabled: true,
                    })
                }
                Step::Berth { call } => {
                    self.settle().await;
                    self.berth(call).await?;
                    self.settle().await;
                    self.take_peer();
                }
                Step::ExpectOutbound { frame } => self.expect_outbound(&frame).await?,
                Step::EmitInbound { frame } => self.emit_inbound(frame)?,
                Step::Wait { ms } => {
                    tokio::time::pause();
                    tokio::time::advance(Duration::from_millis(ms)).await;
                    tokio::time::resume();
                }
                Step::RuntimeExit { code } => {
                    let peer = self.peer.as_ref().context("no agent process")?;
                    peer.to_berth.send(WireIn::Exited(code))?;
                }
            }
        }
        self.settle().await;
        if let Some(peer) = self.peer.as_mut() {
            if let Ok(line) = peer.from_berth.try_recv() {
                bail!("berth sent a frame the fixture does not expect: {line}");
            }
        }
        let observed = self.observed();
        self.daemon.acp.kill_all();
        observed
    }
}

fn fixtures_in(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            fixtures_in(&path, found);
        } else if path.extension().is_some_and(|e| e == "ndjson") {
            found.push(path);
        }
    }
}

fn lines_of(path: &Path) -> Result<Vec<Value>> {
    std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

async fn replay(fixture: &Path) -> Result<()> {
    let steps = lines_of(fixture)?
        .into_iter()
        .map(|line| Ok(serde_json::from_value(line)?))
        .collect::<Result<Vec<Step>>>()?;
    let tmp = tempfile::tempdir()?;
    let observed = Run::new(tmp.path()).await?.play(steps).await?;
    let golden = fixture.with_extension("observed.jsonl");
    if std::env::var_os(BLESS_VAR).is_some() {
        let text: String = observed.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(&golden, text)?;
        return Ok(());
    }
    let expected = lines_of(&golden)?;
    let at = (0..observed.len().max(expected.len())).find(|&i| observed.get(i) != expected.get(i));
    if let Some(i) = at {
        bail!(
            "broadcasts differ from {} at line {}\n  expected {:?}\n  actual   {:?}",
            golden.display(),
            i + 1,
            expected.get(i),
            observed.get(i)
        );
    }
    Ok(())
}

#[tokio::test]
async fn every_fixture_replays() {
    let mut found = Vec::new();
    fixtures_in(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/acp"),
        &mut found,
    );
    assert!(!found.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    for fixture in found {
        if let Err(error) = replay(&fixture).await {
            failures.push(format!("{}: {error:#}", fixture.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
