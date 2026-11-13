use super::rules::{self, RouterFile};
use anyhow::{Context, Result};
use serde::Deserialize;

pub fn to_json(file: &RouterFile) -> Result<String> {
    let text = rules::render(file).map_err(anyhow::Error::msg)?;
    let value: toml::Table = toml::from_str(&text)?;
    Ok(serde_json::to_string(&value)?)
}

pub fn from_json(json: &str) -> Result<(RouterFile, String)> {
    let value: toml::Value = serde_json::from_str(json).context("router config is not JSON")?;
    let file = RouterFile::deserialize(value).context("router config does not fit router.toml")?;
    let text = rules::render(&file).map_err(anyhow::Error::msg)?;
    let file = rules::parse(&text).map_err(anyhow::Error::msg)?;
    Ok((file, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_keeps_the_file_and_unknown_nodes() {
        let text = r#"
schema_version = 1
mode = "auto"
fallback = ["claude:opus", "codex"]
later = { a = 1 }

[[rule]]
id = "r"
note = "kept"
when = { all = [{ not = { time_of_day = { from = "22:00" } } }, { status = ["finished"] }] }
then = [{ next_in_chain = {} }]
"#;
        let file = rules::parse(text).unwrap();
        let json = to_json(&file).unwrap();
        let (again, rendered) = from_json(&json).unwrap();
        assert_eq!(again, file);
        assert_eq!(rendered, rules::render(&file).unwrap());
        assert!(from_json("{\"schema_version\": 9}").is_err());
        assert!(from_json("[]").is_err());
    }
}
