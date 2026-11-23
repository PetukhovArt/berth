mod acp;
mod indexer;
mod query;

use std::path::PathBuf;

pub use indexer::run;
pub use query::search_conversations;

fn db_path() -> PathBuf {
    berth_core::paths::search_index_db()
}
