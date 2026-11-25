import { computed, onScopeDispose, shallowRef, triggerRef, type ComputedRef, type ShallowRef } from 'vue';
import { acpEditQueued, acpMcpStatus, acpPrompt, acpRemoveQueued, acpSendQueuedNow, acpSessionState, acpTranscript, answerFeedbackForm, describeError, onAcpItem, onAcpQueueCleared, onAcpSession, onAcpTruncated, setConfigOption, type AcpAnswers, type AcpAttachmentUpload, type AcpItemAt, type AcpSessionState } from './berth';
import { acpMessages, acpPlan, acpRewinds, type RewindOption } from './acpChat';
import type { TranscriptMessage } from './external';
import type { PlanItem } from './plan';

export type Chat = {
  messages: ComputedRef<TranscriptMessage[]>;
  plan: ComputedRef<PlanItem[]>;
  rewinds: ComputedRef<Map<string, RewindOption>>;
  session: ShallowRef<AcpSessionState | null>;
  loading: ShallowRef<boolean>;
  error: ShallowRef<string | null>;
  reload: () => Promise<void>;
  send: (text: string, uploads?: AcpAttachmentUpload[]) => Promise<void>;
  answer: (answers: AcpAnswers) => Promise<void>;
  setOption: (id: string, value: string) => Promise<void>;
  editQueued: (id: number, text: string) => Promise<void>;
  removeQueued: (id: number) => Promise<void>;
  sendQueuedNow: (id: number) => Promise<void>;
  refreshMcp: () => Promise<void>;
};

function slot(items: readonly AcpItemAt[], seq: number): number {
  let low = 0;
  let high = items.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (items[mid]!.seq < seq) low = mid + 1;
    else high = mid;
  }
  return low;
}

export function useChat(agent: string, onQueueCleared?: (texts: string[]) => void): Chat {
  const items = shallowRef<AcpItemAt[]>([]);
  const session = shallowRef<AcpSessionState | null>(null);
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);
  let frame = 0;

  const flush = () => {
    frame = 0;
    triggerRef(items);
  };
  const schedule = () => {
    if (document.hidden) flush();
    else if (!frame) frame = requestAnimationFrame(flush);
  };
  const put = (at: AcpItemAt) => {
    const list = items.value;
    const index = slot(list, at.seq);
    if (list[index]?.seq === at.seq) list[index] = at;
    else list.splice(index, 0, at);
    schedule();
  };

  let live = false;
  onScopeDispose(onAcpItem((from, at) => from === agent && put(at)));
  onScopeDispose(
    onAcpTruncated((from, fromSeq) => {
      if (from !== agent) return;
      items.value = items.value.filter((at) => at.seq < fromSeq);
    }),
  );
  onScopeDispose(onAcpQueueCleared((from, texts) => from === agent && onQueueCleared?.(texts)));
  onScopeDispose(
    onAcpSession((from, state) => {
      if (from !== agent) return;
      live = true;
      session.value = state;
    }),
  );
  onScopeDispose(() => cancelAnimationFrame(frame));
  void acpSessionState(agent)
    .then((state) => {
      if (!live) session.value = state;
    })
    .catch(() => undefined);

  async function reload() {
    try {
      const loaded = await acpTranscript(agent, 0);
      const known = new Set(items.value.map((at) => at.seq));
      items.value = [...items.value, ...loaded.filter((at) => !known.has(at.seq))].sort((a, b) => a.seq - b.seq);
      error.value = null;
    } catch (e) {
      error.value = describeError(e);
    } finally {
      loading.value = false;
    }
  }
  void reload();

  return {
    messages: computed(() => acpMessages(items.value)),
    plan: computed(() => acpPlan(items.value)),
    rewinds: computed(() => acpRewinds(items.value)),
    session,
    loading,
    error,
    reload,
    send: (text, uploads = []) => acpPrompt(agent, text, uploads),
    answer: (answers) => answerFeedbackForm(agent, answers),
    setOption: (id, value) => setConfigOption(agent, id, value),
    editQueued: (id, text) => acpEditQueued(agent, id, text),
    removeQueued: (id) => acpRemoveQueued(agent, id),
    sendQueuedNow: (id) => acpSendQueuedNow(agent, id),
    refreshMcp: () => acpMcpStatus(agent),
  };
}
