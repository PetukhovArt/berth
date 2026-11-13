import { computed, shallowRef, watch, type ComputedRef } from 'vue';
import { acpPrompt, isAcpAgent, sendInput, useBerth, type SessionRef } from './berth';

export type QueuedPrompt = { id: number; text: string };

const queues = shallowRef<ReadonlyMap<string, readonly QueuedPrompt[]>>(new Map());
let nextId = 0;
let watching = false;

function setQueue(agent: string, items: readonly QueuedPrompt[]) {
  const next = new Map(queues.value);
  if (items.length) next.set(agent, items);
  else next.delete(agent);
  queues.value = next;
}

export function submitPrompt(session: SessionRef, text: string): Promise<void> {
  if ('Agent' in session && isAcpAgent(session.Agent)) return acpPrompt(session.Agent, text);
  return sendInput(session, `\x1b[200~${text}\x1b[201~`).then(() => sendInput(session, '\r'));
}

function flush(agent: string) {
  const [head, ...rest] = queues.value.get(agent) ?? [];
  if (!head) return;
  setQueue(agent, rest);
  void submitPrompt({ Agent: agent }, head.text);
}

function watchStatuses() {
  if (watching) return;
  const store = useBerth();
  if (!store) return;
  watching = true;
  let previous = new Map<string, string>();
  watch(
    () => store.entities.value.agents,
    (agents) => {
      const current = new Map<string, string>();
      for (const [id, agent] of agents) {
        current.set(id, agent.status);
        if (previous.get(id) === 'running' && agent.status === 'finished') flush(id);
      }
      previous = current;
    },
    { immediate: true },
  );
}

export function enqueuePrompt(agent: string, text: string) {
  watchStatuses();
  setQueue(agent, [...(queues.value.get(agent) ?? []), { id: nextId++, text }]);
}

export function removeQueuedPrompt(agent: string, id: number) {
  setQueue(
    agent,
    (queues.value.get(agent) ?? []).filter((item) => item.id !== id),
  );
}

export function useAgentBusy(agent: string): ComputedRef<boolean> {
  return computed(() => useBerth()?.entities.value.agents.get(agent)?.status === 'running');
}

export function useQueuedPrompts(agent: string): ComputedRef<readonly QueuedPrompt[]> {
  return computed(() => queues.value.get(agent) ?? []);
}
