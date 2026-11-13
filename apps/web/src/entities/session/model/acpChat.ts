import { computed, onScopeDispose, shallowRef, type ComputedRef, type ShallowRef } from 'vue';
import { acpTranscript, describeError, onAcpItem, type AcpItem, type AcpPlanEntry } from './berth';
import type { TranscriptMessage } from './external';
import type { PlanItem } from './plan';

const OUTPUT_CHARS = 4000;

const PLAN_STATES: Record<string, PlanItem['state']> = { completed: 'done', in_progress: 'doing', pending: 'todo' };

type AcpChat = { messages: ComputedRef<TranscriptMessage[]>; plan: ComputedRef<PlanItem[]>; loading: ShallowRef<boolean>; error: ShallowRef<string | null>; reload: () => Promise<void> };

const fenced = (text: string, lang: string) => {
  const clipped = text.length > OUTPUT_CHARS ? `${text.slice(0, OUTPUT_CHARS)}\n…` : text;
  const fence = clipped.includes('```') ? '````' : '```';
  return `${fence}${lang}\n${clipped}\n${fence}`;
};

const quoted = (text: string) =>
  text
    .split('\n')
    .map((line) => `> ${line}`)
    .join('\n');

function diffText(diff: { path: string; old: string | null; new: string }): string {
  const removed = (diff.old ?? '').split('\n').filter((line, index, lines) => line || index < lines.length - 1);
  const added = diff.new.split('\n');
  return [`--- ${diff.path}`, `+++ ${diff.path}`, ...removed.map((line) => `-${line}`), ...added.map((line) => `+${line}`)].join('\n');
}

function toMessages(seq: number, item: AcpItem): TranscriptMessage[] {
  const key = `acp:${seq}`;
  const base = { key, at: null, tool: null };
  if ('User' in item) return [{ ...base, role: 'user', text: item.User.text }];
  if ('Assistant' in item) return [{ ...base, role: 'assistant', text: item.Assistant.text }];
  if ('Thought' in item) return [{ ...base, role: 'assistant', text: quoted(item.Thought.text) }];
  if ('Notice' in item) return [{ ...base, role: 'assistant', text: `_${item.Notice.text}_` }];
  if ('Plan' in item) return [];
  const tool = item.Tool;
  const label = [tool.detail, tool.status === 'completed' ? '' : tool.status].filter(Boolean).join(' · ');
  const out: TranscriptMessage[] = [{ ...base, role: 'assistant', tool: tool.title || tool.kind, text: label }];
  if (tool.diff) out.push({ ...base, key: `${key}:diff`, role: 'assistant', text: tool.diff.path, shell: 'output', markdown: fenced(diffText(tool.diff), 'diff') });
  if (tool.output) out.push({ ...base, key: `${key}:output`, role: 'assistant', text: tool.output, shell: 'output', markdown: fenced(tool.output, '') });
  return out;
}

const toPlan = (entries: AcpPlanEntry[]): PlanItem[] => entries.map((entry) => ({ text: entry.content, state: PLAN_STATES[entry.status] ?? 'todo' }));

export function useAcpChat(agent: string): AcpChat {
  const items = shallowRef<ReadonlyMap<number, AcpItem>>(new Map());
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);

  const put = (seq: number, item: AcpItem) => {
    items.value = new Map(items.value).set(seq, item);
  };
  onScopeDispose(onAcpItem((from, at) => from === agent && put(at.seq, at.item)));

  async function reload() {
    try {
      const loaded = await acpTranscript(agent, 0);
      const next = new Map(items.value);
      for (const at of loaded) if (!next.has(at.seq)) next.set(at.seq, at.item);
      items.value = next;
      error.value = null;
    } catch (e) {
      error.value = describeError(e);
    } finally {
      loading.value = false;
    }
  }
  void reload();

  const ordered = computed(() => [...items.value].sort(([a], [b]) => a - b));
  const messages = computed(() => ordered.value.flatMap(([seq, item]) => toMessages(seq, item)));
  const plan = computed(() => {
    const last = ordered.value.findLast(([, item]) => 'Plan' in item)?.[1];
    return last && 'Plan' in last ? toPlan(last.Plan.entries) : [];
  });
  return { messages, plan, loading, error, reload };
}
