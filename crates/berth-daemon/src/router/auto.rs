use berth_core::{AgentId, AgentKind, RouteAction, RouteSuggestion};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub const RETURN_RULE: &str = "Return after the limit reset";
pub const STUCK_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnOffer {
    pub kind: AgentKind,
    pub model: Option<String>,
    pub resets_at_ms: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    InFlight,
    SwitchedThisTurn,
    HopCap,
    Cooldown,
}

#[derive(Default)]
struct State {
    in_flight: HashSet<AgentId>,
    switched_this_turn: HashSet<AgentId>,
    hops: HashMap<AgentId, u32>,
    rule_fired_ms: HashMap<String, i64>,
    returns: HashMap<AgentId, ReturnOffer>,
}

#[derive(Default)]
pub struct AutoRoutes(Mutex<State>);

pub struct Attempt<'a> {
    pub agent: &'a AgentId,
    pub rule_id: &'a str,
    pub cooldown_ms: i64,
    pub hops_before: u32,
    pub hop_cap: u32,
    pub now_ms: i64,
}

impl AutoRoutes {
    pub fn hops(&self, id: &AgentId) -> u32 {
        self.0.lock().unwrap().hops.get(id).copied().unwrap_or(0)
    }

    pub fn begin(&self, a: &Attempt) -> Result<(), Refusal> {
        let mut s = self.0.lock().unwrap();
        if s.in_flight.contains(a.agent) {
            return Err(Refusal::InFlight);
        }
        if s.switched_this_turn.contains(a.agent) {
            return Err(Refusal::SwitchedThisTurn);
        }
        let hops = a.hops_before.max(s.hops.get(a.agent).copied().unwrap_or(0));
        if hops >= a.hop_cap {
            return Err(Refusal::HopCap);
        }
        if s.rule_fired_ms
            .get(a.rule_id)
            .is_some_and(|at| a.now_ms - at < a.cooldown_ms)
        {
            return Err(Refusal::Cooldown);
        }
        s.in_flight.insert(a.agent.clone());
        s.switched_this_turn.insert(a.agent.clone());
        s.rule_fired_ms.insert(a.rule_id.to_string(), a.now_ms);
        s.hops.insert(a.agent.clone(), hops + 1);
        Ok(())
    }

    pub fn finish(&self, agent: &AgentId, continued_in: Option<&AgentId>) {
        let mut s = self.0.lock().unwrap();
        s.in_flight.remove(agent);
        if let Some(new) = continued_in {
            let hops = s.hops.get(agent).copied().unwrap_or(0);
            s.hops.insert(new.clone(), hops);
            s.switched_this_turn.insert(new.clone());
        }
    }

    pub fn turn_started(&self, agent: &AgentId) {
        self.0.lock().unwrap().switched_this_turn.remove(agent);
    }

    pub fn offer_return(&self, agent: &AgentId, offer: ReturnOffer) {
        self.0.lock().unwrap().returns.insert(agent.clone(), offer);
    }

    pub fn drop_return(&self, agent: &AgentId) {
        self.0.lock().unwrap().returns.remove(agent);
    }

    pub fn due_return(
        &self,
        agent: &AgentId,
        kind: AgentKind,
        model: Option<&str>,
        now_ms: i64,
    ) -> Option<RouteSuggestion> {
        let s = self.0.lock().unwrap();
        let offer = s.returns.get(agent)?;
        if now_ms < offer.resets_at_ms {
            return None;
        }
        let action = if offer.kind == kind {
            if offer.model.as_deref() == model {
                return None;
            }
            RouteAction::SwitchModel {
                model: offer.model.clone(),
            }
        } else {
            RouteAction::ContinueIn {
                kind: offer.kind,
                model: offer.model.clone(),
            }
        };
        Some(RouteSuggestion {
            rule: RETURN_RULE.into(),
            action,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt<'a>(agent: &'a AgentId, rule: &'a str, now_ms: i64) -> Attempt<'a> {
        Attempt {
            agent,
            rule_id: rule,
            cooldown_ms: 600_000,
            hops_before: 0,
            hop_cap: 3,
            now_ms,
        }
    }

    #[test]
    fn a_simulated_limit_switches_once_and_does_not_ping_pong() {
        let routes = AutoRoutes::default();
        let a = AgentId("a".into());
        let b = AgentId("b".into());
        assert_eq!(routes.begin(&attempt(&a, "fallback", 0)), Ok(()));
        assert_eq!(
            routes.begin(&attempt(&a, "fallback", 1)),
            Err(Refusal::InFlight)
        );
        routes.finish(&a, Some(&b));
        assert_eq!(
            routes.begin(&attempt(&a, "other", 2)),
            Err(Refusal::SwitchedThisTurn)
        );
        assert_eq!(
            routes.begin(&attempt(&b, "other", 3)),
            Err(Refusal::SwitchedThisTurn)
        );
        routes.turn_started(&b);
        assert_eq!(
            routes.begin(&attempt(&b, "fallback", 4)),
            Err(Refusal::Cooldown)
        );
        assert_eq!(routes.begin(&attempt(&b, "other", 5)), Ok(()));
        routes.finish(&b, None);
        routes.turn_started(&b);
        assert_eq!(routes.hops(&b), 2);
        let mut third = attempt(&b, "third", 6);
        third.hop_cap = 2;
        assert_eq!(routes.begin(&third), Err(Refusal::HopCap));
    }

    #[test]
    fn return_is_offered_only_after_the_reset() {
        let routes = AutoRoutes::default();
        let b = AgentId("b".into());
        routes.offer_return(
            &b,
            ReturnOffer {
                kind: AgentKind::Claude,
                model: None,
                resets_at_ms: 100,
            },
        );
        assert!(routes.due_return(&b, AgentKind::Codex, None, 99).is_none());
        let s = routes.due_return(&b, AgentKind::Codex, None, 100).unwrap();
        assert_eq!(
            s.action,
            RouteAction::ContinueIn {
                kind: AgentKind::Claude,
                model: None
            }
        );
        routes.drop_return(&b);
        assert!(routes.due_return(&b, AgentKind::Codex, None, 200).is_none());
    }
}
