use crate::acp::driver::options::{self, Model};
use serde_json::Value;

const DEFAULT_MODEL: &str = "default";
const DEFAULT_EFFORT: &str = "medium";
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

pub fn static_models() -> Vec<Model> {
    ["default", "sonnet", "opus", "haiku"]
        .iter()
        .map(|value| Model {
            value: value.to_string(),
            name: value.to_string(),
            description: None,
            efforts: EFFORTS.iter().map(|e| e.to_string()).collect(),
            default_effort: None,
            images: true,
        })
        .collect()
}

pub fn models_from(initialized: &Value) -> Vec<Model> {
    let text =
        |model: &Value, key: &str| model.get(key).and_then(Value::as_str).map(str::to_string);
    initialized
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let value = text(model, "value")?;
            Some(Model {
                name: text(model, "displayName").unwrap_or_else(|| value.clone()),
                description: text(model, "description"),
                efforts: model
                    .get("supportedEffortLevels")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.as_str().map(str::to_string))
                    .collect(),
                default_effort: None,
                images: true,
                value,
            })
        })
        .collect()
}

pub fn config_options(
    models: &[Model],
    model: Option<&str>,
    effort: Option<&str>,
    resolved: Option<&str>,
) -> Value {
    let current = model.unwrap_or(DEFAULT_MODEL);
    let label = resolved
        .filter(|_| current != DEFAULT_MODEL)
        .and_then(model_label);
    let mut named: Vec<Model> = models
        .iter()
        .cloned()
        .map(|mut choice| {
            if let (true, Some(label)) = (choice.value == current, &label) {
                choice.name = label.clone();
            }
            choice
        })
        .collect();
    if !models.iter().any(|choice| choice.value == current) {
        if let Some(base) = models
            .iter()
            .find(|choice| choice.value != DEFAULT_MODEL && current.contains(choice.value.as_str()))
        {
            named.push(Model {
                value: current.into(),
                name: model_label(current).unwrap_or_else(|| current.into()),
                ..base.clone()
            });
        }
    }
    options::config_options(&named, current, effort.unwrap_or(DEFAULT_EFFORT))
}

pub fn is_real_model(id: &str) -> bool {
    id.starts_with("claude-")
}

fn model_label(id: &str) -> Option<String> {
    let rest = id.strip_prefix("claude-")?;
    let rest = rest.trim_end_matches("[1m]");
    let rest = match rest.rsplit_once('-') {
        Some((head, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => rest,
    };
    let family = rest
        .split('-')
        .find(|part| part.chars().all(char::is_alphabetic))?;
    let version: Vec<&str> = rest
        .split('-')
        .filter(|part| part.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    let mut name = family.to_string();
    name[..1].make_ascii_uppercase();
    Some(if version.is_empty() {
        name
    } else {
        format!("{name} {}", version.join("."))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_model_is_named_after_what_ran() {
        let options = config_options(
            &static_models(),
            Some("haiku"),
            None,
            Some("claude-haiku-4-5-20251001"),
        );
        let names: Vec<&str> = options[0]["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|choice| choice["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["default", "sonnet", "opus", "Haiku 4.5"]);
    }

    #[test]
    fn a_full_model_id_is_its_own_choice() {
        let options = config_options(
            &static_models(),
            Some("claude-haiku-4-5-20251001"),
            None,
            None,
        );
        let choices = options[0]["options"].as_array().unwrap();
        let last = choices.last().unwrap();
        assert_eq!(last["value"], "claude-haiku-4-5-20251001");
        assert_eq!(last["name"], "Haiku 4.5");
        assert_eq!(choices.len(), 5);
        assert_eq!(options[0]["currentValue"], "claude-haiku-4-5-20251001");
        assert_eq!(options.as_array().unwrap().len(), 2);
    }
}
