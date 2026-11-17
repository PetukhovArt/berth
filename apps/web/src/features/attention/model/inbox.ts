import { computed, ref } from 'vue';
import { externalDot, useBerth, useExternalSessions, useSessionDock, type SessionNode, type SessionStatus } from '../../../entities/session';
import { attentionKindOf, type AttentionKind } from './attention';

export type InboxEntry = { key: string; node: SessionNode; status: SessionStatus; kind: AttentionKind; since: number; place: string; until?: number | undefined };

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
  const external = useExternalSessions();
  if (!ticking) {
    ticking = true;
    setInterval(() => (now.value = Date.now()), TICK_MS);
  }
  const entries = computed<InboxEntry[]>(() => {
    if (!berth) return [];
    const { agents, terminals, worktrees, projects } = berth.entities.value;
    const linked = external?.linked.value;
    const list: InboxEntry[] = [];
    for (const node of berth.sessions.value.values()) {
      let worktreeId: string;
      let changedAt: number;
      let kind: AttentionKind | null;
      let status: SessionStatus = node.status;
      if ('Agent' in node.ref) {
        const agent = agents.get(node.id);
        if (!agent || agent.archived) continue;
        worktreeId = agent.worktree_id;
        changedAt = agent.status_changed_at;
        kind = attentionKindOf(node.status, node.unseen);
      } else {
        const terminal = terminals.get(node.id);
        const link = linked?.get(node.key);
        if (!terminal || !link) continue;
        worktreeId = terminal.worktree_id;
        changedAt = link.updated_at;
        status = externalDot(link);
        kind = attentionKindOf(status, false);
      }
      if (!kind) continue;
      const known = since.get(node.key);
      const at = known?.sig === kind ? known.at : known ? Date.now() : changedAt ? toMillis(changedAt) : Date.now();
      since.set(node.key, { sig: kind, at });
      const worktree = worktrees.get(worktreeId);
      const project = worktree ? projects.get(worktree.project_id) : undefined;
      list.push({ key: node.key, node, status, kind, since: at, place: [project?.name, worktree?.branch].filter(Boolean).join(' › '), until: node.limitUntil });
    }
    return list.sort((a, b) => b.since - a.since);
  });
  const items = computed(() => entries.value.filter((entry) => !hidden(entry, now.value)));
  return { items, now };
}

export function openNextWaiting(): void {
  const dock = useSessionDock();
  const active = dock.activeSession.value?.key;
  const next = useInbox()
    .items.value.filter((entry) => entry.kind === 'waiting' && entry.node.key !== active)
    .sort((a, b) => a.since - b.since)[0];
  if (next) dock.openSession(next.node);
}
