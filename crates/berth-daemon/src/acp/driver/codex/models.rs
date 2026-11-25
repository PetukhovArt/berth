use crate::acp::driver::options::Model;
use crate::acp::driver::translator::str_at;
use serde_json::Value;

pub fn listed(result: &Value) -> Vec<Model> {
    let mut models: Vec<(bool, Model)> = result
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|model| model.get("hidden").and_then(Value::as_bool) != Some(true))
        .filter_map(|model| {
            let value = str_at(model, "model").or(str_at(model, "id"))?.to_string();
            let default = model.get("isDefault").and_then(Value::as_bool) == Some(true);
            let images = model
                .get("inputModalities")
                .and_then(Value::as_array)
                .is_none_or(|kinds| kinds.iter().any(|kind| kind == "image"));
            let efforts = model
                .get("supportedReasoningEfforts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|option| str_at(option, "reasoningEffort").map(str::to_string))
                .collect();
            Some((
                default,
                Model {
                    name: str_at(model, "displayName").unwrap_or(&value).to_string(),
                    description: str_at(model, "description").map(str::to_string),
                    default_effort: str_at(model, "defaultReasoningEffort").map(str::to_string),
                    efforts,
                    images,
                    value,
                },
            ))
        })
        .collect();
    models.sort_by_key(|(default, _)| !default);
    models.into_iter().map(|(_, model)| model).collect()
}
