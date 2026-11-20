use std::fmt::Display;

use serde_json::{Map, Value};

const OPEN: char = '\u{1e}';
const CLOSE: char = '\u{1f}';

pub fn coded(code: &str, params: &[(&str, &dyn Display)]) -> String {
    let params: Map<String, Value> = params
        .iter()
        .map(|(name, value)| ((*name).to_owned(), Value::String(value.to_string())))
        .collect();
    let body = serde_json::json!({ "code": code, "params": params });
    format!("{OPEN}{body}{CLOSE}")
}

#[macro_export]
macro_rules! host_text {
    ($code:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::host_text::coded($code, &[$((stringify!($name), &$value as &dyn std::fmt::Display)),*])
    };
}
