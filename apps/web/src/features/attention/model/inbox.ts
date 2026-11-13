import { computed, ref } from 'vue';
import { useBerth, type SessionNode } from '../../../entities/session';
import { attentionKindOf, type AttentionKind } from './attention';

export type InboxEntry = { key: string; node: SessionNode; kind: AttentionKind; since: number; place: string; until?: number | undefined };

type Mark = { sig: string; until: number | null };

const STORE_KEY = 'berth.inbox.marks';
const TICK_MS = 30_000;
const SECONDS_LIMIT = 1e11;

const now = ref(Date.now());
const marks = ref<Record<string, Mark>>(readMarks());
const since = new Map<string, { sig: string; at: number }>();
let ticking = false;

function readMarks(): Record<string, Mark> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? '{}');
    return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, Mark>) : {};
  } catch {
    return {};
  }
}

function writeMarks(next: Record<string, Mark>): void {
  marks.value = next;
  localStorage.setItem(STORE_KEY, JSON.stringify(next));
}

const toMillis = (value: number) => (value > 0 && value < SECONDS_LIMIT ? value * 1000 : value);

const signature = (entry: Pick<InboxEntry, 'kind' | 'since'>) => `${entry.kind}:${entry.since}`;

export function snoozeInbox(entry: InboxEntry, ms: number): void {
  now.value = Date.now();
  writeMarks({ ...marks.value, [entry.key]: { sig: signature(entry), until: now.value + ms } });
}

export function settleInbox(entry: InboxEntry): void {
  writeMarks({ ...marks.value, [entry.key]: { sig: signature(entry), until: null } });
}

function hidden(entry: InboxEntry, at: number): boolean {
  const mark = marks.value[entry.key];
  if (!mark || mark.sig !== signature(entry)) return false;
  return mark.until === null || mark.until > at;
}

export function useInbox() {
  const berth = useBerth();
  if (!ticking) {
    ticking = true;
    setInterval(() => (now.value = Date.now()), TICK_MS);
  }
  const entries = computed<InboxEntry[]>(() => {
    if (!berth) return [];
    const { agents, worktrees, projects } = berth.entities.value;
    const list: InboxEntry[] = [];
    for (const node of berth.sessions.value.values()) {
      if (!('Agent' in node.ref)) continue;
      const agent = agents.get(node.id);
      if (!agent || agent.archived) continue;
      const kind = attentionKindOf(node.status, node.unseen);
      if (!kind) continue;
      const known = since.get(node.key);
      const at = known?.sig === kind ? known.at : known ? Date.now() : agent.status_changed_at ? toMillis(agent.status_changed_at) : Date.now();
      since.set(node.key, { sig: kind, at });
      const worktree = worktrees.get(agent.worktree_id);
      const project = worktree ? projects.get(worktree.project_id) : undefined;
      list.push({ key: node.key, node, kind, since: at, place: [project?.name, worktree?.branch].filter(Boolean).join(' › '), until: node.limitUntil });
    }
    return list.sort((a, b) => b.since - a.since);
  });
  const items = computed(() => entries.value.filter((entry) => !hidden(entry, now.value)));
  return { items, now };
}
