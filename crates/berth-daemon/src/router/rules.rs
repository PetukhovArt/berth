use berth_core::{
    AgentKind, AgentStatus, ProviderError, ProviderErrorKind, RouteAction, RouteSuggestion,
    UsageWindow,
};
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

pub const SCHEMA_VERSION: u32 = 1;
pub const FULL_PERCENT: f32 = 95.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    All(Vec<Expr>),
    Any(Vec<Expr>),
    Not(Box<Expr>),
    ProviderError {
        kinds: Vec<ProviderErrorKind>,
        for_secs: u64,
    },
    LimitUsed {
        pct: f32,
        window_minutes: Option<u32>,
    },
    Status(Vec<AgentStatus>),
    Kind(Vec<AgentKind>),
    Model(Vec<String>),
    Unknown(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    NextInChain,
    SwitchModel {
        model: String,
    },
    ContinueIn {
        kind: AgentKind,
        model: Option<String>,
    },
    Unknown(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub when: ExprValue,
    #[serde(default)]
    pub then: Vec<ActionValue>,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "Value", into = "Value")]
pub struct ExprValue(pub Expr);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "Value", into = "Value")]
pub struct ActionValue(pub Action);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouterFile {
    pub schema_version: u32,
    #[serde(default = "suggest_mode")]
    pub mode: String,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    #[serde(flatten)]
    pub extra: Table,
}

fn yes() -> bool {
    true
}

fn suggest_mode() -> String {
    "suggest".into()
}

impl Default for RouterFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            mode: suggest_mode(),
            fallback: vec!["claude".into(), "codex".into(), "grok".into()],
            rules: vec![Rule {
                id: "fallback".into(),
                name: "Next agent in the fallback chain".into(),
                enabled: true,
                when: ExprValue(Expr::Any(vec![
                    Expr::ProviderError {
                        kinds: vec![
                            ProviderErrorKind::UsageLimit,
                            ProviderErrorKind::Overloaded,
                            ProviderErrorKind::Server,
                        ],
                        for_secs: 0,
                    },
                    Expr::LimitUsed {
                        pct: FULL_PERCENT,
                        window_minutes: None,
                    },
                ])),
                then: vec![ActionValue(Action::NextInChain)],
                extra: Table::new(),
            }],
            extra: Table::new(),
        }
    }
}

fn single(value: &Value) -> Option<(&str, &Value)> {
    let table = value.as_table()?;
    if table.len() != 1 {
        return None;
    }
    table.iter().next().map(|(k, v)| (k.as_str(), v))
}

fn strings(value: &Value) -> Option<Vec<&str>> {
    value.as_array()?.iter().map(Value::as_str).collect()
}

fn parse_list<T>(value: &Value, parse: impl Fn(&str) -> Option<T>) -> Option<Vec<T>> {
    strings(value)?.into_iter().map(parse).collect()
}

fn error_kind(s: &str) -> Option<ProviderErrorKind> {
    Value::String(s.into()).try_into().ok()
}

fn error_kind_name(kind: ProviderErrorKind) -> String {
    match Value::try_from(kind) {
        Ok(Value::String(s)) => s,
        _ => "other".into(),
    }
}

fn parse_expr(value: &Value) -> Option<Expr> {
    let (key, body) = single(value)?;
    Some(match key {
        "all" => Expr::All(body.as_array()?.iter().map(expr_of).collect()),
        "any" => Expr::Any(body.as_array()?.iter().map(expr_of).collect()),
        "not" => Expr::Not(Box::new(expr_of(body))),
        "provider_error" => {
            let t = body.as_table()?;
            Expr::ProviderError {
                kinds: match t.get("kinds") {
                    Some(v) => parse_list(v, error_kind)?,
                    None => Vec::new(),
                },
                for_secs: match t.get("for_secs") {
                    Some(v) => u64::try_from(v.as_integer()?).ok()?,
                    None => 0,
                },
            }
        }
        "limit_used" => {
            let t = body.as_table()?;
            let pct = t.get("pct")?;
            Expr::LimitUsed {
                pct: pct
                    .as_float()
                    .or_else(|| pct.as_integer().map(|i| i as f64))? as f32,
                window_minutes: match t.get("window_minutes") {
                    Some(v) => Some(u32::try_from(v.as_integer()?).ok()?),
                    None => None,
                },
            }
        }
        "status" => Expr::Status(parse_list(body, AgentStatus::parse)?),
        "kind" => Expr::Kind(parse_list(body, AgentKind::parse)?),
        "model" => Expr::Model(parse_list(body, |s| Some(s.to_string()))?),
        _ => return None,
    })
}

fn expr_of(value: &Value) -> Expr {
    parse_expr(value).unwrap_or_else(|| Expr::Unknown(value.clone()))
}

fn one(key: &str, body: Value) -> Value {
    let mut t = Table::new();
    t.insert(key.into(), body);
    Value::Table(t)
}

fn list<T>(items: &[T], name: impl Fn(&T) -> String) -> Value {
    Value::Array(items.iter().map(|i| Value::String(name(i))).collect())
}

fn expr_value(expr: &Expr) -> Value {
    match expr {
        Expr::All(items) => one("all", Value::Array(items.iter().map(expr_value).collect())),
        Expr::Any(items) => one("any", Value::Array(items.iter().map(expr_value).collect())),
        Expr::Not(inner) => one("not", expr_value(inner)),
        Expr::ProviderError { kinds, for_secs } => {
            let mut t = Table::new();
            t.insert("kinds".into(), list(kinds, |k| error_kind_name(*k)));
            if *for_secs > 0 {
                t.insert("for_secs".into(), Value::Integer(*for_secs as i64));
            }
            one("provider_error", Value::Table(t))
        }
        Expr::LimitUsed {
            pct,
            window_minutes,
        } => {
            let mut t = Table::new();
            t.insert("pct".into(), Value::Float(*pct as f64));
            if let Some(w) = window_minutes {
                t.insert("window_minutes".into(), Value::Integer(*w as i64));
            }
            one("limit_used", Value::Table(t))
        }
        Expr::Status(items) => one("status", list(items, |s| s.as_str().to_string())),
        Expr::Kind(items) => one("kind", list(items, |k| k.as_str().to_string())),
        Expr::Model(items) => one("model", list(items, String::clone)),
        Expr::Unknown(raw) => raw.clone(),
    }
}

fn parse_action(value: &Value) -> Option<Action> {
    let (key, body) = single(value)?;
    let t = body.as_table()?;
    let model = || t.get("model").and_then(Value::as_str).map(str::to_string);
    Some(match key {
        "next_in_chain" => Action::NextInChain,
        "switch_model" => Action::SwitchModel { model: model()? },
        "continue_in" => Action::ContinueIn {
            kind: AgentKind::parse(t.get("kind")?.as_str()?)?,
            model: model(),
        },
        _ => return None,
    })
}

fn action_value(action: &Action) -> Value {
    let mut t = Table::new();
    let key = match action {
        Action::NextInChain => "next_in_chain",
        Action::SwitchModel { model } => {
            t.insert("model".into(), Value::String(model.clone()));
            "switch_model"
        }
        Action::ContinueIn { kind, model } => {
            t.insert("kind".into(), Value::String(kind.as_str().into()));
            if let Some(m) = model {
                t.insert("model".into(), Value::String(m.clone()));
            }
            "continue_in"
        }
        Action::Unknown(raw) => return raw.clone(),
    };
    one(key, Value::Table(t))
}

impl From<Value> for ExprValue {
    fn from(value: Value) -> Self {
        ExprValue(expr_of(&value))
    }
}

impl From<ExprValue> for Value {
    fn from(e: ExprValue) -> Self {
        expr_value(&e.0)
    }
}

impl From<Value> for ActionValue {
    fn from(value: Value) -> Self {
        ActionValue(parse_action(&value).unwrap_or(Action::Unknown(value)))
    }
}

impl From<ActionValue> for Value {
    fn from(a: ActionValue) -> Self {
        action_value(&a.0)
    }
}

pub fn parse(text: &str) -> Result<RouterFile, String> {
    let file: RouterFile = toml::from_str(text).map_err(|e| e.to_string())?;
    if file.schema_version > SCHEMA_VERSION {
        return Err(format!(
            "schema_version {} is newer than this berth understands ({SCHEMA_VERSION})",
            file.schema_version
        ));
    }
    Ok(file)
}

pub fn render(file: &RouterFile) -> Result<String, String> {
    toml::to_string_pretty(file).map_err(|e| e.to_string())
}

pub struct Facts<'a> {
    pub kind: AgentKind,
    pub model: Option<&'a str>,
    pub status: AgentStatus,
    pub error: Option<&'a ProviderError>,
    pub windows: &'a [UsageWindow],
    pub now_ms: i64,
}

fn used(windows: &[UsageWindow], kind: AgentKind, minutes: Option<u32>) -> f32 {
    windows
        .iter()
        .filter(|w| w.kind == kind && minutes.is_none_or(|m| m == w.window_minutes))
        .map(|w| w.used_percent)
        .fold(0.0, f32::max)
}

fn eval(expr: &Expr, f: &Facts) -> bool {
    match expr {
        Expr::All(items) => items.iter().all(|e| eval(e, f)),
        Expr::Any(items) => items.iter().any(|e| eval(e, f)),
        Expr::Not(inner) => !matches!(**inner, Expr::Unknown(_)) && !eval(inner, f),
        Expr::ProviderError { kinds, for_secs } => f.error.is_some_and(|e| {
            (kinds.is_empty() || kinds.contains(&e.kind))
                && f.now_ms - e.since_ms >= *for_secs as i64 * 1000
        }),
        Expr::LimitUsed {
            pct,
            window_minutes,
        } => used(f.windows, f.kind, *window_minutes) >= *pct,
        Expr::Status(items) => items.contains(&f.status),
        Expr::Kind(items) => items.contains(&f.kind),
        Expr::Model(items) => f
            .model
            .is_some_and(|m| items.iter().any(|i| i.eq_ignore_ascii_case(m))),
        Expr::Unknown(_) => false,
    }
}

fn target(entry: &str) -> Option<(AgentKind, Option<&str>)> {
    let (kind, model) = match entry.split_once(':') {
        Some((k, m)) => (k, Some(m.trim()).filter(|m| !m.is_empty())),
        None => (entry, None),
    };
    Some((AgentKind::parse(kind.trim())?, model))
}

fn next_in_chain(chain: &[String], f: &Facts) -> Option<RouteAction> {
    let targets = chain.iter().filter_map(|e| target(e)).collect::<Vec<_>>();
    let here = targets
        .iter()
        .position(|(k, m)| *k == f.kind && (m.is_none() || *m == f.model));
    let start = here.map_or(0, |i| i + 1);
    targets[start..]
        .iter()
        .find(|(k, m)| {
            !(*k == f.kind && (m.is_none() || *m == f.model))
                && (*k == f.kind || used(f.windows, *k, None) < FULL_PERCENT)
        })
        .map(|(kind, model)| {
            let model = model.map(str::to_string);
            if *kind == f.kind {
                RouteAction::SwitchModel { model }
            } else {
                RouteAction::ContinueIn { kind: *kind, model }
            }
        })
}

pub fn suggest(file: &RouterFile, f: &Facts) -> Option<RouteSuggestion> {
    file.rules
        .iter()
        .filter(|r| r.enabled && eval(&r.when.0, f))
        .find_map(|rule| {
            let action = rule.then.iter().find_map(|a| match &a.0 {
                Action::NextInChain => next_in_chain(&file.fallback, f),
                Action::SwitchModel { model } => {
                    (f.model != Some(model.as_str())).then(|| RouteAction::SwitchModel {
                        model: Some(model.clone()),
                    })
                }
                Action::ContinueIn { kind, model } => {
                    (*kind != f.kind).then(|| RouteAction::ContinueIn {
                        kind: *kind,
                        model: model.clone(),
                    })
                }
                Action::Unknown(_) => None,
            })?;
            Some(RouteSuggestion {
                rule: if rule.name.is_empty() {
                    rule.id.clone()
                } else {
                    rule.name.clone()
                },
                action,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(kind: AgentKind, pct: f32) -> UsageWindow {
        UsageWindow {
            kind,
            window_minutes: 300,
            used_percent: pct,
            resets_at_ms: 10_000,
        }
    }

    fn facts<'a>(
        kind: AgentKind,
        error: Option<&'a ProviderError>,
        windows: &'a [UsageWindow],
    ) -> Facts<'a> {
        Facts {
            kind,
            model: None,
            status: AgentStatus::Finished,
            error,
            windows,
            now_ms: 1_000,
        }
    }

    #[test]
    fn limit_at_95_suggests_the_next_chain_target() {
        let file = RouterFile::default();
        let windows = [window(AgentKind::Claude, 96.0)];
        let s = suggest(&file, &facts(AgentKind::Claude, None, &windows)).unwrap();
        assert_eq!(
            s.action,
            RouteAction::ContinueIn {
                kind: AgentKind::Codex,
                model: None
            }
        );
        let calm = [window(AgentKind::Claude, 40.0)];
        assert!(suggest(&file, &facts(AgentKind::Claude, None, &calm)).is_none());
    }

    #[test]
    fn exhausted_targets_are_skipped_and_the_chain_does_not_wrap() {
        let file = RouterFile::default();
        let windows = [
            window(AgentKind::Claude, 100.0),
            window(AgentKind::Codex, 99.0),
        ];
        let s = suggest(&file, &facts(AgentKind::Claude, None, &windows)).unwrap();
        assert_eq!(
            s.action,
            RouteAction::ContinueIn {
                kind: AgentKind::Grok,
                model: None
            }
        );
        let error = ProviderError {
            kind: ProviderErrorKind::Overloaded,
            since_ms: 0,
            message: String::new(),
            resets_at_ms: None,
        };
        assert!(suggest(&file, &facts(AgentKind::Grok, Some(&error), &[])).is_none());
    }

    #[test]
    fn model_downgrade_only_through_an_explicit_rule() {
        let text = r#"
schema_version = 1
fallback = ["claude:opus", "claude:sonnet", "codex"]

[[rule]]
id = "opus-overloaded"
when = { all = [{ model = ["opus"] }, { provider_error = { kinds = ["overloaded"], for_secs = 60 } }] }
then = [{ switch_model = { model = "sonnet" } }]
"#;
        let file = parse(text).unwrap();
        let error = ProviderError {
            kind: ProviderErrorKind::Overloaded,
            since_ms: 0,
            message: String::new(),
            resets_at_ms: None,
        };
        let mut f = facts(AgentKind::Claude, Some(&error), &[]);
        f.model = Some("opus");
        assert!(suggest(&file, &f).is_none());
        f.now_ms = 61_000;
        assert_eq!(
            suggest(&file, &f).unwrap().action,
            RouteAction::SwitchModel {
                model: Some("sonnet".into())
            }
        );
    }

    #[test]
    fn unknown_nodes_survive_a_round_trip() {
        let text = r#"
schema_version = 1
future_key = "kept"
fallback = ["claude", "codex"]

[[rule]]
id = "r"
note = "kept too"
when = { any = [{ time_of_day = { from = "22:00", to = "06:00" } }, { not = { status = ["running"] } }] }
then = [{ page_me = { channel = "x" } }, { next_in_chain = {} }]
"#;
        let file = parse(text).unwrap();
        let Expr::Any(items) = &file.rules[0].when.0 else {
            panic!("any")
        };
        assert!(matches!(items[0], Expr::Unknown(_)));
        assert!(matches!(file.rules[0].then[0].0, Action::Unknown(_)));
        let again = parse(&render(&file).unwrap()).unwrap();
        assert_eq!(again, file);
        let rendered = render(&again).unwrap();
        assert!(rendered.contains("time_of_day") && rendered.contains("page_me"));
        assert!(rendered.contains("future_key") && rendered.contains("kept too"));
    }

    #[test]
    fn default_file_round_trips_and_newer_schema_is_refused() {
        let file = RouterFile::default();
        assert_eq!(parse(&render(&file).unwrap()).unwrap(), file);
        assert!(parse("schema_version = 2").is_err());
    }
}
