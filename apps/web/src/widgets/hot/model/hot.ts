import { computed, shallowRef, watch, type ComputedRef } from 'vue';
import { externalTarget, feedbackText, readTranscript, useExternalSessions, useBerth, useSessionStates, type AgentKind, type Harness, type SessionState, type SessionStatus, useSessionDock } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';

type HotLabel = 'feedback' | 'working' | 'idle' | 'error' | 'limit';
export type HotItem = {
  key: string;
  title: string;
  project: string | null;
  branch: string | null;
  kind: AgentKind | null;
  status: SessionStatus;
  label: HotLabel;
  unseen: boolean;
  lastActivity: number | null;
  preview: string | null;
};

type Source = { harness: Harness; id: string };
type Entry = { state: SessionState; project: string | null; branch: string | null; source: Source | null; notice: string | null };

const MRU_LIMIT = 50;
const PREVIEW_LIMIT = 12;
const PREVIEW_CHARS = 140;
const NOTICE_CHARS = 300;
const PREVIEW_DEBOUNCE_MS = 800;
const TOAST_MS = 1800;

const LABELS: Partial<Record<SessionStatus, HotLabel>> = { needs_feedback: 'feedback', running: 'working', terminated: 'error', limit: 'limit' };

export const hotToast = shallowRef<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | undefined;

function showToast(text: string): void {
  hotToast.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    hotToast.value = null;
  }, TOAST_MS);
}

const needsYou = (item: Pick<HotItem, 'status' | 'unseen'>) => item.status === 'needs_feedback' || item.unseen;

const isHarness = (kind: AgentKind | undefined): kind is Harness => kind === 'claude' || kind === 'codex';

function previewOf(messages: readonly { role: string; text: string; tool: string | null }[]): string | null {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (!message) continue;
    const text = message.text.replace(/\s+/g, ' ').trim();
    if (message.role === 'assistant' && text) return text.slice(0, PREVIEW_CHARS);
    if (message.tool) return message.tool;
  }
  return null;
}

function create() {
  const berth = useBerth();
  const external = useExternalSessions();
  const states = useSessionStates();
  const dock = useDock();
  const sessions = useSessionDock();
  const order = shallowRef<ReadonlyMap<string, number>>(new Map());
  const previews = shallowRef<ReadonlyMap<string, string>>(new Map());
  const focused = shallowRef<string[]>([]);
  let counter = 0;

  const entries = computed<Entry[]>(() => {
    const out: Entry[] = [];
    const entities = berth?.entities.value;
    for (const state of states.value.values()) {
      if (!state.alive) continue;
      const node = berth?.sessions.value.get(state.key);
      if (node) {
        const path = sessions.paths.value.get(state.key);
        const agent = 'Agent' in node.ref ? entities?.agents.get(node.id) : undefined;
        const link = agent ? undefined : external?.linked.value.get(state.key);
        const source = agent?.session_id && isHarness(agent.kind) ? { harness: agent.kind, id: agent.session_id } : link?.running ? { harness: link.harness, id: link.id } : null;
        const request = agent && state.status === 'needs_feedback' ? berth?.feedback.value.get(node.id) : undefined;
        const failure = node.providerError;
        const notice = request ? (request.kind === 'Permission' && request.detail ? `${request.title}: ${request.detail}` : feedbackText(request)) : failure ? `${t(`agents.route.errorKind.${failure.kind}`)}: ${failure.message}` : null;
        out.push({ state, project: path?.project.name ?? null, branch: path?.worktree.branch ?? null, source, notice });
        continue;
      }
      const session = external?.anyByKey.value.get(state.key);
      if (!session) continue;
      const target = externalTarget(session, entities);
      out.push({ state, project: target?.projectName ?? null, branch: target?.branch ?? session.branch, source: { harness: session.harness, id: session.id }, notice: null });
    }
    return out;
  });

  watch(
    () => entries.value.map((entry) => entry.state.key),
    (keys) => {
      const previous = order.value;
      if (keys.length === previous.size && keys.every((key) => previous.has(key))) return;
      order.value = new Map(keys.map((key) => [key, previous.get(key) ?? ++counter]));
    },
    { immediate: true, flush: 'sync' },
  );

  const items = computed<HotItem[]>(() => {
    const seq = order.value;
    const rank = (state: SessionState) => (state.status === 'needs_feedback' ? 0 : state.unseen ? 1 : state.status === 'running' ? 2 : state.status === 'terminated' ? 3 : 4);
    const ranked = entries.value.map((entry) => {
      const { state } = entry;
      const item: HotItem = {
        key: state.key,
        title: state.name,
        project: entry.project,
        branch: entry.branch,
        kind: state.kind,
        status: state.status,
        label: LABELS[state.status] ?? 'idle',
        unseen: state.unseen,
        lastActivity: state.since,
        preview: entry.notice?.slice(0, NOTICE_CHARS) ?? previews.value.get(state.key) ?? null,
      };
      return { item, rank: rank(state), since: state.since ?? 0, seq: seq.get(state.key) ?? 0 };
    });
    ranked.sort((a, b) => {
      if (a.rank !== b.rank) return a.rank - b.rank;
      if (a.rank <= 1) return b.since - a.since;
      if (a.rank === 4) return b.since - a.since || a.seq - b.seq;
      return a.seq - b.seq;
    });
    return ranked.map((entry) => entry.item);
  });

  const waiting = computed(() => items.value.filter(needsYou));
  const keys = computed(() => new Set(items.value.map((item) => item.key)));

  const canonical = (key: string | null) => (key ? (external?.terminalOf.value.get(key) ?? key) : null);

  watch(
    () => canonical(dock.activeKey.value),
    (key) => {
      if (!key) return;
      focused.value = [key, ...focused.value.filter((entry) => entry !== key)].slice(0, MRU_LIMIT);
    },
    { immediate: true },
  );

  const mru = computed(() => {
    const live = keys.value;
    const recent = focused.value.filter((key) => live.has(key));
    const known = new Set(recent);
    return [...recent, ...items.value.map((item) => item.key).filter((key) => !known.has(key))];
  });

  const previewTargets = computed(() => {
    const byKey = new Map(entries.value.map((entry) => [entry.state.key, entry]));
    return items.value
      .flatMap((item) => {
        const entry = byKey.get(item.key);
        return entry?.source ? [entry] : [];
      })
      .slice(0, PREVIEW_LIMIT);
  });

  const fetched = new Map<string, string>();
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let previewRun = 0;

  async function refreshPreviews() {
    const run = ++previewRun;
    const present = new Set(entries.value.map((entry) => entry.state.key));
    for (const key of fetched.keys()) if (!present.has(key)) fetched.delete(key);
    if ([...previews.value.keys()].some((key) => !present.has(key))) previews.value = new Map([...previews.value].filter(([key]) => present.has(key)));
    for (const entry of previewTargets.value) {
      if (run !== previewRun) return;
      const source = entry.source;
      if (!source) continue;
      const stamp = `${entry.state.since}|${entry.state.status}`;
      if (fetched.get(entry.state.key) === stamp) continue;
      fetched.set(entry.state.key, stamp);
      const text = await readTranscript(source, 6).then(previewOf, () => null);
      if (run !== previewRun || !text || previews.value.get(entry.state.key) === text) continue;
      previews.value = new Map(previews.value).set(entry.state.key, text);
    }
  }

  watch(
    () => `${entries.value.map((entry) => `${entry.state.key}@${entry.state.since}|${entry.state.status}|${entry.source?.id ?? ''}`).join(',')}#${previewTargets.value.map((entry) => entry.state.key).join(',')}`,
    () => {
      clearTimeout(previewTimer);
      previewTimer = setTimeout(() => void refreshPreviews(), PREVIEW_DEBOUNCE_MS);
    },
    { immediate: true },
  );

  function next() {
    const list = waiting.value;
    if (!list.length) {
      showToast(t('hot.toast.nobody'));
      return;
    }
    const current = canonical(dock.activeKey.value);
    const index = list.findIndex((item) => item.key === current);
    const target = list[(index + 1) % list.length];
    if (!target) return;
    sessions.openKey(target.key);
    if (list.length > 1) showToast(t('hot.toast.waiting', { count: list.length }));
  }

  return {
    items: items as ComputedRef<HotItem[]>,
    waiting: waiting as ComputedRef<HotItem[]>,
    mru: mru as ComputedRef<string[]>,
    open: sessions.openKey,
    next,
  };
}

let store: ReturnType<typeof create> | null = null;

export function useHot() {
  store ??= create();
  return store;
}
