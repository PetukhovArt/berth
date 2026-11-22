use crate::entities::{Agent, AgentStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attention {
    NeedsFeedback,
    Unseen,
    Terminated,
}

impl Attention {
    pub fn as_str(self) -> &'static str {
        match self {
            Attention::NeedsFeedback => "needs_feedback",
            Attention::Unseen => "unseen",
            Attention::Terminated => "terminated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionState {
    pub status: AgentStatus,
    pub unseen: bool,
    pub archived: bool,
    pub ended: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusChange {
    pub after: AttentionState,
    pub raised: Option<Attention>,
}

impl AttentionState {
    fn out_of_sight(self) -> bool {
        self.archived || self.ended
    }

    pub fn attention(self) -> Option<Attention> {
        if self.out_of_sight() {
            return None;
        }
        match self.status {
            AgentStatus::NeedsFeedback => Some(Attention::NeedsFeedback),
            AgentStatus::Terminated => Some(Attention::Terminated),
            AgentStatus::Finished if self.unseen => Some(Attention::Unseen),
            _ => None,
        }
    }

    pub fn change(self, status: AgentStatus) -> StatusChange {
        let turn_was_live = matches!(
            self.status,
            AgentStatus::Running | AgentStatus::NeedsFeedback
        );
        let unseen = status == AgentStatus::Finished
            && (self.unseen || turn_was_live && !self.out_of_sight());
        let after = AttentionState {
            status,
            unseen,
            archived: self.archived,
            ended: self.ended,
        };
        let raised = after
            .attention()
            .filter(|now| self.attention() != Some(*now));
        StatusChange { after, raised }
    }
}

impl Agent {
    pub fn attention(&self) -> Option<Attention> {
        AttentionState {
            status: self.status,
            unseen: self.unseen,
            archived: self.archived,
            ended: self.is_ended(),
        }
        .attention()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentStatus::*;

    fn state(status: AgentStatus) -> AttentionState {
        AttentionState {
            status,
            unseen: false,
            archived: false,
            ended: false,
        }
    }

    fn raised(from: AttentionState, to: AgentStatus) -> Option<Attention> {
        from.change(to).raised
    }

    #[test]
    fn a_turn_finished_unwatched_raises_unseen_from_running_and_from_needs_feedback() {
        assert_eq!(raised(state(Running), Finished), Some(Attention::Unseen));
        assert_eq!(
            raised(state(NeedsFeedback), Finished),
            Some(Attention::Unseen)
        );
        assert!(state(NeedsFeedback).change(Finished).after.unseen);
    }

    #[test]
    fn a_finish_berth_never_saw_start_raises_nothing() {
        let change = state(Fresh).change(Finished);
        assert_eq!(change.raised, None);
        assert!(!change.after.unseen);
    }

    #[test]
    fn waiting_and_dying_raise_once() {
        assert_eq!(
            raised(state(Running), NeedsFeedback),
            Some(Attention::NeedsFeedback)
        );
        assert_eq!(raised(state(NeedsFeedback), NeedsFeedback), None);
        assert_eq!(
            raised(state(Running), Terminated),
            Some(Attention::Terminated)
        );
        assert_eq!(
            raised(state(NeedsFeedback), Terminated),
            Some(Attention::Terminated)
        );
    }

    #[test]
    fn unseen_stays_on_finished_and_drops_when_finished_is_left() {
        let read_me = state(Running).change(Finished).after;
        let again = read_me.change(Finished);
        assert!(again.after.unseen);
        assert_eq!(again.raised, None, "still the same unread turn");
        let next = read_me.change(Running);
        assert!(!next.after.unseen);
        assert_eq!(next.raised, None);
    }

    #[test]
    fn archived_agents_never_want_the_user() {
        let archived = AttentionState {
            archived: true,
            ..state(Running)
        };
        let change = archived.change(Finished);
        assert!(!change.after.unseen);
        assert_eq!(change.raised, None);
        assert_eq!(archived.change(NeedsFeedback).raised, None);
        assert_eq!(archived.change(Terminated).raised, None);
    }

    #[test]
    fn ended_agents_never_want_the_user() {
        let ended = AttentionState {
            ended: true,
            ..state(Running)
        };
        let change = ended.change(Finished);
        assert!(!change.after.unseen);
        assert_eq!(change.raised, None);
        assert_eq!(ended.change(NeedsFeedback).raised, None);
        assert_eq!(ended.change(Terminated).raised, None);
    }

    #[test]
    fn the_level_is_waiting_crashed_or_unread() {
        assert_eq!(
            state(NeedsFeedback).attention(),
            Some(Attention::NeedsFeedback)
        );
        assert_eq!(state(Terminated).attention(), Some(Attention::Terminated));
        assert_eq!(state(Finished).attention(), None, "already read");
        let unread = AttentionState {
            unseen: true,
            ..state(Finished)
        };
        assert_eq!(unread.attention(), Some(Attention::Unseen));
        for quiet in [Fresh, Running, Disconnected] {
            assert_eq!(state(quiet).attention(), None);
        }
    }
}
