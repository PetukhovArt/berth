import { computed, ref } from 'vue';
import { attentionOf, externalTarget, urgencyOf, useBerth, useExternalSessions, useSessionDock, useSessionStates, type AttentionKind, type SessionNode, type SessionState } from '../../../entities/session';
import { useDock } from '../../../shared/model/dock';

export type InboxEntry = { key: string; state: SessionState; kind: AttentionKind; since: number | null; place: string; node: SessionNode | undefined };

type Mark = { sig: string; until: number | null };

const STORE_KEY = 'berth.inbox.marks';
const TICK_MS = 30_000;

const now = ref(Date.now());
const marks = ref<Record<string, Mark>>(readMarks());
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

const signature = (entry: Pick<InboxEntry, 'kind' | 'since'>) => `${entry.kind}:${entry.since ?? ''}`;

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

function createInbox() {
  const berth = useBerth();
  const external = useExternalSessions();
  const states = useSessionStates();
  const { paths } = useSessionDock();
  if (!ticking) {
    ticking = true;
    setInterval(() => (now.value = Date.now()), TICK_MS);
  }
  const entries = computed<InboxEntry[]>(() => {
    const list: InboxEntry[] = [];
    for (const state of states.value.values()) {
      const kind = attentionOf(state);
      if (!kind) continue;
      const node = berth?.sessions.value.get(state.key);
      const path = paths.value.get(state.key);
      const conversation = node ? undefined : external?.anyByKey.value.get(state.key);
      const target = conversation ? externalTarget(conversation, berth?.entities.value) : null;
      const place = path ? [path.project.name, path.worktree.branch] : [target?.projectName, target?.branch];
      list.push({ key: state.key, state, kind, since: state.since, place: place.filter(Boolean).join(' › '), node });
    }
    return list.sort((a, b) => (b.since ?? 0) - (a.since ?? 0));
  });
  const items = computed(() => entries.value.filter((entry) => !hidden(entry, now.value)));
  return { items, now };
}

let inbox: ReturnType<typeof createInbox> | null = null;

export function useInbox() {
  inbox ??= createInbox();
  return inbox;
}

export function openNextNeedingYou(): void {
  const active = useDock().activeKey.value;
  const current = active ? (useExternalSessions()?.terminalOf.value.get(active) ?? active) : null;
  const next = useInbox()
    .items.value.filter((entry) => entry.key !== current)
    .sort((a, b) => urgencyOf(b.state) - urgencyOf(a.state) || (a.since ?? 0) - (b.since ?? 0))[0];
  if (next) useSessionDock().openKey(next.key);
}
