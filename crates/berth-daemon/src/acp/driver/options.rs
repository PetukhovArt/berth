use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub images: bool,
}

pub fn config_options(models: &[Model], model: &str, effort: &str) -> Value {
    let mut options = vec![json!({
        "id": "model",
        "name": "Model",
        "category": "model",
        "type": "select",
        "currentValue": model,
        "options": models.iter().map(|m| json!({
            "value": m.value,
            "name": m.name,
            "description": m.description,
        })).collect::<Vec<_>>(),
    })];
    let efforts = models
        .iter()
        .find(|m| m.value == model)
        .map(|m| m.efforts.as_slice())
        .unwrap_or_default();
    if !efforts.is_empty() {
        options.push(json!({
            "id": "effort",
            "name": "Effort",
            "category": "thought_level",
            "type": "select",
            "currentValue": effort,
            "options": efforts.iter().map(|e| json!({"value": e, "name": e})).collect::<Vec<_>>(),
        }));
    }
    Value::Array(options)
}
