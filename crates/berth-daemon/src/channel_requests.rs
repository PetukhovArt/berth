use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::{AgentId, ChannelEvent, FeedbackKind, FeedbackRequest, SessionRef};
use berth_hub::client::HubClient;
use berth_hub::{Event, EVENT_TAG, KIND_ANSWER, KIND_REQUEST, KIND_SETTLED};

use crate::channels::{convert, hub};
use crate::registry::Daemon;

const POLL: Duration = Duration::from_secs(3);
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
    channel: &str,
    request: i64,
    answer: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    if agent.is_some() {
        bail!("an Agent cannot answer a request in the Channel; its owner does");
    }
    let answer = Answer::parse(answer).context("answer with allow, deny or an option number")?;
    let event = hub()?
        .publish(channel, KIND_ANSWER, &answer.token(), &refers(request), None)
        .await?;
    convert(event)
}

struct Posted {
    channel: String,
    seq: i64,
    member: String,
    request: FeedbackRequest,
    read_after: i64,
}

impl Posted {
    async fn settle(&self, hub: &HubClient, answer: &str, by: Option<&str>, content: &str) {
        let mut tags = refers(self.seq);
        tags.push(vec!["answer".to_string(), answer.to_string()]);
        if let Some(by) = by {
            tags.push(vec!["by".to_string(), by.to_string()]);
        }
        if let Err(e) = hub
            .publish(
                &self.channel,
                KIND_SETTLED,
                content,
                &tags,
                Some(&self.member),
            )
            .await
        {
            tracing::warn!(channel = %self.channel, error = %e, "settling a channel request failed");
        }
    }
}

type Requests = HashMap<AgentId, FeedbackRequest>;

impl Daemon {
    fn request_channel(&self, agent: &AgentId, keys: &[String]) -> Option<String> {
        let row = self.store.get_agent(agent).ok().flatten()?;
        let worktree = self.store.get_worktree(&row.worktree_id).ok().flatten()?;
        crate::presence::on_tasks(&[(agent.0.clone(), worktree.branch)], keys)
            .into_keys()
            .min()
    }

    async fn apply_answer(self: &Arc<Self>, agent: &AgentId, answer: Answer) -> Result<()> {
        if self.is_acp_agent(agent) {
            let Answer::Option(i) = answer else {
                bail!("an ACP agent is answered with one of its options");
            };
            return self.answer_feedback(agent, i).await;
        }
        let keys = match answer {
            Answer::Allow => b"1".to_vec(),
            Answer::Deny => b"\x1b".to_vec(),
            Answer::Option(_) => answer.token().into_bytes(),
        };
        self.session(&SessionRef::Agent(agent.clone()))
            .context("the Agent's terminal is not running")?
            .write_input(&keys)
    }

    async fn settle_answered_locally(
        &self,
        hub: &HubClient,
        pending: &Requests,
        posted: &mut HashMap<AgentId, Posted>,
    ) {
        let gone: Vec<AgentId> = posted
            .iter()
            .filter(|(agent, p)| pending.get(*agent) != Some(&p.request))
            .map(|(agent, _)| agent.clone())
            .collect();
        for agent in gone {
            if let Some(p) = posted.remove(&agent) {
                p.settle(hub, ANSWERED_LOCALLY, None, "Answered in berth")
                    .await;
            }
        }
    }

    async fn take_owner_answers(
        self: &Arc<Self>,
        hub: &HubClient,
        owner: &str,
        posted: &mut HashMap<AgentId, Posted>,
        quiet: &mut Requests,
    ) {
        let mut answered = Vec::new();
        for (agent, p) in posted.iter_mut() {
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
            if let Some(p) = posted.remove(&agent) {
                quiet.insert(agent, p.request);
            }
        }
    }

    async fn post_new_requests(
        &self,
        hub: &HubClient,
        fresh: Vec<(AgentId, FeedbackRequest)>,
        posted: &mut HashMap<AgentId, Posted>,
        quiet: &mut Requests,
    ) {
        let Ok(channels) = hub.channels().await else {
            return;
        };
        let keys: Vec<String> = channels.into_iter().map(|c| c.key).collect();
        for (agent, request) in fresh {
            let Some(channel) = self.request_channel(&agent, &keys) else {
                quiet.insert(agent, request);
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
                    posted.insert(
                        agent,
                        Posted {
                            channel,
                            seq: event.seq,
                            member,
                            request,
                            read_after: event.seq,
                        },
                    );
                }
                Err(e) => tracing::warn!(agent = %agent.0, error = %e, "posting a request failed"),
            }
        }
    }

    pub async fn run_channel_requests(self: Arc<Self>) {
        let mut interval = tokio::time::interval(POLL);
        let mut posted: HashMap<AgentId, Posted> = HashMap::new();
        let mut quiet: Requests = HashMap::new();
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = interval.tick() => {}
            }
            let pending: Requests = self.feedback.all().into_iter().collect();
            quiet.retain(|agent, request| pending.get(agent) == Some(request));
            let unposted = |posted: &HashMap<AgentId, Posted>, quiet: &Requests| {
                pending
                    .iter()
                    .filter(|(agent, request)| {
                        !posted.contains_key(*agent) && quiet.get(*agent) != Some(request)
                    })
                    .map(|(agent, request)| (agent.clone(), request.clone()))
                    .collect::<Vec<_>>()
            };
            if posted.is_empty() && unposted(&posted, &quiet).is_empty() {
                continue;
            }
            let Ok(hub) = hub() else { continue };
            let Ok(owner) = hub.me().await else { continue };
            self.settle_answered_locally(&hub, &pending, &mut posted)
                .await;
            self.take_owner_answers(&hub, &owner, &mut posted, &mut quiet)
                .await;
            let fresh = unposted(&posted, &quiet);
            if !fresh.is_empty() {
                self.post_new_requests(&hub, fresh, &mut posted, &mut quiet)
                    .await;
            }
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
    fn a_question_takes_an_option_number_within_its_options() {
        let question = FeedbackRequest {
            kind: FeedbackKind::Question,
            title: "Which rounding?".into(),
            detail: None,
            options: vec!["Half up".into(), "Half even".into()],
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
