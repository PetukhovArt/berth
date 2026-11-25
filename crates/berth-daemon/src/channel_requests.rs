use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::acp::FeedbackAnswer;
use berth_core::{AgentId, ChannelEvent, FeedbackKind, FeedbackRequest, ServerEvent, SessionRef};
use berth_hub::client::HubClient;
use berth_hub::{Event, EVENT_TAG, KIND_ANSWER, KIND_REQUEST, KIND_SETTLED};

use rusqlite::params;
use tokio::sync::broadcast;

use crate::channels::{hub, mark, own};
use crate::config::Config;
use crate::hub_stream::HubPush;
use crate::registry::Daemon;
use crate::store::Store;

const FALLBACK_POLL: Duration = Duration::from_secs(15);
const ANSWERED_LOCALLY: &str = "local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Allow,
    Deny,
    Option(u8),
}

impl Answer {
    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            number => number
                .parse::<u8>()
                .ok()
                .filter(|n| *n >= 1)
                .map(|n| Self::Option(n - 1)),
        }
    }

    fn token(self) -> String {
        match self {
            Self::Allow => "allow".into(),
            Self::Deny => "deny".into(),
            Self::Option(i) => (u16::from(i) + 1).to_string(),
        }
    }

    fn fits(self, request: &FeedbackRequest) -> bool {
        match self {
            Self::Allow | Self::Deny => {
                request.kind == FeedbackKind::Permission && request.options.is_empty()
            }
            Self::Option(i) => usize::from(i) < request.options.len(),
        }
    }

    fn describe(self, request: &FeedbackRequest, by: &str) -> String {
        match self {
            Self::Allow => format!("Allowed by {by} from the Channel"),
            Self::Deny => format!("Denied by {by} from the Channel"),
            Self::Option(i) => format!(
                "{by} chose \"{}\" from the Channel",
                request
                    .options
                    .get(usize::from(i))
                    .map(String::as_str)
                    .unwrap_or("?")
            ),
        }
    }
}

fn refers(seq: i64) -> Vec<Vec<String>> {
    vec![vec![EVENT_TAG.to_string(), seq.to_string()]]
}

fn refers_to(event: &Event, seq: i64) -> bool {
    let seq = seq.to_string();
    event
        .tags
        .iter()
        .any(|t| t.len() == 2 && t[0] == EVENT_TAG && t[1] == seq)
}

pub fn request_event(request: &FeedbackRequest) -> (String, Vec<Vec<String>>) {
    let kind = match request.kind {
        FeedbackKind::Permission => "permission",
        FeedbackKind::Question => "question",
    };
    let mut tags = vec![
        vec!["request".to_string(), kind.to_string()],
        vec!["title".to_string(), request.title.clone()],
    ];
    if let Some(detail) = &request.detail {
        tags.push(vec!["detail".to_string(), detail.clone()]);
    }
    for option in &request.options {
        tags.push(vec!["option".to_string(), option.clone()]);
    }
    let content = match (&request.kind, &request.detail) {
        (FeedbackKind::Permission, Some(detail)) => {
            format!("Waiting for permission: {}\n{detail}", request.title)
        }
        (FeedbackKind::Permission, None) => format!("Waiting for permission: {}", request.title),
        (FeedbackKind::Question, _) => request.title.clone(),
    };
    (content, tags)
}

pub fn owner_answer(
    events: &[Event],
    seq: i64,
    owner: &str,
    request: &FeedbackRequest,
) -> Option<(Answer, String)> {
    events
        .iter()
        .filter(|e| e.kind == KIND_ANSWER && e.author == owner && refers_to(e, seq))
        .find_map(|e| {
            Answer::parse(&e.content)
                .filter(|a| a.fits(request))
                .map(|a| (a, e.author_name.clone()))
        })
}

pub async fn answer_in_channel(
    store: &Store,
    channel: &str,
    request: i64,
    answer: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    if agent.is_some() {
        bail!("an Agent cannot answer a request in the Channel; its owner does");
    }
    let answer = Answer::parse(answer).context("answer with allow, deny or an option number")?;
    let hub = hub()?;
    let event = hub
        .publish(
            channel,
            KIND_ANSWER,
            &answer.token(),
            &refers(request),
            None,
        )
        .await?;
    mark(event, &own(store, &hub).await)
}

struct Posted {
    channel: String,
    seq: i64,
    member: String,
    request: FeedbackRequest,
    read_after: i64,
}

impl Posted {
    async fn settle(&self, hub: &HubClient, answer: &str, by: Option<&str>, content: &str) -> bool {
        let mut tags = refers(self.seq);
        tags.push(vec!["answer".to_string(), answer.to_string()]);
        if let Some(by) = by {
            tags.push(vec!["by".to_string(), by.to_string()]);
        }
        match hub
            .publish(
                &self.channel,
                KIND_SETTLED,
                content,
                &tags,
                Some(&self.member),
            )
            .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(channel = %self.channel, error = %e, "settling a channel request failed");
                false
            }
        }
    }
}

fn save_posted(store: &Store, hub_url: &str, agent: &AgentId, p: &Posted) -> Result<()> {
    store.conn().execute(
        "INSERT OR REPLACE INTO channel_requests_posted
         (agent_id, hub_url, channel, seq, member, request) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            agent.as_str(),
            hub_url,
            p.channel,
            p.seq,
            p.member,
            serde_json::to_string(&p.request)?
        ],
    )?;
    Ok(())
}

fn forget_posted(store: &Store, agent: &AgentId) {
    if let Err(e) = store.conn().execute(
        "DELETE FROM channel_requests_posted WHERE agent_id = ?1",
        params![agent.as_str()],
    ) {
        tracing::warn!(agent = %agent.0, error = %e, "forgetting a posted request failed");
    }
}

fn load_posted(store: &Store, hub_url: &str) -> Result<HashMap<AgentId, Posted>> {
    let conn = store.conn();
    let mut st = conn.prepare(
        "SELECT agent_id, channel, seq, member, request FROM channel_requests_posted
         WHERE hub_url = ?1",
    )?;
    let rows = st.query_map(params![hub_url], |r| {
        Ok((
            AgentId(r.get(0)?),
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut posted = HashMap::new();
    for row in rows {
        let (agent, channel, seq, member, request) = row?;
        let Ok(request) = serde_json::from_str(&request) else {
            continue;
        };
        posted.insert(
            agent,
            Posted {
                channel,
                seq,
                member,
                request,
                read_after: seq,
            },
        );
    }
    Ok(posted)
}

type Requests = HashMap<AgentId, FeedbackRequest>;

#[derive(Default)]
struct Tracked {
    hub_url: Option<String>,
    posted: HashMap<AgentId, Posted>,
    quiet: Requests,
}

impl Tracked {
    fn follow_hub(&mut self, store: &Store) {
        let url = Config::load().hub_url.unwrap_or_default();
        if self.hub_url.as_deref() == Some(url.as_str()) {
            return;
        }
        match load_posted(store, &url) {
            Ok(posted) => self.posted = posted,
            Err(e) => {
                tracing::warn!(error = %e, "reading posted requests failed");
                self.posted.clear();
            }
        }
        self.hub_url = Some(url);
    }

    fn remember(&mut self, store: &Store, agent: AgentId, p: Posted) {
        let url = self.hub_url.as_deref().unwrap_or_default();
        if let Err(e) = save_posted(store, url, &agent, &p) {
            tracing::warn!(agent = %agent.0, error = %e, "saving a posted request failed");
        }
        self.posted.insert(agent, p);
    }

    fn forget(&mut self, store: &Store, agent: &AgentId) -> Option<Posted> {
        forget_posted(store, agent);
        self.posted.remove(agent)
    }
}

impl Daemon {
    fn request_channel(&self, agent: &AgentId, keys: &[String]) -> Option<String> {
        let row = self.store.get_agent(agent).ok().flatten()?;
        let worktree = self.store.get_worktree(&row.worktree_id).ok().flatten()?;
        crate::presence::on_tasks(&[(agent.0.clone(), worktree.branch)], keys, false)
            .into_keys()
            .min()
    }

    async fn apply_answer(self: &Arc<Self>, agent: &AgentId, answer: Answer) -> Result<()> {
        if self.is_acp_agent(agent) {
            let Answer::Option(i) = answer else {
                bail!("an ACP agent is answered with one of its options");
            };
            return self.answer_feedback(agent, FeedbackAnswer::Option(i)).await;
        }
        let keys = match answer {
            Answer::Allow => crate::feedback::ALLOW_KEY.to_vec(),
            Answer::Deny => crate::feedback::DENY_KEY.to_vec(),
            Answer::Option(_) => answer.token().into_bytes(),
        };
        self.session(&SessionRef::Agent(agent.clone()))
            .context("the Agent's terminal is not running")?
            .write_input(&keys)?;
        self.note_typed_into_agent(agent, &keys);
        Ok(())
    }

    async fn settle_answered_locally(
        &self,
        hub: &HubClient,
        pending: &Requests,
        tracked: &mut Tracked,
    ) {
        let gone: Vec<AgentId> = tracked
            .posted
            .iter()
            .filter(|(agent, p)| pending.get(*agent) != Some(&p.request))
            .map(|(agent, _)| agent.clone())
            .collect();
        for agent in gone {
            let Some(p) = tracked.posted.get(&agent) else {
                continue;
            };
            if p.settle(hub, ANSWERED_LOCALLY, None, "Answered in berth")
                .await
            {
                tracked.forget(&self.store, &agent);
            }
        }
    }

    async fn take_owner_answers(
        self: &Arc<Self>,
        hub: &HubClient,
        owner: &str,
        tracked: &mut Tracked,
    ) {
        let mut answered = Vec::new();
        for (agent, p) in tracked.posted.iter_mut() {
            let Ok(events) = hub.read(&p.channel, p.read_after, Some(&p.member)).await else {
                continue;
            };
            if let Some(last) = events.last() {
                p.read_after = last.seq;
            }
            let Some((answer, by)) = owner_answer(&events, p.seq, owner, &p.request) else {
                continue;
            };
            match self.apply_answer(agent, answer).await {
                Ok(()) => {
                    p.settle(
                        hub,
                        &answer.token(),
                        Some(&by),
                        &answer.describe(&p.request, &by),
                    )
                    .await;
                    answered.push(agent.clone());
                }
                Err(e) => {
                    tracing::warn!(agent = %agent.0, error = %e, "answer from the channel failed")
                }
            }
        }
        for agent in answered {
            if let Some(p) = tracked.forget(&self.store, &agent) {
                tracked.quiet.insert(agent, p.request);
            }
        }
    }

    async fn post_new_requests(
        &self,
        hub: &HubClient,
        fresh: Vec<(AgentId, FeedbackRequest)>,
        tracked: &mut Tracked,
    ) {
        let Ok(channels) = hub.channels().await else {
            return;
        };
        let keys: Vec<String> = channels.into_iter().map(|c| c.key).collect();
        for (agent, request) in fresh {
            let Some(channel) = self.request_channel(&agent, &keys) else {
                continue;
            };
            let Ok(member) = hub.agent(agent.as_str()).await else {
                continue;
            };
            let (content, tags) = request_event(&request);
            match hub
                .publish(&channel, KIND_REQUEST, &content, &tags, Some(&member))
                .await
            {
                Ok(event) => {
                    let posted = Posted {
                        channel,
                        seq: event.seq,
                        member,
                        request,
                        read_after: event.seq,
                    };
                    tracked.remember(&self.store, agent, posted);
                }
                Err(e) => tracing::warn!(agent = %agent.0, error = %e, "posting a request failed"),
            }
        }
    }

    async fn requests_pass(self: &Arc<Self>, tracked: &mut Tracked) -> bool {
        let pending: Requests = self.feedback.all().into_iter().collect();
        tracked
            .quiet
            .retain(|agent, request| pending.get(agent) == Some(request));
        let unposted = |tracked: &Tracked| {
            pending
                .iter()
                .filter(|(agent, request)| {
                    !tracked.posted.contains_key(*agent)
                        && tracked.quiet.get(*agent) != Some(request)
                })
                .map(|(agent, request)| (agent.clone(), request.clone()))
                .collect::<Vec<_>>()
        };
        let idle = |tracked: &Tracked| tracked.posted.is_empty() && unposted(tracked).is_empty();
        if tracked.hub_url.is_some() && idle(tracked) {
            return false;
        }
        let Ok(hub) = hub() else { return true };
        tracked.follow_hub(&self.store);
        if idle(tracked) {
            return false;
        }
        let Ok(owner) = hub.me().await.map(|me| me.id) else {
            return true;
        };
        self.settle_answered_locally(&hub, &pending, tracked).await;
        self.take_owner_answers(&hub, &owner, tracked).await;
        let fresh = unposted(tracked);
        if !fresh.is_empty() {
            self.post_new_requests(&hub, fresh, tracked).await;
        }
        !unposted(tracked).is_empty()
    }

    pub(crate) async fn run_channel_requests(self: Arc<Self>, push: Arc<HubPush>) {
        let mut events = self.events.subscribe();
        let mut live = push.watch_live();
        let mut tracked = Tracked::default();
        loop {
            let unrouted = self.requests_pass(&mut tracked).await;
            let streamed = *live.borrow_and_update() && !unrouted;
            let fallback = async move {
                if streamed {
                    std::future::pending::<()>().await
                } else {
                    tokio::time::sleep(FALLBACK_POLL).await
                }
            };
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = fallback => {}
                _ = live.changed() => {}
                woke = wake_event(&mut events) => if !woke { break },
            }
        }
    }
}

async fn wake_event(events: &mut broadcast::Receiver<ServerEvent>) -> bool {
    loop {
        match events.recv().await {
            Ok(
                ServerEvent::ChannelAppended { .. } | ServerEvent::FeedbackRequestChanged { .. },
            ) => return true,
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => return true,
            Err(broadcast::error::RecvError::Closed) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission() -> FeedbackRequest {
        FeedbackRequest {
            kind: FeedbackKind::Permission,
            title: "Bash".into(),
            detail: Some("pnpm test".into()),
            options: vec![],
            always_allow: false,
            form: None,
        }
    }

    fn answer(seq: i64, author: &str, content: &str, refers_to: i64) -> Event {
        Event {
            seq,
            kind: KIND_ANSWER,
            author: author.into(),
            author_name: format!("{author}-name"),
            author_kind: "person".into(),
            channel: "PROJ-1".into(),
            tags: refers(refers_to),
            content: content.into(),
            created_at: 0,
        }
    }

    #[test]
    fn only_the_owners_answer_to_that_request_counts() {
        let request = permission();
        let events = vec![
            answer(11, "m-bob", "allow", 10),
            answer(12, "m-ann", "deny", 9),
            answer(13, "m-ann", "2", 10),
            answer(14, "m-ann", "deny", 10),
        ];
        assert_eq!(
            owner_answer(&events, 10, "m-ann", &request),
            Some((Answer::Deny, "m-ann-name".into()))
        );
        assert_eq!(owner_answer(&events[..3], 10, "m-ann", &request), None);
    }

    #[test]
    fn a_posted_request_is_still_known_after_a_restart_until_it_is_settled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("berth.db");
        let agent = AgentId("agent-1".into());
        {
            let store = Store::open(&path).unwrap();
            let posted = Posted {
                channel: "PROJ-1".into(),
                seq: 10,
                member: "a-1".into(),
                request: permission(),
                read_after: 10,
            };
            save_posted(&store, "https://hub.example", &agent, &posted).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert!(load_posted(&store, "https://other.example")
            .unwrap()
            .is_empty());
        let restored = load_posted(&store, "https://hub.example").unwrap();
        let p = &restored[&agent];
        assert_eq!(
            (p.channel.as_str(), p.seq, p.member.as_str(), &p.request),
            ("PROJ-1", 10, "a-1", &permission())
        );
        forget_posted(&store, &agent);
        drop(store);
        let store = Store::open(&path).unwrap();
        assert!(load_posted(&store, "https://hub.example")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_question_takes_an_option_number_within_its_options() {
        let question = FeedbackRequest {
            kind: FeedbackKind::Question,
            title: "Which rounding?".into(),
            detail: None,
            options: vec!["Half up".into(), "Half even".into()],
            always_allow: false,
            form: None,
        };
        assert!(Answer::parse("2").unwrap().fits(&question));
        assert!(!Answer::parse("3").unwrap().fits(&question));
        assert!(!Answer::Allow.fits(&question));
        assert_eq!(Answer::parse("0"), None);
        assert_eq!(Answer::parse(" Allow "), Some(Answer::Allow));
        assert_eq!(Answer::Option(1).token(), "2");
    }

    #[test]
    fn a_request_carries_only_what_its_card_shows() {
        let (content, tags) = request_event(&permission());
        assert_eq!(content, "Waiting for permission: Bash\npnpm test");
        assert_eq!(
            tags,
            [
                vec!["request", "permission"],
                vec!["title", "Bash"],
                vec!["detail", "pnpm test"]
            ]
        );
    }
}
