use super::map::{apply_patch, TextKind, ToolPatch, Update};
use berth_core::acp::{AcpItem, AcpItemAt, AcpTool};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TEXT_EMIT_GAP: Duration = Duration::from_millis(200);

struct OpenText {
    seq: u64,
    kind: TextKind,
    text: String,
    emitted_at: Instant,
    unsent: bool,
}

pub struct Transcript {
    last_seq: u64,
    open: Option<OpenText>,
    tools: HashMap<String, (u64, AcpTool)>,
}

impl Transcript {
    pub fn after(last_seq: u64) -> Self {
        Self {
            last_seq,
            open: None,
            tools: HashMap::new(),
        }
    }

    pub fn push(&mut self, item: AcpItem) -> Vec<AcpItemAt> {
        let mut out = self.flush();
        self.open = None;
        out.push(AcpItemAt {
            seq: self.next_seq(),
            item,
        });
        out
    }

    pub fn apply(&mut self, update: Update, now: Instant) -> Vec<AcpItemAt> {
        match update {
            Update::Text(kind, chunk) => self.text(kind, chunk, now),
            Update::Tool(patch) => self.tool(patch),
            Update::Plan(entries) => self.push(AcpItem::Plan { entries }),
            Update::Title(_) | Update::Ignored => Vec::new(),
        }
    }

    pub fn flush(&mut self) -> Vec<AcpItemAt> {
        match &mut self.open {
            Some(open) if open.unsent => {
                open.unsent = false;
                vec![open_item(open)]
            }
            _ => Vec::new(),
        }
    }

    fn text(&mut self, kind: TextKind, chunk: String, now: Instant) -> Vec<AcpItemAt> {
        if let Some(open) = self.open.as_mut().filter(|o| o.kind == kind) {
            open.text.push_str(&chunk);
            if now.duration_since(open.emitted_at) < TEXT_EMIT_GAP {
                open.unsent = true;
                return Vec::new();
            }
            open.emitted_at = now;
            open.unsent = false;
            return vec![open_item(open)];
        }
        let mut out = self.flush();
        let open = OpenText {
            seq: self.next_seq(),
            kind,
            text: chunk,
            emitted_at: now,
            unsent: false,
        };
        out.push(open_item(&open));
        self.open = Some(open);
        out
    }

    fn tool(&mut self, patch: ToolPatch) -> Vec<AcpItemAt> {
        if self.tools.contains_key(&patch.id) {
            let mut out = self.flush();
            let (seq, tool) = self.tools.get_mut(&patch.id).expect("checked above");
            apply_patch(tool, patch);
            out.push(AcpItemAt {
                seq: *seq,
                item: AcpItem::Tool(tool.clone()),
            });
            return out;
        }
        let mut tool = AcpTool {
            id: patch.id.clone(),
            status: "pending".into(),
            kind: "other".into(),
            ..Default::default()
        };
        apply_patch(&mut tool, patch);
        let out = self.push(AcpItem::Tool(tool.clone()));
        if let Some(last) = out.last() {
            self.tools.insert(tool.id.clone(), (last.seq, tool));
        }
        out
    }

    fn next_seq(&mut self) -> u64 {
        self.last_seq += 1;
        self.last_seq
    }
}

fn open_item(open: &OpenText) -> AcpItemAt {
    let text = open.text.clone();
    AcpItemAt {
        seq: open.seq,
        item: match open.kind {
            TextKind::User => AcpItem::User { text },
            TextKind::Assistant => AcpItem::Assistant { text },
            TextKind::Thought => AcpItem::Thought { text },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(text: &str) -> Update {
        Update::Text(TextKind::Assistant, text.into())
    }

    #[test]
    fn chunks_coalesce_into_one_item_and_the_tail_flushes() {
        let mut t = Transcript::after(10);
        let now = Instant::now();
        let first = t.apply(chunk("Hel"), now);
        assert_eq!(first[0].seq, 11);
        assert!(t.apply(chunk("lo"), now).is_empty());
        let tail = t.flush();
        assert_eq!(
            tail,
            vec![AcpItemAt {
                seq: 11,
                item: AcpItem::Assistant {
                    text: "Hello".into()
                }
            }]
        );
        assert!(t.flush().is_empty());
    }

    #[test]
    fn a_tool_closes_the_text_run_and_its_update_keeps_the_seq() {
        let mut t = Transcript::after(0);
        let now = Instant::now();
        t.apply(chunk("a"), now);
        t.apply(chunk("b"), now);
        let call = t.apply(
            Update::Tool(ToolPatch {
                id: "t".into(),
                title: Some("Run".into()),
                ..Default::default()
            }),
            now,
        );
        assert_eq!(call.len(), 2);
        assert_eq!(call[1].seq, 2);
        let update = t.apply(
            Update::Tool(ToolPatch {
                id: "t".into(),
                status: Some("completed".into()),
                ..Default::default()
            }),
            now,
        );
        let AcpItem::Tool(tool) = &update[0].item else {
            panic!("not a tool")
        };
        assert_eq!((update[0].seq, tool.status.as_str()), (2, "completed"));
        assert_eq!(t.apply(chunk("c"), now)[0].seq, 3);
    }
}
