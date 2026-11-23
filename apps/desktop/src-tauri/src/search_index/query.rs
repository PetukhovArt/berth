use berth_transcripts::search::{search, store, SearchReply};
use tokio::task::spawn_blocking;

use super::db_path;

const DEFAULT_LIMIT: u32 = 200;
const MAX_LIMIT: u32 = 500;

fn search_index(query: &str, limit: u32) -> Result<SearchReply, String> {
    if crate::ssh_remote::target().is_some() {
        return Ok(SearchReply::remote());
    }
    let Some(conn) = store::open_reader(&db_path())? else {
        return Ok(SearchReply::default());
    };
    search(&conn, query, limit).map_err(|e| e.to_string())
}

pub async fn search_conversations(
    query: String,
    limit: Option<u32>,
) -> Result<SearchReply, String> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    spawn_blocking(move || search_index(&query, limit))
        .await
        .map_err(|e| e.to_string())?
}
