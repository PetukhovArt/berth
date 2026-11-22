//! DAEMON SOCKET server: accept loop, per-client request handling, PTY
//! attach/forward plumbing. The stream type and how a client is authorized
//! are the transport's business (`berth_core::transport`) — this module
//! sees one shape on every platform.

use crate::pty::{PtyEvent, PtySession};
use crate::registry::{CreateAgentSpec, Daemon};
use anyhow::Result;
use berth_core::codec::{read_frame, write_frame};
use berth_core::transport::{Authorizer, Listener, Stream};
use berth_core::{ClientRequest, ServerEvent, SessionRef, WorkspaceId, PROTOCOL_VERSION};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::mpsc;

pub async fn accept_loop(daemon: Arc<Daemon>, listener: Listener) {
    // Cloned once and moved into every served task: the authorization check
    // must not run on the accept loop, or one client that connects and then
    // says nothing stalls every other client behind it.
    let authorizer = listener.authorizer();
    loop {
        tokio::select! {
            _ = daemon.shutdown.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let daemon = daemon.clone();
                    let authorizer = authorizer.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(daemon, stream, authorizer).await {
                            tracing::debug!(error = %e, "client connection ended with error");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "accept failed");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            },
        }
    }
}

async fn handle_client(
    daemon: Arc<Daemon>,
    mut stream: Stream,
    authorizer: Authorizer,
) -> Result<()> {
    // Ahead of `Hello`: on Unix a no-op, on Windows the bearer-token frame.
    // A client that fails it is never handed a protocol version, so a wrong
    // token and a version skew stay distinguishable.
    authorizer.authorize(&mut stream).await?;
    let (read_half, write_half) = stream.into_split();
    let mut reader = tokio::io::BufReader::new(read_half);

    // Single writer task; everything else sends frames through this channel
    // so PTY forwards and RPC replies never interleave mid-frame.
    let (out_tx, mut out_rx) = mpsc::channel::<ServerEvent>(256);
    let writer_task = tokio::spawn(async move {
        let mut w = BufWriter::new(write_half);
        while let Some(ev) = out_rx.recv().await {
            if write_frame(&mut w, &ev).await.is_err() {
                break;
            }
        }
        let _ = w.shutdown().await;
    });

    // Per-connection attach state: forward-task handles keyed by session.
    let mut attached: HashMap<SessionRef, tokio::task::JoinHandle<()>> = HashMap::new();
    let mut handshaken = false;
    // Which workspace THIS client is scoped to. Per-connection on purpose:
    // two berth instances are two independent views, so one switching
    // workspaces must not move the other. Pinned at Subscribe to whatever
    // the client was handed to boot into, because "the current default" is
    // not a stable answer — another instance switching moves it, and a
    // client that read it once must not silently follow. `None` outlives
    // Subscribe only for connections that never subscribe: the one-shot
    // `berth add`, whose workspace genuinely is the current default.
    let mut workspace: Option<WorkspaceId> = None;
    let mut _transcription_offer: Option<crate::messenger::transcribers::Offer> = None;

    let result: Result<()> = async {
        while let Some(req) = read_frame::<ClientRequest, _>(&mut reader).await? {
            match req {
                ClientRequest::Hello { protocol_version } => {
                    handshaken = protocol_version == PROTOCOL_VERSION;
                    let reply = if handshaken {
                        ServerEvent::HelloOk {
                            protocol_version: PROTOCOL_VERSION,
                            daemon_pid: std::process::id(),
                        }
                    } else {
                        ServerEvent::Incompatible {
                            daemon_protocol_version: PROTOCOL_VERSION,
                        }
                    };
                    let closing = !handshaken;
                    let _ = out_tx.send(reply).await;
                    if closing {
                        break;
                    }
                }
                ClientRequest::Shutdown => {
                    tracing::info!("shutdown requested by client");
                    daemon.shutdown.cancel();
                    break;
                }
                _ if !handshaken => {
                    let _ = out_tx
                        .send(ServerEvent::Error {
                            req_id: None,
                            message: "handshake required".into(),
                        })
                        .await;
                    break;
                }
                ClientRequest::Subscribe => {
                    let snapshot = daemon.snapshot().unwrap_or(ServerEvent::Snapshot {
                        workspaces: vec![],
                        active_workspace: Default::default(),
                        projects: vec![],
                        worktrees: vec![],
                        agents: vec![],
                        terminals: vec![],
                        archived_external_sessions: vec![],
                    });
                    // Scope this client to the workspace it is being shown.
                    // First Subscribe only — a re-subscribe must not undo a
                    // switch the client made in between.
                    if workspace.is_none() {
                        if let ServerEvent::Snapshot {
                            active_workspace, ..
                        } = &snapshot
                        {
                            workspace = Some(active_workspace.clone());
                        }
                    }
                    let _ = out_tx.send(snapshot).await;
                    let mut rx = daemon.events.subscribe();
                    for (agent, request) in daemon.feedback.all() {
                        let _ = out_tx
                            .send(ServerEvent::FeedbackRequestChanged {
                                agent,
                                request: Some(request),
                            })
                            .await;
                    }
                    let tx = out_tx.clone();
                    tokio::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(ev) => {
                                    if tx.send(ev).await.is_err() {
                                        break;
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                    continue
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    });
                }
                ClientRequest::Attach {
                    session: sref,
                    from_seq,
                    cols,
                    rows,
                } => {
                    let installed = daemon.installed_sessions.subscribe();
                    match daemon.ensure_session(&sref, cols, rows) {
                        Ok(session) => {
                            // Subscribe BEFORE snapshotting so nothing falls in
                            // the gap; the forward task drops frames the
                            // snapshot already covers.
                            let events_rx = session.events.subscribe();
                            let (base_seq, data) = session.snapshot(from_seq);
                            let replay_end = base_seq + data.len() as u64;
                            let _ = out_tx
                                .send(ServerEvent::Scrollback {
                                    session: sref.clone(),
                                    base_seq,
                                    data,
                                })
                                .await;
                            let _ = session.resize_with_jiggle(cols, rows);

                            let rebind = attached.remove(&sref);
                            if let Some(old) = &rebind {
                                old.abort();
                            }
                            let handle = tokio::spawn(forward_pty(
                                daemon.clone(),
                                session.clone(),
                                events_rx,
                                installed,
                                out_tx.clone(),
                                replay_end,
                            ));
                            // Count this connection once even across
                            // re-attaches to the same session.
                            if rebind.is_none() {
                                daemon.note_attached(&sref);
                            }
                            attached.insert(sref, handle);
                        }
                        Err(e) => {
                            let _ = out_tx
                                .send(ServerEvent::Error {
                                    req_id: None,
                                    message: format!("attach: {e:#}"),
                                })
                                .await;
                        }
                    }
                }
                ClientRequest::Detach { session } => {
                    if let Some(h) = attached.remove(&session) {
                        h.abort();
                        daemon.note_detached(&session);
                    }
                }
                ClientRequest::Input { session, data } => {
                    if let Some(s) = daemon.session(&session) {
                        if let Err(e) = s.write_input(&data) {
                            tracing::warn!(error = %e, "pty write failed");
                        } else if let SessionRef::Agent(agent) = &session {
                            daemon.note_typed_into_agent(agent, &data);
                        }
                    }
                }
                ClientRequest::Resize {
                    session,
                    cols,
                    rows,
                } => {
                    if let Some(s) = daemon.session(&session) {
                        let _ = s.resize(cols, rows);
                    }
                }
                ClientRequest::MarkAgentSeen { id } => {
                    if let Err(e) = daemon.mark_agent_seen(&id) {
                        tracing::warn!(error = %e, "mark agent seen failed");
                    }
                }
                ClientRequest::GetMetrics { req_id } => {
                    // The sampler measures; this only reads what it last
                    // published. Sweeping here would date every CPU delta
                    // from whichever client asked last — see `crate::sampler`.
                    //
                    // Always answer, even before the first tick: the client
                    // frees its pending slot on the reply, so silence leaks
                    // one slot per poll. `unsampled()` is the "nothing
                    // measured yet" reply it knows to skip.
                    let snapshot = daemon
                        .last_sample()
                        .unwrap_or_else(berth_core::MetricsSnapshot::unsampled);
                    let _ = out_tx.send(ServerEvent::Metrics { req_id, snapshot }).await;
                }
                ClientRequest::GetUsageLimits { req_id } => {
                    let windows = tokio::task::spawn_blocking(crate::usage_limits::read)
                        .await
                        .unwrap_or_default();
                    let _ = out_tx
                        .send(ServerEvent::UsageLimits { req_id, windows })
                        .await;
                }
                ClientRequest::GetTaskNote { req_id, worktree } => {
                    let ev = match daemon.store.task_note(&worktree) {
                        Ok(text) => ServerEvent::TaskNote {
                            req_id,
                            worktree,
                            text,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::GetProjectSetup { req_id, project } => {
                    let ev = match daemon.store.project_setup(&project) {
                        Ok(command) => ServerEvent::ProjectSetup {
                            req_id,
                            project,
                            command,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::SetProjectSetup {
                    req_id,
                    project,
                    command,
                } => {
                    reply_done(
                        &out_tx,
                        req_id,
                        daemon.set_project_setup(&project, &command),
                    )
                    .await;
                }
                ClientRequest::TestChatBridge { req_id } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        reply_done(&out_tx, req_id, crate::messenger::send_test().await).await;
                    });
                }
                ClientRequest::OfferTranscription => {
                    _transcription_offer =
                        Some(crate::messenger::transcribers::offer(out_tx.clone()));
                }
                ClientRequest::VoiceTranscribed {
                    request,
                    text,
                    error,
                } => {
                    crate::messenger::transcribers::answer(
                        request,
                        text.ok_or_else(|| error.unwrap_or_default()),
                    );
                }
                ClientRequest::ListTriggers { req_id } => {
                    let ev = match daemon.store.list_triggers() {
                        Ok(triggers) => ServerEvent::Triggers { req_id, triggers },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::CreateTrigger { req_id, trigger } => {
                    let ev = match daemon.create_trigger(trigger) {
                        Ok((trigger, secret)) => ServerEvent::TriggerCreated {
                            req_id,
                            trigger,
                            secret,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::DeleteTrigger { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.delete_trigger(&id)).await;
                }
                ClientRequest::ListChannels { req_id } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channels::list_channels().await {
                            Ok(channels) => ServerEvent::Channels { req_id, channels },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::GetChannelPresence { req_id, channel } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channels::channel_presence(&channel).await {
                            Ok(here) => ServerEvent::ChannelPresence { req_id, here },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::OpenChannel {
                    req_id,
                    key,
                    agent,
                    shared,
                } => {
                    let out_tx = out_tx.clone();
                    let daemon = daemon.clone();
                    tokio::spawn(async move {
                        let ev = match daemon.open_channel(key, agent, shared).await {
                            Ok(key) => ServerEvent::ChannelOpened { req_id, key },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::LeaveChannel { req_id, key } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let done = crate::channels::leave_channel(&key).await;
                        reply_done(&out_tx, req_id, done).await;
                    });
                }
                ClientRequest::DeleteChannel { req_id, key } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let done = crate::channels::delete_channel(&key).await;
                        reply_done(&out_tx, req_id, done).await;
                    });
                }
                ClientRequest::GetChannelTouched { req_id, channel } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channels::channel_touched(&channel).await {
                            Ok(touched) => ServerEvent::ChannelTouched { req_id, touched },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::PostChannelNote {
                    req_id,
                    channel,
                    content,
                    agent,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channels::post_channel_note(
                            &store, &channel, &content, &agent,
                        )
                        .await
                        {
                            Ok(event) => ServerEvent::ChannelEvents {
                                req_id,
                                events: vec![event],
                            },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::PromoteChannelMessage {
                    req_id,
                    channel,
                    seq,
                    agent,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let done = crate::team_access::promote(&store, &channel, seq, agent).await;
                        reply_done(&out_tx, req_id, done).await;
                    });
                }
                ClientRequest::SetChannelGrant {
                    req_id,
                    channel,
                    agent,
                    member,
                    granted,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let result = crate::team_access::set_grant(
                            &store, &channel, &agent, &member, granted,
                        )
                        .await;
                        let _ = out_tx
                            .send(crate::team_access::grants_reply(req_id, result))
                            .await;
                    });
                }
                ClientRequest::ListChannelGrants { req_id, channel } => {
                    let result = crate::team_access::list_grants(&daemon.store, channel.as_deref());
                    let _ = out_tx
                        .send(crate::team_access::grants_reply(req_id, result))
                        .await;
                }
                ClientRequest::GetHub { req_id } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let result = crate::hub_admin::view().await;
                        let _ = out_tx.send(crate::hub_admin::reply(req_id, result)).await;
                    });
                }
                ClientRequest::ChangeHub { req_id, change } => {
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let result = crate::hub_admin::change(&change).await;
                        let _ = out_tx.send(crate::hub_admin::reply(req_id, result)).await;
                    });
                }
                ClientRequest::CreateAcpAgent {
                    req_id,
                    worktree,
                    name,
                    provider,
                    auto_title,
                } => {
                    let result = daemon
                        .create_acp_agent(worktree, name, &provider, auto_title)
                        .await;
                    reply(&out_tx, req_id, result.map(Some)).await;
                }
                ClientRequest::AcpPrompt {
                    req_id,
                    agent,
                    text,
                } => {
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        reply_done(&out_tx, req_id, daemon.acp_prompt(&agent, &text).await).await;
                    });
                }
                ClientRequest::AcpTranscript {
                    req_id,
                    agent,
                    after,
                } => {
                    let ev = match daemon.acp_transcript(&agent, after) {
                        Ok(items) => ServerEvent::AcpTranscript {
                            req_id,
                            agent,
                            items,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::AcpCommands { req_id, agent } => {
                    let commands = daemon.acp_commands(&agent);
                    let _ = out_tx
                        .send(ServerEvent::AcpCommands {
                            req_id,
                            agent,
                            commands,
                        })
                        .await;
                }
                ClientRequest::AnswerFeedback {
                    req_id,
                    agent,
                    option,
                } => {
                    reply_done(
                        &out_tx,
                        req_id,
                        daemon.answer_feedback(&agent, option).await,
                    )
                    .await;
                }
                ClientRequest::AnswerInChannel {
                    req_id,
                    channel,
                    request,
                    answer,
                    agent,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channel_requests::answer_in_channel(
                            &store, &channel, request, &answer, &agent,
                        )
                        .await
                        {
                            Ok(event) => ServerEvent::ChannelEvents {
                                req_id,
                                events: vec![event],
                            },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::ReadChannel {
                    req_id,
                    channel,
                    after,
                    agent,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev =
                            match crate::channels::read_channel(&store, &channel, after, &agent)
                                .await
                            {
                                Ok(events) => ServerEvent::ChannelEvents { req_id, events },
                                Err(e) => ServerEvent::Error {
                                    req_id: Some(req_id),
                                    message: format!("{e:#}"),
                                },
                            };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::PostToChannel {
                    req_id,
                    channel,
                    content,
                    agent,
                } => {
                    let store = daemon.store.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match crate::channels::post_to_channel(
                            &store, &channel, &content, &agent,
                        )
                        .await
                        {
                            Ok(event) => ServerEvent::ChannelEvents {
                                req_id,
                                events: vec![event],
                            },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::ListCheckpoints { req_id, agent } => {
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match daemon.list_checkpoints(&agent).await {
                            Ok(checkpoints) => ServerEvent::Checkpoints {
                                req_id,
                                agent,
                                checkpoints,
                            },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::RestoreCheckpoint {
                    req_id,
                    agent,
                    number,
                } => {
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let done = daemon.restore_checkpoint(&agent, number).await;
                        reply_done(&out_tx, req_id, done).await;
                    });
                }
                ClientRequest::SetTaskNote {
                    req_id,
                    worktree,
                    text,
                } => {
                    reply_done(&out_tx, req_id, daemon.set_task_note(&worktree, &text)).await;
                }
                // ---- entity CRUD: run the op, reply Ack/Error ----
                ClientRequest::AddWorkspace { req_id, name } => {
                    reply(&out_tx, req_id, daemon.add_workspace(&name).map(Some)).await;
                }
                ClientRequest::RemoveWorkspace { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.remove_workspace(&id)).await;
                }
                ClientRequest::RenameWorkspace { req_id, id, name } => {
                    reply_done(&out_tx, req_id, daemon.rename_workspace(&id, &name)).await;
                }
                ClientRequest::OpenWorkspace { req_id, id } => {
                    // Scope this connection, and leave the pick behind as the
                    // default a fresh client boots into. A workspace that
                    // doesn't exist scopes nothing.
                    let result = daemon.set_default_workspace(&id);
                    if result.is_ok() {
                        workspace = Some(id);
                    }
                    reply_done(&out_tx, req_id, result).await;
                }
                ClientRequest::AddProject {
                    req_id,
                    path,
                    name,
                    create_missing,
                } => {
                    reply(
                        &out_tx,
                        req_id,
                        daemon
                            .add_project(&path, name, create_missing, workspace.clone())
                            .await
                            .map(Some),
                    )
                    .await;
                }
                ClientRequest::RemoveProject { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.remove_project(&id)).await;
                }
                ClientRequest::RenameProject { req_id, id, name } => {
                    reply(
                        &out_tx,
                        req_id,
                        daemon.rename_project(&id, &name).map(|_| None),
                    )
                    .await;
                }
                ClientRequest::SetProjectPins { req_id, order } => {
                    reply(
                        &out_tx,
                        req_id,
                        daemon.set_project_pins(&order).map(|_| None),
                    )
                    .await;
                }
                ClientRequest::CreateWorktree {
                    req_id,
                    project,
                    branch,
                    base,
                } => {
                    reply(
                        &out_tx,
                        req_id,
                        daemon
                            .create_worktree(&project, &branch, base.as_deref())
                            .await
                            .map(Some),
                    )
                    .await;
                }
                ClientRequest::DeleteWorktree { req_id, id, force } => {
                    // `git worktree remove` can take seconds on a large
                    // checkout; run it off the request loop so Input/Attach
                    // frames keep flowing while it grinds. `worktree_ops`
                    // still serializes it against create/sync.
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        reply_done(&out_tx, req_id, daemon.delete_worktree(&id, force).await).await;
                    });
                }
                ClientRequest::CreateAgent {
                    req_id,
                    worktree,
                    name,
                    kind,
                    model,
                    effort,
                    auto_title,
                    starting_prompt,
                } => {
                    // Logged by mode only — never the task or prompt text.
                    let launch_mode = starting_prompt.is_some().then_some("preset");
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let providers = crate::config::Config::load().providers;
                        let result =
                            match crate::acp::providers::ensure_native_enabled(&providers, kind) {
                                Err(off) => Err(off),
                                Ok(()) => {
                                    daemon
                                        .create_agent(CreateAgentSpec {
                                            worktree: worktree.clone(),
                                            name,
                                            kind,
                                            model,
                                            effort,
                                            auto_title,
                                            starting_prompt,
                                        })
                                        .await
                                }
                            };
                        if let Some(launch_mode) = launch_mode {
                            match &result {
                                Ok(berth_core::EntityId::Agent(agent)) => tracing::info!(
                                    req_id,
                                    agent = %agent,
                                    kind = kind.as_str(),
                                    worktree = %worktree,
                                    launch_mode,
                                    "agent session spawned"
                                ),
                                Err(error) => tracing::warn!(
                                    req_id,
                                    error = %error,
                                    kind = kind.as_str(),
                                    worktree = %worktree,
                                    launch_mode,
                                    "agent session spawn failed"
                                ),
                                Ok(_) => unreachable!("CreateAgent returned a non-agent id"),
                            }
                        }
                        reply(&out_tx, req_id, result.map(Some)).await;
                    });
                }
                ClientRequest::PrewarmAgent {
                    worktree,
                    kind,
                    model,
                    effort,
                } => {
                    // Fire-and-forget: boot the CLI while the user is still
                    // typing the session name; CreateAgent adopts it. Runs
                    // off the request loop (the CLI probe can take a bit).
                    let ticket = daemon.prewarm.ticket();
                    let daemon = daemon.clone();
                    tokio::spawn(async move {
                        if let Err(e) = daemon
                            .prewarm_agent(ticket, &worktree, kind, model, effort)
                            .await
                        {
                            tracing::debug!(error = %e, "prewarm failed");
                        }
                    });
                }
                ClientRequest::RenameAgent { req_id, id, name } => {
                    reply_done(&out_tx, req_id, daemon.rename_agent(&id, &name)).await;
                }
                ClientRequest::AutoRenameAgent { req_id, id, name } => {
                    reply_done(&out_tx, req_id, daemon.auto_rename_agent(&id, &name)).await;
                }
                ClientRequest::MoveAgent {
                    req_id,
                    id,
                    worktree,
                } => {
                    reply_done(&out_tx, req_id, daemon.move_agent(&id, &worktree)).await;
                }
                ClientRequest::SpawnSiblingAgent {
                    req_id,
                    id,
                    kind,
                    starting_prompt,
                    worktree,
                } => {
                    // Logged by mode only — never the prompt text.
                    let result = daemon
                        .spawn_sibling_agent(&id, kind, &starting_prompt, worktree.as_ref())
                        .await;
                    match &result {
                        Ok(berth_core::EntityId::Agent(agent)) => tracing::info!(
                            req_id,
                            agent = %agent,
                            spawned_by = %id,
                            launch_mode = "sibling",
                            "agent session spawned"
                        ),
                        Err(error) => tracing::warn!(
                            req_id,
                            error = %error,
                            spawned_by = %id,
                            launch_mode = "sibling",
                            "agent session spawn failed"
                        ),
                        Ok(_) => unreachable!("SpawnSiblingAgent returned a non-agent id"),
                    }
                    reply(&out_tx, req_id, result.map(Some)).await;
                }
                ClientRequest::EnterWorktree {
                    req_id,
                    id,
                    branch,
                    base,
                } => {
                    let ev = match daemon.enter_worktree(&id, &branch, base.as_deref()).await {
                        Ok((worktree, outcome, warning)) => ServerEvent::WorktreeEntered {
                            req_id,
                            worktree,
                            outcome,
                            warning,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::EnterProject { req_id, id, path } => {
                    let ev = match daemon.enter_project(&id, &path).await {
                        Ok((worktree, outcome)) => ServerEvent::WorktreeEntered {
                            req_id,
                            worktree,
                            outcome,
                            warning: None,
                        },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::ListBranches { req_id, project } => {
                    // The reconcile behind it waits on `worktree_ops` and
                    // shells out to git; keep the request loop free.
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let ev = match daemon.list_branches(&project).await {
                            Ok(branches) => ServerEvent::Branches { req_id, branches },
                            Err(e) => ServerEvent::Error {
                                req_id: Some(req_id),
                                message: format!("{e:#}"),
                            },
                        };
                        let _ = out_tx.send(ev).await;
                    });
                }
                ClientRequest::ListOrphanedSessions {
                    req_id,
                    project,
                    include_live,
                } => {
                    let ev = match daemon.list_orphaned_sessions(&project, include_live) {
                        Ok(sessions) => ServerEvent::OrphanedSessions { req_id, sessions },
                        Err(e) => ServerEvent::Error {
                            req_id: Some(req_id),
                            message: format!("{e:#}"),
                        },
                    };
                    let _ = out_tx.send(ev).await;
                }
                ClientRequest::ResumeOrphanedSession {
                    req_id,
                    session_id,
                    worktree,
                } => {
                    // Probing the agent CLI and booting it takes as long as
                    // any create does; keep the request loop free.
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let created = daemon
                            .resume_orphaned_session(&session_id, &worktree)
                            .await
                            .map(Some);
                        reply(&out_tx, req_id, created).await;
                    });
                }
                ClientRequest::ResumeConversation {
                    req_id,
                    worktree,
                    kind,
                    session_id,
                    name,
                } => {
                    let daemon = daemon.clone();
                    let out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        let created = daemon
                            .resume_conversation(&worktree, kind, &session_id, &name)
                            .await
                            .map(Some);
                        reply(&out_tx, req_id, created).await;
                    });
                }
                ClientRequest::ArchiveSelf { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.archive_self(&id)).await;
                }
                ClientRequest::ArchiveAgent { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.archive_agent(&id)).await;
                }
                ClientRequest::UnarchiveAgent { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.unarchive_agent(&id)).await;
                }
                ClientRequest::StopAgent { req_id, id } => {
                    reply_done(
                        &out_tx,
                        req_id,
                        daemon.stop_agent_or_its_acp_turn(&id).await,
                    )
                    .await;
                }
                ClientRequest::ArchiveExternalSession {
                    req_id,
                    kind,
                    session_id,
                } => {
                    let done = daemon.archive_external_session(kind, &session_id);
                    reply_done(&out_tx, req_id, done).await;
                }
                ClientRequest::UnarchiveExternalSession {
                    req_id,
                    kind,
                    session_id,
                } => {
                    let done = daemon.unarchive_external_session(kind, &session_id);
                    reply_done(&out_tx, req_id, done).await;
                }
                ClientRequest::DeleteAgent { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.delete_agent(&id)).await;
                }
                ClientRequest::RestartAgent { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.restart_agent(&id).await).await;
                }
                ClientRequest::SwitchAgentModel { req_id, id, model } => {
                    let done = daemon.switch_agent_model(&id, model).await;
                    reply_done(&out_tx, req_id, done).await;
                }
                ClientRequest::ContinueAgentIn {
                    req_id,
                    id,
                    kind,
                    model,
                } => {
                    let result = daemon.continue_agent_in(&id, kind, model).await;
                    reply(&out_tx, req_id, result.map(Some)).await;
                }
                ClientRequest::GetRouterConfig { req_id } => {
                    let result = daemon.router_config();
                    reply_router_config(&out_tx, req_id, result).await;
                }
                ClientRequest::SaveRouterConfig { req_id, json } => {
                    let result = daemon.save_router_config(&json);
                    reply_router_config(&out_tx, req_id, result).await;
                }
                ClientRequest::CreateTerminal {
                    req_id,
                    worktree,
                    name,
                } => {
                    reply(
                        &out_tx,
                        req_id,
                        daemon.create_terminal(&worktree, name).map(Some),
                    )
                    .await;
                }
                ClientRequest::RenameTerminal { req_id, id, name } => {
                    reply_done(&out_tx, req_id, daemon.rename_terminal(&id, &name)).await;
                }
                ClientRequest::CloseTerminal { req_id, id } => {
                    reply_done(&out_tx, req_id, daemon.close_terminal(&id)).await;
                }
            }
        }
        Ok(())
    }
    .await;

    for (sref, h) in attached.drain() {
        h.abort();
        daemon.note_detached(&sref);
    }
    drop(out_tx);
    let _ = writer_task.await;
    result
}

/// Forward live PTY output/exit to one client, skipping bytes the attach
/// replay already delivered. On broadcast lag, resync with a fresh
/// Scrollback (the client resets its parser on every Scrollback frame).
async fn forward_pty(
    daemon: Arc<Daemon>,
    mut session: Arc<PtySession>,
    mut rx: broadcast::Receiver<PtyEvent>,
    mut installed: broadcast::Receiver<Arc<PtySession>>,
    out_tx: mpsc::Sender<ServerEvent>,
    mut min_seq: u64,
) {
    let sref = session.sref.clone();
    let mut running = true;
    loop {
        let event = tokio::select! {
            event = rx.recv(), if running => event,
            next = installed.recv() => {
                let next = match next {
                    Ok(next) => next,
                    Err(RecvError::Lagged(_)) => match daemon.session(&sref) {
                        Some(next) => next,
                        None => continue,
                    },
                    Err(RecvError::Closed) => break,
                };
                if next.sref != sref || Arc::ptr_eq(&next, &session) {
                    continue;
                }
                let (cols, rows) = session.size();
                rx = next.events.subscribe();
                let (base_seq, data) = next.snapshot(None);
                min_seq = base_seq + data.len() as u64;
                let _ = next.resize_with_jiggle(cols, rows);
                session = next;
                running = true;
                let replay = ServerEvent::Scrollback {
                    session: sref.clone(),
                    base_seq,
                    data,
                };
                if out_tx.send(replay).await.is_err() {
                    break;
                }
                continue;
            }
        };
        match event {
            Ok(PtyEvent::Output { seq, data }) => {
                let end = seq + data.len() as u64;
                if end <= min_seq {
                    continue; // fully covered by the replay
                }
                let skip = min_seq.saturating_sub(seq) as usize;
                let payload = if skip > 0 {
                    data[skip..].to_vec()
                } else {
                    data
                };
                let send_seq = seq + skip as u64;
                min_seq = end;
                if out_tx
                    .send(ServerEvent::Output {
                        session: sref.clone(),
                        seq: send_seq,
                        data: payload,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(PtyEvent::Exited { exit_code }) => {
                let _ = out_tx
                    .send(ServerEvent::SessionExited {
                        session: sref.clone(),
                        exit_code,
                    })
                    .await;
                running = false;
            }
            // Daemon-side only: the progress and title edges drive the
            // status machine and reach clients as a StatusChanged, not as
            // session output
            Ok(PtyEvent::Progress { .. } | PtyEvent::TitleIdle { .. } | PtyEvent::Suggestion) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Catch up from the ring. If the missed bytes are still
                // retained, send them as a plain Output continuation so the
                // client keeps its parser state; only when the gap has fallen
                // off the ring do we force a full replay (parser reset —
                // expensive on the client, so avoid it when possible).
                let wanted = min_seq;
                let (base_seq, data) = session.snapshot(Some(wanted));
                min_seq = base_seq + data.len() as u64;
                let ev = if base_seq == wanted {
                    ServerEvent::Output {
                        session: sref.clone(),
                        seq: base_seq,
                        data,
                    }
                } else {
                    ServerEvent::Scrollback {
                        session: sref.clone(),
                        base_seq,
                        data,
                    }
                };
                if out_tx.send(ev).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// [`reply`] for the requests that create nothing: success is a bare Ack.
async fn reply_router_config(
    out_tx: &mpsc::Sender<ServerEvent>,
    req_id: u64,
    result: anyhow::Result<(String, String)>,
) {
    let ev = match result {
        Ok((json, path)) => ServerEvent::RouterConfig { req_id, json, path },
        Err(e) => ServerEvent::Error {
            req_id: Some(req_id),
            message: format!("{e:#}"),
        },
    };
    let _ = out_tx.send(ev).await;
}

async fn reply_done(out_tx: &mpsc::Sender<ServerEvent>, req_id: u64, result: anyhow::Result<()>) {
    reply(out_tx, req_id, result.map(|_| None)).await
}

async fn reply(
    out_tx: &mpsc::Sender<ServerEvent>,
    req_id: u64,
    result: anyhow::Result<Option<berth_core::EntityId>>,
) {
    let ev = match result {
        Ok(created) => ServerEvent::Ack { req_id, created },
        Err(e) => ServerEvent::Error {
            req_id: Some(req_id),
            message: format!("{e:#}"),
        },
    };
    let _ = out_tx.send(ev).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::{SpawnSpec, DEFAULT_COLS, DEFAULT_ROWS};
    use berth_core::AgentId;
    use std::time::Duration;

    fn print_child(sref: &SessionRef, marker: &str) -> Arc<PtySession> {
        #[cfg(unix)]
        let (program, args) = (
            "/bin/sh".to_owned(),
            vec!["-c".into(), format!("printf '{marker}'")],
        );
        #[cfg(windows)]
        let (program, args) = (
            "powershell.exe".to_owned(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Write-Host {marker}"),
            ],
        );
        PtySession::spawn(
            sref.clone(),
            SpawnSpec {
                program,
                args,
                cwd: std::env::temp_dir(),
                env: vec![],
                scrub_env: &[],
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
            },
        )
        .unwrap()
    }

    async fn next_until(
        out_rx: &mut mpsc::Receiver<ServerEvent>,
        mut done: impl FnMut(&ServerEvent) -> bool,
    ) {
        tokio::time::timeout(Duration::from_secs(60), async {
            while !done(&out_rx.recv().await.expect("forwarding ended")) {}
        })
        .await
        .expect("timed out waiting for the forwarded session");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_attached_client_follows_its_session_into_the_respawned_process() {
        let daemon = crate::registry::tests::test_daemon();
        let sref = SessionRef::Agent(AgentId::generate());
        let installed = daemon.installed_sessions.subscribe();
        let first = print_child(&sref, "FIRST_PROCESS");
        let events = first.events.subscribe();
        let (out_tx, mut out_rx) = mpsc::channel(256);
        let forward = tokio::spawn(forward_pty(
            daemon.clone(),
            first,
            events,
            installed,
            out_tx,
            0,
        ));

        next_until(&mut out_rx, |event| {
            matches!(event, ServerEvent::SessionExited { .. })
        })
        .await;

        let second = print_child(&sref, "SECOND_PROCESS");
        assert!(daemon.installed_sessions.send(second).is_ok());
        let mut seen = Vec::new();
        next_until(&mut out_rx, |event| {
            if let ServerEvent::Output { data, .. } | ServerEvent::Scrollback { data, .. } = event {
                seen.extend_from_slice(data);
            }
            String::from_utf8_lossy(&seen).contains("SECOND_PROCESS")
        })
        .await;
        forward.abort();
    }
}
