use berth_core::acp::{
    AcpCommand, AcpItem, AcpItemAt, AcpSessionState, AcpTool, Applies, Attachment,
    AttachmentUpload, ChatCaps, Choice, ConfigOption, FeedbackAnswer, McpAccess, McpServer,
    OptionCategory, PromptInput, Question, QuestionForm, Questions, QueueId, QueuedPrompt, Rewind,
};
use berth_core::codec::{read_frame, write_frame};
use berth_core::{
    AgentId, Backend, ClientRequest, FeedbackKind, FeedbackRequest, ServerEvent, SpawnWorktree,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;

async fn through_codec<T: Serialize + DeserializeOwned>(value: &T) -> T {
    let mut buf = Vec::new();
    write_frame(&mut buf, value).await.unwrap();
    let mut cursor = std::io::Cursor::new(buf);
    read_frame(&mut cursor)
        .await
        .expect("frame decodes")
        .expect("one frame")
}

async fn assert_round_trips<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: T) {
    assert_eq!(through_codec(&value).await, value);
}

async fn assert_round_trips_by_debug_form<T: Serialize + DeserializeOwned + Debug>(value: T) {
    let back = through_codec(&value).await;
    assert_eq!(format!("{back:?}"), format!("{value:?}"));
}

fn agent() -> AgentId {
    AgentId("agent-1".into())
}

fn choice(full: bool) -> Choice {
    Choice {
        value: "v".into(),
        name: "name".into(),
        description: full.then(|| "why".to_string()),
    }
}

fn attachments() -> Vec<Attachment> {
    vec![
        Attachment::Image {
            blob: "b1".into(),
            mime: "image/png".into(),
            name: "a.png".into(),
        },
        Attachment::File {
            blob: "b2".into(),
            mime: "text/plain".into(),
            name: "a.txt".into(),
        },
        Attachment::WorktreeFile {
            path: "src/main.rs".into(),
        },
    ]
}

fn rewind(full: bool) -> Rewind {
    Rewind {
        checkpoint: full.then_some(3),
        after: full.then(|| "msg".to_string()),
    }
}

fn uploads() -> Vec<AttachmentUpload> {
    vec![
        AttachmentUpload::Image {
            mime: "image/png".into(),
            name: "a.png".into(),
            data: "AAAA".into(),
        },
        AttachmentUpload::File {
            mime: "text/plain".into(),
            name: "a.txt".into(),
            text: "hi".into(),
        },
        AttachmentUpload::WorktreeFile {
            path: "src/main.rs".into(),
        },
    ]
}

fn form(full: bool) -> QuestionForm {
    QuestionForm {
        questions: vec![Question {
            id: "q1".into(),
            header: "h".into(),
            text: "t".into(),
            choices: if full { vec![choice(true)] } else { vec![] },
            multi_select: full,
            free_text_allowed: full,
        }],
    }
}

fn state(full: bool) -> AcpSessionState {
    if !full {
        return AcpSessionState::default();
    }
    AcpSessionState {
        caps: ChatCaps {
            images: true,
            embedded_files: true,
            questions: Questions::Typed,
            mcp: McpAccess::Toggle,
            rewind: true,
            steer: true,
            terminal_view: true,
        },
        options: vec![ConfigOption {
            id: "model".into(),
            name: "Model".into(),
            category: OptionCategory::Model,
            current: "a".into(),
            choices: vec![choice(true), choice(false)],
            applies: Applies::NextStart,
        }],
        commands: vec![AcpCommand {
            name: "compact".into(),
            description: "d".into(),
        }],
        queue: vec![QueuedPrompt {
            id: QueueId(7),
            prompt: PromptInput {
                text: "later".into(),
                attachments: attachments(),
            },
        }],
        mcp: vec![
            McpServer {
                name: "fs".into(),
                status: Some("ready".into()),
            },
            McpServer {
                name: "git".into(),
                status: None,
            },
        ],
    }
}

fn user_items_with_some_optional_fields_set() -> Vec<AcpItem> {
    let user = |attachments: Vec<Attachment>, rewind: Option<Rewind>| AcpItem::User {
        text: "hello".into(),
        attachments,
        rewind,
    };
    vec![
        user(vec![], None),
        user(vec![], Some(rewind(true))),
        user(vec![], Some(rewind(false))),
        user(attachments(), None),
        user(attachments(), Some(rewind(true))),
    ]
}

fn all_items() -> Vec<AcpItem> {
    let mut items = user_items_with_some_optional_fields_set();
    items.push(AcpItem::Assistant { text: "a".into() });
    items.push(AcpItem::Thought { text: "t".into() });
    items.push(AcpItem::Tool(AcpTool::default()));
    items.push(AcpItem::Plan { entries: vec![] });
    items.push(AcpItem::Notice { text: "n".into() });
    items
}

#[tokio::test]
async fn user_item_decodes_when_only_some_optional_fields_are_set() {
    for item in user_items_with_some_optional_fields_set() {
        assert_round_trips(item).await;
    }
}

#[tokio::test]
async fn chat_items_round_trip_inside_the_events_that_carry_them() {
    for item in all_items() {
        assert_round_trips_by_debug_form(ServerEvent::AcpItem {
            agent: agent(),
            seq: 1,
            item: Box::new(item.clone()),
        })
        .await;
    }
    let items = all_items()
        .into_iter()
        .zip(1u64..)
        .map(|(item, seq)| AcpItemAt { seq, item })
        .collect();
    assert_round_trips_by_debug_form(ServerEvent::AcpTranscript {
        req_id: 1,
        agent: agent(),
        items,
    })
    .await;
}

#[tokio::test]
async fn session_state_round_trips_full_and_minimal() {
    for full in [false, true] {
        assert_round_trips(state(full)).await;
        assert_round_trips_by_debug_form(ServerEvent::AcpSessionState {
            req_id: 1,
            agent: agent(),
            state: Box::new(state(full)),
        })
        .await;
        assert_round_trips_by_debug_form(ServerEvent::AcpSessionChanged {
            agent: agent(),
            state: Box::new(state(full)),
        })
        .await;
    }
}

#[tokio::test]
async fn prompt_attachment_and_answer_types_round_trip_full_and_minimal() {
    assert_round_trips(PromptInput::default()).await;
    assert_round_trips(PromptInput {
        text: "t".into(),
        attachments: attachments(),
    })
    .await;
    for upload in uploads() {
        assert_round_trips(upload).await;
    }
    for full in [false, true] {
        assert_round_trips(rewind(full)).await;
        assert_round_trips(form(full)).await;
        assert_round_trips(choice(full)).await;
    }
    assert_round_trips(FeedbackAnswer::Option(2)).await;
    assert_round_trips(FeedbackAnswer::Form(BTreeMap::new())).await;
    assert_round_trips(FeedbackAnswer::Form(BTreeMap::from([(
        "q1".to_string(),
        vec!["a".to_string(), "b".to_string()],
    )])))
    .await;
}

#[tokio::test]
async fn feedback_request_round_trips_with_and_without_a_form() {
    for form in [None, Some(form(false)), Some(form(true))] {
        for detail in [None, Some("d".to_string())] {
            let request = FeedbackRequest {
                kind: FeedbackKind::Question,
                title: "t".into(),
                detail,
                options: vec!["yes".into()],
                always_allow: true,
                form: form.clone(),
            };
            assert_round_trips(request.clone()).await;
            assert_round_trips_by_debug_form(ServerEvent::FeedbackRequestChanged {
                agent: agent(),
                request: Some(request),
            })
            .await;
        }
    }
}

#[tokio::test]
async fn client_requests_for_chats_round_trip_full_and_minimal() {
    for attachments in [vec![], uploads()] {
        assert_round_trips_by_debug_form(ClientRequest::AcpPrompt {
            req_id: 1,
            agent: agent(),
            text: "t".into(),
            attachments,
        })
        .await;
    }
    for answer in [
        FeedbackAnswer::Option(1),
        FeedbackAnswer::Form(BTreeMap::new()),
    ] {
        assert_round_trips_by_debug_form(ClientRequest::AnswerFeedback {
            req_id: 1,
            agent: agent(),
            answer,
        })
        .await;
    }
    for backend in [Backend::Chat, Backend::Terminal] {
        assert_round_trips_by_debug_form(ClientRequest::SetAgentBackend {
            req_id: 1,
            id: agent(),
            backend,
        })
        .await;
    }
    for worktree in [
        None,
        Some(SpawnWorktree {
            branch: "b".into(),
            base: None,
        }),
        Some(SpawnWorktree {
            branch: "b".into(),
            base: Some("main".into()),
        }),
    ] {
        assert_round_trips_by_debug_form(ClientRequest::SpawnSiblingAgent {
            req_id: 1,
            id: agent(),
            kind: None,
            starting_prompt: "p".into(),
            worktree,
        })
        .await;
    }
}
