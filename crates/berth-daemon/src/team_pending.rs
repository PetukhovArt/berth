use anyhow::Result;
use berth_hub::Mention;
use rusqlite::params;

use crate::store::Store;

pub(crate) const PENDING_MAX: i64 = 64;

pub(crate) struct Pending {
    pub mention: Mention,
    pub promoted: bool,
}

pub(crate) fn queue_and_advance(
    store: &Store,
    hub_url: &str,
    found: &[Mention],
    cursor: i64,
) -> Result<()> {
    let mut conn = store.conn();
    let tx = conn.transaction()?;
    let mut room: i64 = PENDING_MAX
        - tx.query_row(
            "SELECT COUNT(*) FROM hub_mention_pending WHERE hub_url = ?1",
            params![hub_url],
            |r| r.get::<_, i64>(0),
        )?;
    for m in found {
        if room <= 0 {
            break;
        }
        room -= tx.execute(
            "INSERT OR IGNORE INTO hub_mention_pending (hub_url, seq, member, mention)
             VALUES (?1, ?2, ?3, ?4)",
            params![hub_url, m.event.seq, m.agent, serde_json::to_string(m)?],
        )? as i64;
    }
    tx.execute(
        "INSERT INTO hub_mention_cursor (hub_url, seq) VALUES (?1, ?2)
         ON CONFLICT(hub_url) DO UPDATE SET seq = excluded.seq",
        params![hub_url, cursor],
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn promote(store: &Store, hub_url: &str, mentions: &[Mention]) -> Result<()> {
    let mut conn = store.conn();
    let tx = conn.transaction()?;
    for m in mentions {
        tx.execute(
            "INSERT INTO hub_mention_pending (hub_url, seq, member, mention, promoted)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(hub_url, seq, member) DO UPDATE SET promoted = 1",
            params![hub_url, m.event.seq, m.agent, serde_json::to_string(m)?],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn load(store: &Store, hub_url: &str) -> Result<Vec<Pending>> {
    let conn = store.conn();
    let mut st = conn.prepare(
        "SELECT mention, promoted FROM hub_mention_pending WHERE hub_url = ?1
         ORDER BY seq, member",
    )?;
    let rows = st.query_map(params![hub_url], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
    })?;
    let mut pending = Vec::new();
    for row in rows {
        let (json, promoted) = row?;
        match serde_json::from_str(&json) {
            Ok(mention) => pending.push(Pending { mention, promoted }),
            Err(e) => tracing::warn!(error = %e, "skipping an unreadable pending mention"),
        }
    }
    Ok(pending)
}

pub(crate) fn remove(store: &Store, hub_url: &str, m: &Mention) -> Result<()> {
    store.conn().execute(
        "DELETE FROM hub_mention_pending WHERE hub_url = ?1 AND seq = ?2 AND member = ?3",
        params![hub_url, m.event.seq, m.agent],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_hub::Event;

    const HUB: &str = "https://hub.example";

    fn mention(seq: i64, agent: &str) -> Mention {
        Mention {
            event: Event {
                seq,
                kind: 1,
                author: "m-ann".into(),
                author_name: "ann".into(),
                author_kind: "person".into(),
                channel: "PROJ-1".into(),
                tags: vec![],
                content: format!("@{agent} hi"),
                created_at: 0,
            },
            agent: format!("a-{agent}"),
            agent_name: agent.into(),
            from_owner: true,
            chain: 0,
        }
    }

    fn seqs(store: &Store) -> Vec<(i64, bool)> {
        load(store, HUB)
            .unwrap()
            .into_iter()
            .map(|p| (p.mention.event.seq, p.promoted))
            .collect()
    }

    #[test]
    fn fetched_mentions_and_promotions_survive_a_restart_and_are_delivered_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("berth.db");
        {
            let store = Store::open(&path).unwrap();
            queue_and_advance(&store, HUB, &[mention(7, "one"), mention(9, "two")], 9).unwrap();
            promote(&store, HUB, &[mention(5, "one")]).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.mention_cursor(HUB).unwrap(), Some(9));
        assert_eq!(seqs(&store), [(5, true), (7, false), (9, false)]);
        assert!(load(&store, "https://other.example").unwrap().is_empty());

        remove(&store, HUB, &mention(7, "one")).unwrap();
        promote(&store, HUB, &[mention(9, "two")]).unwrap();
        queue_and_advance(&store, HUB, &[mention(9, "two")], 9).unwrap();
        drop(store);

        let store = Store::open(&path).unwrap();
        assert_eq!(seqs(&store), [(5, true), (9, true)]);
    }

    #[test]
    fn the_queue_stops_at_its_cap_but_the_cursor_moves_on() {
        let store = Store::open_in_memory().unwrap();
        let found: Vec<Mention> = (1..=PENDING_MAX + 5).map(|s| mention(s, "one")).collect();
        queue_and_advance(&store, HUB, &found, PENDING_MAX + 5).unwrap();
        assert_eq!(load(&store, HUB).unwrap().len() as i64, PENDING_MAX);
        assert_eq!(store.mention_cursor(HUB).unwrap(), Some(PENDING_MAX + 5));
    }
}
