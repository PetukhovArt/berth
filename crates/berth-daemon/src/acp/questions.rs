use anyhow::{bail, ensure, Context, Result};
use berth_core::acp::{Answers, Choice, Question, QuestionForm};
use serde_json::{json, Map, Value};

pub fn form_from_schema(schema: &Value) -> Option<QuestionForm> {
    let properties = schema.get("properties")?.as_object()?;
    let questions: Option<Vec<Question>> = properties
        .iter()
        .map(|(id, property)| question(id, property))
        .collect();
    questions
        .filter(|questions| !questions.is_empty())
        .map(|questions| QuestionForm { questions })
}

pub fn option_names(form: &QuestionForm) -> Vec<String> {
    match form.questions.as_slice() {
        [only] => only
            .choices
            .iter()
            .take(crate::feedback::MAX_OPTIONS)
            .map(|choice| choice.name.clone())
            .collect(),
        _ => Vec::new(),
    }
}

pub fn answers_for_option(form: &QuestionForm, option: usize) -> Result<Answers> {
    let [only] = form.questions.as_slice() else {
        bail!("this form has more than one question");
    };
    let choice = only.choices.get(option).context("no such option")?;
    Ok(Answers::from([(
        only.id.clone(),
        vec![choice.value.clone()],
    )]))
}

pub fn content(form: &QuestionForm, answers: &Answers) -> Result<Value> {
    ensure!(
        answers
            .keys()
            .all(|id| form.questions.iter().any(|q| &q.id == id)),
        "an answer names a question the form does not have"
    );
    let mut content = Map::new();
    for question in &form.questions {
        let values = answers
            .get(&question.id)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| anyhow::anyhow!("\"{}\" is not answered", question.header))?;
        ensure!(
            question.multi_select || values.len() == 1,
            "\"{}\" takes one answer",
            question.header
        );
        ensure!(
            question.free_text_allowed
                || values
                    .iter()
                    .all(|value| question.choices.iter().any(|c| &c.value == value)),
            "\"{}\" has an answer outside its choices",
            question.header
        );
        let value = if question.multi_select {
            json!(values)
        } else {
            json!(values[0])
        };
        content.insert(question.id.clone(), value);
    }
    Ok(Value::Object(content))
}

fn question(id: &str, property: &Value) -> Option<Question> {
    let header = str_at(property, "title").unwrap_or(id).to_string();
    let text = str_at(property, "description")
        .unwrap_or(&header)
        .to_string();
    let (choices, free_text, multi_select) = match str_at(property, "type")? {
        "string" => {
            let (choices, free) = choices_of(property);
            let free = free || choices.is_empty();
            (choices, free, false)
        }
        "array" => {
            let (choices, free) = choices_of(property.get("items")?);
            if choices.is_empty() && !free {
                return None;
            }
            (choices, free, true)
        }
        _ => return None,
    };
    Some(Question {
        id: id.to_string(),
        header,
        text,
        choices,
        multi_select,
        free_text_allowed: free_text,
    })
}

fn choices_of(schema: &Value) -> (Vec<Choice>, bool) {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let names = schema.get("enumNames").and_then(Value::as_array);
        let choices = values
            .iter()
            .enumerate()
            .filter_map(|(at, value)| {
                let value = value.as_str()?;
                Some(Choice {
                    value: value.to_string(),
                    name: names
                        .and_then(|names| names.get(at))
                        .and_then(Value::as_str)
                        .unwrap_or(value)
                        .to_string(),
                    description: None,
                })
            })
            .collect();
        return (choices, false);
    }
    let entries = ["oneOf", "anyOf"]
        .iter()
        .find_map(|key| schema.get(key).and_then(Value::as_array));
    let Some(entries) = entries else {
        return (Vec::new(), false);
    };
    let mut free = false;
    let mut choices = Vec::new();
    for entry in entries {
        match str_at(entry, "const") {
            Some(value) => choices.push(Choice {
                value: value.to_string(),
                name: str_at(entry, "title").unwrap_or(value).to_string(),
                description: str_at(entry, "description").map(str::to_string),
            }),
            None => free = free || str_at(entry, "type") == Some("string"),
        }
    }
    (choices, free)
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
