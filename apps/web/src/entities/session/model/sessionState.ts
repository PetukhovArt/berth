import { computed, type ComputedRef } from 'vue';
import { limitUntil } from './limits';
import { externalKey, useExternalSessions, type ExternalSession } from './external';
import { useBerth, type AgentKind, type ProviderError, type SessionNode, type SessionStatus } from './berth';

export type SessionState = {
  key: string;
  name: string;
  status: SessionStatus;
  unseen: boolean;
  alive: boolean;
  since: number | null;
  kind: AgentKind | null;
  limitUntil?: number;
  providerError?: ProviderError;
};

function withLimit(state: SessionState, providerError: ProviderError | undefined): SessionState {
  const seenFinish = state.status === 'finished' && !state.unseen;
  if (providerError && (seenFinish || state.status === 'running' || state.status === 'fresh')) {
    const folded: SessionState = { ...state, status: 'limit', since: providerError.since_ms, providerError };
    if (providerError.resets_at_ms) folded.limitUntil = providerError.resets_at_ms;
    return folded;
  }
  const until = seenFinish && state.alive && state.kind ? limitUntil(state.kind) : null;
  return until === null ? state : { ...state, status: 'limit', limitUntil: until };
}

function agentState(node: SessionNode, connected: boolean): SessionState {
  const state: SessionState = { key: node.key, name: node.name, status: node.status, unseen: node.unseen, alive: connected && node.alive && node.status !== 'disconnected', since: node.changedAt ?? null, kind: node.kind ?? null };
  return withLimit(state, node.providerError);
}

function terminalState(node: SessionNode, link: ExternalSession | undefined, connected: boolean): SessionState {
  const alive = connected && node.alive;
  if (!link?.running) return { key: node.key, name: node.name, status: node.status, unseen: false, alive, since: null, kind: null };
  return withLimit({ key: node.key, name: node.name, status: link.status, unseen: false, alive, since: link.updated_at, kind: link.harness }, undefined);
}

function externalState(session: ExternalSession): SessionState {
  return withLimit({ key: externalKey(session), name: session.title, status: session.status, unseen: false, alive: session.running, since: session.updated_at, kind: session.harness }, undefined);
}

let states: ComputedRef<ReadonlyMap<string, SessionState>> | null = null;

export function useSessionStates(): ComputedRef<ReadonlyMap<string, SessionState>> {
  states ??= computed(() => {
    const berth = useBerth();
    const external = useExternalSessions();
    const connected = berth?.connection.value.state === 'connected';
    const linked = external?.linked.value;
    const map = new Map<string, SessionState>();
    for (const node of berth?.sessions.value.values() ?? []) map.set(node.key, 'Agent' in node.ref ? agentState(node, connected) : terminalState(node, linked?.get(node.key), connected));
    for (const session of external?.byKey.value.values() ?? []) map.set(externalKey(session), externalState(session));
    return map;
  });
  return states;
}

export function sessionStateOf(key: string): SessionState | undefined {
  return useSessionStates().value.get(key);
}
