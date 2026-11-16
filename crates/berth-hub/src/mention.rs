use std::ops::Range;

pub fn mention_key(name: &str) -> String {
    let mut key = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        } else if !key.is_empty() && !key.ends_with('-') {
            key.push('-');
        }
    }
    key.trim_end_matches('-').to_string()
}

pub fn mentions_in(content: &str) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    for (at, _) in content.match_indices('@') {
        let start = at + 1;
        let len: usize = content[start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .map(char::len_utf8)
            .sum();
        let key = mention_key(&content[start..start + len]);
        if !key.is_empty() {
            out.push((at..start + len, key));
        }
    }
    out
}
