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

pub fn config_options(models: &[Model], model: Option<&str>, effort: Option<&str>) -> Value {
    options::config_options(
        models,
        model.unwrap_or(DEFAULT_MODEL),
        effort.unwrap_or(DEFAULT_EFFORT),
    )
}
