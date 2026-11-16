use berth_core::{AgentId, AgentStatus};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Body {
    Text(String),
    Voice { file_id: String, seconds: u64 },
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct Inbound {
    pub message_id: i64,
    pub date: i64,
    pub from: Option<i64>,
    pub from_bot: bool,
    pub chat_id: i64,
    pub chat_username: Option<String>,
    pub reply_to_bot: Option<i64>,
    pub body: Body,
}

impl Inbound {
    pub fn parse(update: &Value, bot_id: Option<i64>) -> Option<Self> {
        let message = update.get("message")?;
        let replied = &message["reply_to_message"];
        let replied_from = &replied["from"];
        let replied_to_bot = match bot_id {
            Some(bot) => replied_from["id"].as_i64() == Some(bot),
            None => replied_from["is_bot"].as_bool() == Some(true),
        };
        let audio = message.get("voice").or_else(|| message.get("audio"));
        let body = if let Some(text) = message["text"].as_str() {
            Body::Text(text.to_owned())
        } else if let Some(file_id) = audio.and_then(|audio| audio["file_id"].as_str()) {
            Body::Voice {
                file_id: file_id.to_owned(),
                seconds: audio
                    .and_then(|audio| audio["duration"].as_u64())
                    .unwrap_or(0),
            }
        } else {
            Body::Other
        };
        Some(Self {
            message_id: message["message_id"].as_i64()?,
            date: message["date"].as_i64().unwrap_or(0),
            from: message["from"]["id"].as_i64(),
            from_bot: message["from"]["is_bot"].as_bool() == Some(true),
            chat_id: message["chat"]["id"].as_i64()?,
            chat_username: message["chat"]["username"].as_str().map(str::to_owned),
            reply_to_bot: replied_to_bot
                .then(|| replied["message_id"].as_i64())
                .flatten(),
            body,
        })
    }
}

pub(crate) fn telegram_id(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn chat_matches(chat: &Value, message: &Inbound) -> bool {
    if let Some(id) = telegram_id(chat) {
        return id == message.chat_id;
    }
    match chat
        .as_str()
        .map(str::trim)
        .and_then(|chat| chat.strip_prefix('@'))
    {
        Some(name) => message
            .chat_username
            .as_deref()
            .is_some_and(|username| username.eq_ignore_ascii_case(name)),
        None => false,
    }
}

pub(crate) fn from_owner(owner: Option<&Value>, chat: Option<&Value>, message: &Inbound) -> bool {
    let (Some(owner), Some(chat)) = (owner.and_then(telegram_id), chat) else {
        return false;
    };
    !message.from_bot && message.from == Some(owner) && chat_matches(chat, message)
}

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub id: AgentId,
    pub status: AgentStatus,
    pub archived: bool,
    pub changed_at: i64,
    pub options: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    Deliver { agent: AgentId, queued: bool },
    Answer { agent: AgentId, option: u8 },
    NeedsAnswer { agent: AgentId },
    Gone,
    Forgotten,
    NoTarget,
}

fn waits_for_input(status: AgentStatus) -> bool {
    matches!(
        status,
        AgentStatus::Finished | AgentStatus::NeedsFeedback | AgentStatus::Fresh
    )
}

fn option_number(text: &str, count: usize) -> Option<u8> {
    let number: usize = text.trim().trim_end_matches(['.', ')']).parse().ok()?;
    (1..=count)
        .contains(&number)
        .then(|| u8::try_from(number - 1).ok())
        .flatten()
}

pub(crate) fn route(
    text: &str,
    reply_to_bot: Option<i64>,
    remembered: impl Fn(i64) -> Option<AgentId>,
    agents: &[Candidate],
) -> Route {
    let target = match reply_to_bot {
        Some(message) => {
            let Some(id) = remembered(message) else {
                return Route::Forgotten;
            };
            match agents
                .iter()
                .find(|agent| agent.id == id && !agent.archived)
            {
                Some(agent) => agent,
                None => return Route::Gone,
            }
        }
        None => match agents
            .iter()
            .filter(|agent| !agent.archived && waits_for_input(agent.status))
            .max_by_key(|agent| agent.changed_at)
        {
            Some(agent) => agent,
            None => return Route::NoTarget,
        },
    };
    let agent = target.id.clone();
    if target.status == AgentStatus::NeedsFeedback {
        if let Some(count) = target.options.filter(|count| *count > 0) {
            return match option_number(text, count) {
                Some(option) => Route::Answer { agent, option },
                None => Route::NeedsAnswer { agent },
            };
        }
    }
    Route::Deliver {
        agent,
        queued: target.status == AgentStatus::Running,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const OWNER: i64 = 1001;
    const BOT: i64 = 777;

    fn update(from: i64, chat: i64, extra: Value) -> Value {
        let mut message = json!({
            "message_id": 50,
            "date": 1_700_000_000,
            "from": { "id": from, "is_bot": false },
            "chat": { "id": chat, "type": "private", "username": "owner_chat" },
        });
        for (key, value) in extra.as_object().unwrap() {
            message[key] = value.clone();
        }
        json!({ "update_id": 9, "message": message })
    }

    fn text_from(from: i64, chat: i64) -> Inbound {
        Inbound::parse(
            &update(from, chat, json!({ "text": "run the tests" })),
            Some(BOT),
        )
        .unwrap()
    }

    fn agent(id: &str, status: AgentStatus, changed_at: i64) -> Candidate {
        Candidate {
            id: AgentId(id.into()),
            status,
            archived: false,
            changed_at,
            options: None,
        }
    }

    fn nothing_remembered(_: i64) -> Option<AgentId> {
        None
    }

    #[test]
    fn only_the_owner_in_the_configured_chat_is_heard_and_nothing_while_the_owner_is_unset() {
        let owner = json!(OWNER);
        let chat = json!(OWNER);
        let mine = text_from(OWNER, OWNER);
        assert!(from_owner(Some(&owner), Some(&chat), &mine));
        assert!(from_owner(
            Some(&json!("1001")),
            Some(&json!("1001")),
            &mine
        ));
        assert!(
            !from_owner(None, Some(&chat), &mine),
            "no owner set: nobody is heard"
        );
        assert!(!from_owner(Some(&json!("")), Some(&chat), &mine));
        assert!(!from_owner(Some(&owner), None, &mine));
        assert!(!from_owner(
            Some(&owner),
            Some(&chat),
            &text_from(2002, OWNER)
        ));
        assert!(!from_owner(
            Some(&owner),
            Some(&chat),
            &text_from(OWNER, -500)
        ));
        assert!(from_owner(Some(&owner), Some(&json!("@Owner_Chat")), &mine));
        assert!(!from_owner(
            Some(&owner),
            Some(&json!("@someone_else")),
            &mine
        ));
        let mut bot = mine.clone();
        bot.from_bot = true;
        assert!(!from_owner(Some(&owner), Some(&chat), &bot));
    }

    #[test]
    fn a_voice_note_and_a_reply_to_the_bot_are_read_from_the_update() {
        let voice = Inbound::parse(
            &update(
                OWNER,
                OWNER,
                json!({
                    "voice": { "file_id": "AwAD-x_1", "duration": 7 },
                    "reply_to_message": { "message_id": 41, "from": { "id": BOT, "is_bot": true } },
                }),
            ),
            Some(BOT),
        )
        .unwrap();
        assert_eq!(
            voice.body,
            Body::Voice {
                file_id: "AwAD-x_1".into(),
                seconds: 7
            }
        );
        assert_eq!(voice.reply_to_bot, Some(41));
        let reply_to_someone = Inbound::parse(
            &update(
                OWNER,
                OWNER,
                json!({
                    "text": "hi",
                    "reply_to_message": { "message_id": 42, "from": { "id": 3, "is_bot": false } },
                }),
            ),
            Some(BOT),
        )
        .unwrap();
        assert_eq!(reply_to_someone.reply_to_bot, None);
        assert!(
            Inbound::parse(&json!({ "update_id": 1, "edited_message": {} }), Some(BOT)).is_none()
        );
    }

    #[test]
    fn a_reply_to_a_notification_goes_to_its_session_and_waits_when_it_is_busy() {
        let agents = [
            agent("busy", AgentStatus::Running, 10),
            agent("idle", AgentStatus::Finished, 20),
        ];
        let remembered = |message: i64| (message == 41).then(|| AgentId("busy".into()));
        assert_eq!(
            route("go on", Some(41), remembered, &agents),
            Route::Deliver {
                agent: AgentId("busy".into()),
                queued: true
            }
        );
        assert_eq!(
            route("go on", Some(99), remembered, &agents),
            Route::Forgotten
        );
        let mut archived = agents.clone();
        archived[0].archived = true;
        assert_eq!(route("go on", Some(41), remembered, &archived), Route::Gone);
    }

    #[test]
    fn without_a_reply_the_latest_session_that_waits_for_input_gets_it() {
        let mut agents = vec![
            agent("older", AgentStatus::Finished, 10),
            agent("newer", AgentStatus::Finished, 30),
            agent("running", AgentStatus::Running, 50),
            agent("dead", AgentStatus::Terminated, 60),
        ];
        let mut archived = agent("archived", AgentStatus::Finished, 70);
        archived.archived = true;
        agents.push(archived);
        assert_eq!(
            route("next step", None, nothing_remembered, &agents),
            Route::Deliver {
                agent: AgentId("newer".into()),
                queued: false
            }
        );
        let busy = [agent("running", AgentStatus::Running, 50)];
        assert_eq!(
            route("next step", None, nothing_remembered, &busy),
            Route::NoTarget
        );
        assert_eq!(
            route("next step", None, nothing_remembered, &[]),
            Route::NoTarget
        );
    }

    #[test]
    fn a_session_waiting_on_a_question_takes_an_option_number_and_nothing_else() {
        let mut asking = agent("asking", AgentStatus::NeedsFeedback, 40);
        asking.options = Some(3);
        let agents = [agent("idle", AgentStatus::Finished, 10), asking];
        assert_eq!(
            route("2", None, nothing_remembered, &agents),
            Route::Answer {
                agent: AgentId("asking".into()),
                option: 1
            }
        );
        assert_eq!(
            route("4", None, nothing_remembered, &agents),
            Route::NeedsAnswer {
                agent: AgentId("asking".into())
            }
        );
        assert_eq!(
            route("sure, do it", None, nothing_remembered, &agents),
            Route::NeedsAnswer {
                agent: AgentId("asking".into())
            }
        );
        let unknown = [agent("codex", AgentStatus::NeedsFeedback, 40)];
        assert_eq!(
            route("yes", None, nothing_remembered, &unknown),
            Route::Deliver {
                agent: AgentId("codex".into()),
                queued: false
            }
        );
    }
}
