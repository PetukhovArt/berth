use super::types::Kind;
use super::{forge, gitlab, jira, Connectors};

fn jira_keys(branch: &str) -> Vec<String> {
    let upper = branch.to_uppercase();
    let bytes = upper.as_bytes();
    let mut keys = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        if boundary && bytes[i].is_ascii_uppercase() {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_uppercase() || bytes[j].is_ascii_digit()) {
                j += 1;
            }
            let digits = j + 1 < bytes.len() && bytes[j] == b'-' && bytes[j + 1].is_ascii_digit();
            if digits && j - i >= 2 {
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let key = upper[i..k].to_owned();
                if !keys.contains(&key) {
                    keys.push(key);
                }
                i = k;
                continue;
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    keys
}

fn gitlab_issue(branch: &str) -> Option<u64> {
    let (digits, _) = branch.rsplit('/').next()?.split_once('-')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

pub async fn tracker_comment(
    state: &Connectors,
    path: String,
    branch: String,
    body: String,
) -> Result<Option<String>, String> {
    let keys = jira_keys(&branch);
    if !keys.is_empty() {
        for record in state.records() {
            if record.kind != Kind::Jira {
                continue;
            }
            let Ok(ctx) = state.ctx(&record.id, &[Kind::Jira]).await else {
                continue;
            };
            for key in &keys {
                if jira::post_comment(&ctx, key, &body).await? {
                    return Ok(Some(key.clone()));
                }
            }
        }
    }
    if let Some(iid) = gitlab_issue(&branch) {
        if let Some((ctx, project)) = forge::worktree_forge(state, &path).await? {
            if ctx.kind == Kind::Gitlab && gitlab::issue_note(&ctx, &project, iid, &body).await? {
                return Ok(Some(format!("#{iid}")));
            }
        }
    }
    Ok(None)
}
