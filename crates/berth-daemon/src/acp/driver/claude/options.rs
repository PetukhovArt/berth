use serde_json::{json, Value};

const DEFAULT_MODEL: &str = "default";
const DEFAULT_EFFORT: &str = "medium";
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub efforts: Vec<String>,
}

pub fn static_models() -> Vec<Model> {
    ["default", "sonnet", "opus", "haiku"]
        .iter()
        .map(|value| Model {
            value: value.to_string(),
            name: value.to_string(),
            description: None,
            efforts: EFFORTS.iter().map(|e| e.to_string()).collect(),
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
                value,
            })
        })
        .collect()
}

pub fn config_options(models: &[Model], model: Option<&str>, effort: Option<&str>) -> Value {
    let current = model.unwrap_or(DEFAULT_MODEL);
    let mut options = vec![json!({
        "id": "model",
        "name": "Model",
        "category": "model",
        "type": "select",
        "currentValue": current,
        "options": models.iter().map(|m| json!({
            "value": m.value,
            "name": m.name,
            "description": m.description,
        })).collect::<Vec<_>>(),
    })];
    let efforts = models
        .iter()
        .find(|m| m.value == current)
        .map(|m| m.efforts.as_slice())
        .unwrap_or_default();
    if !efforts.is_empty() {
        options.push(json!({
            "id": "effort",
            "name": "Effort",
            "category": "thought_level",
            "type": "select",
            "currentValue": effort.unwrap_or(DEFAULT_EFFORT),
            "options": efforts.iter().map(|e| json!({"value": e, "name": e})).collect::<Vec<_>>(),
        }));
    }
    Value::Array(options)
}
