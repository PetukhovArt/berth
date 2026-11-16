import { shallowRef, toValue, watch, type MaybeRefOrGetter, type ShallowRef } from 'vue';
import { acpTranscript, type AgentKind } from './berth';
import { acpMessages } from './acpChat';
import { readTranscript, type TranscriptMessage } from './external';

type Source = { id: string; kind: AgentKind; session_id: string | null };

async function load(source: Source, tail: number): Promise<TranscriptMessage[]> {
  if (source.kind === 'acp') return acpMessages(await acpTranscript(source.id, 0));
  if ((source.kind === 'claude' || source.kind === 'codex') && source.session_id) return readTranscript({ harness: source.kind, id: source.session_id }, tail);
  return [];
}

export function useAgentMessages(source: MaybeRefOrGetter<Source | undefined>, tail: number, refreshKey?: MaybeRefOrGetter<unknown>): ShallowRef<TranscriptMessage[]> {
  const messages = shallowRef<TranscriptMessage[]>([]);
  let run = 0;

  async function refresh() {
    const current = toValue(source);
    const mine = ++run;
    if (!current) {
      messages.value = [];
      return;
    }
    try {
      const next = await load(current, tail);
      if (mine === run) messages.value = next;
    } catch {
      if (mine === run) messages.value = [];
    }
  }

  watch(
    () => [toValue(source)?.id, toValue(source)?.session_id, toValue(refreshKey)] as const,
    (next, previous) => {
      if (previous && next[0] !== previous[0]) messages.value = [];
      void refresh();
    },
    { immediate: true },
  );
  return messages;
}
