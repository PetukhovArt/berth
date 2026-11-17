import { shallowRef, watch, type ShallowRef } from 'vue';
import { limitUntil } from './limits';
import { externalKey, useExternalSessions, type ExternalSession } from './external';
import { useBerth, type AgentKind, type ProviderError, type SessionNode, type SessionStatus } from './berth';

type Reported = Exclude<SessionStatus, 'limit'>;
type Conversation = { status: Reported; at: number; harness: AgentKind };

type Phase =
  | { name: 'fresh' }
  | { name: 'running' }
  | { name: 'needs_feedback' }
  | { name: 'finished' }
  | { name: 'finished_unseen' }
  | { name: 'limit'; until: number | null; providerError: ProviderError | null }
  | { name: 'crashed' }
  | { name: 'disconnected' }
  | { name: 'shell' }
  | { name: 'closed' };

type PhaseName = Phase['name'];

type SessionStateEvent =
  | { type: 'status'; status: Reported; unseen: boolean; at: number | null }
  | { type: 'seen' }
  | { type: 'process'; alive: boolean }
  | { type: 'provider_error'; error: ProviderError | null }
  | { type: 'usage_window'; until: number | null }
  | { type: 'connection'; connected: boolean }
  | { type: 'conversation'; conversation: Conversation | null };

type Context = { status: Reported; unseen: boolean; at: number | null; running: boolean; connected: boolean; providerError: ProviderError | null; window: number | null; conversation: Conversation | null; agentKind: AgentKind | null };

export type SessionState = {
  key: string;
  name: string;
  phase: Phase;
  status: SessionStatus;
  unseen: boolean;
  alive: boolean;
  since: number | null;
  kind: AgentKind | null;
  limitUntil?: number;
  providerError?: ProviderError;
  context: Context;
};

const MOVES: Readonly<Record<Exclude<SessionStateEvent['type'], 'status'>, readonly PhaseName[]>> = {
  seen: ['finished_unseen'],
  process: ['finished', 'limit'],
  provider_error: ['fresh', 'running', 'finished', 'limit'],
  usage_window: ['finished', 'limit'],
  connection: [],
  conversation: ['shell', 'closed', 'running', 'finished', 'limit', 'needs_feedback'],
};

const SHOWN: Readonly<Record<PhaseName, SessionStatus>> = {
  fresh: 'fresh',
  running: 'running',
  needs_feedback: 'needs_feedback',
  finished: 'finished',
  finished_unseen: 'finished',
  limit: 'limit',
  crashed: 'terminated',
  disconnected: 'disconnected',
  shell: 'shell',
  closed: 'closed',
};

function settle(context: Context): Phase {
  const reported = context.conversation?.status ?? context.status;
  const kind = context.conversation?.harness ?? context.agentKind;
  const error = context.conversation ? null : context.providerError;
  switch (reported) {
    case 'terminated':
      return { name: 'crashed' };
    case 'fresh':
    case 'running':
      return error ? { name: 'limit', until: error.resets_at_ms ?? null, providerError: error } : { name: reported };
    case 'finished': {
      if (context.unseen && !context.conversation) return { name: 'finished_unseen' };
      if (error) return { name: 'limit', until: error.resets_at_ms ?? null, providerError: error };
      return context.running && kind && context.window ? { name: 'limit', until: context.window, providerError: null } : { name: 'finished' };
    }
    default:
      return { name: reported };
  }
}

function absorb(context: Context, event: SessionStateEvent): Context {
  switch (event.type) {
    case 'status':
      return { ...context, status: event.status, unseen: event.unseen, at: event.at };
    case 'seen':
      return { ...context, unseen: false };
    case 'process':
      return { ...context, running: event.alive };
    case 'provider_error':
      return { ...context, providerError: event.error };
    case 'usage_window':
      return { ...context, window: event.until };
    case 'connection':
      return { ...context, connected: event.connected };
    case 'conversation':
      return { ...context, conversation: event.conversation };
  }
}

function view(key: string, name: string, phase: Phase, context: Context): SessionState {
  const live = context.conversation ? context.running : context.running && context.status !== 'disconnected';
  const state: SessionState = {
    key,
    name,
    phase,
    status: SHOWN[phase.name],
    unseen: phase.name === 'finished_unseen',
    alive: context.connected && live,
    since: phase.name === 'limit' && phase.providerError ? phase.providerError.since_ms : (context.conversation?.at ?? context.at),
    kind: context.conversation?.harness ?? context.agentKind,
    context,
  };
  if (phase.name === 'limit' && phase.until !== null) state.limitUntil = phase.until;
  if (phase.name === 'limit' && phase.providerError) state.providerError = phase.providerError;
  return state;
}

function next(state: SessionState, event: SessionStateEvent): SessionState {
  const context = absorb(state.context, event);
  const moves = event.type === 'status' || MOVES[event.type].includes(state.phase.name);
  return view(state.key, state.name, moves ? settle(context) : state.phase, context);
}

type Input = { name: string; status: Reported; unseen: boolean; at: number | null; running: boolean; connected: boolean; providerError: ProviderError | null; window: number | null; conversation: Conversation | null; agentKind: AgentKind | null };

function eventsBetween(before: Input, after: Input): SessionStateEvent[] {
  const events: SessionStateEvent[] = [];
  if (before.status !== after.status || before.at !== after.at || (!before.unseen && after.unseen)) events.push({ type: 'status', status: after.status, unseen: after.unseen, at: after.at });
  else if (before.unseen && !after.unseen) events.push({ type: 'seen' });
  if (before.running !== after.running) events.push({ type: 'process', alive: after.running });
  if (before.providerError !== after.providerError) events.push({ type: 'provider_error', error: after.providerError });
  if (before.window !== after.window) events.push({ type: 'usage_window', until: after.window });
  if (before.connected !== after.connected) events.push({ type: 'connection', connected: after.connected });
  const was = before.conversation;
  const now = after.conversation;
  if (was?.status !== now?.status || was?.at !== now?.at || was?.harness !== now?.harness) events.push({ type: 'conversation', conversation: now });
  return events;
}

function start(key: string, input: Input): SessionState {
  const context: Context = { status: input.status, unseen: input.unseen, at: input.at, running: input.running, connected: input.connected, providerError: input.providerError, window: input.window, conversation: input.conversation, agentKind: input.agentKind };
  return view(key, input.name, settle(context), context);
}

function agentInput(node: SessionNode, connected: boolean): Input {
  const kind = node.kind ?? null;
  return { name: node.name, status: node.status, unseen: node.unseen, at: node.changedAt ?? null, running: node.alive, connected, providerError: node.providerError ?? null, window: kind ? limitUntil(kind) : null, conversation: null, agentKind: kind };
}

function terminalInput(node: SessionNode, link: ExternalSession | undefined, connected: boolean): Input {
  const conversation = link?.running ? { status: link.status, at: link.updated_at, harness: link.harness } : null;
  return { name: node.name, status: node.status, unseen: false, at: null, running: node.alive, connected, providerError: null, window: conversation ? limitUntil(conversation.harness) : null, conversation, agentKind: null };
}

function externalInput(session: ExternalSession): Input {
  return { name: session.title, status: session.status, unseen: false, at: session.updated_at, running: session.running, connected: true, providerError: null, window: limitUntil(session.harness), conversation: null, agentKind: session.harness };
}

let states: ShallowRef<ReadonlyMap<string, SessionState>> | null = null;

function createStates(): ShallowRef<ReadonlyMap<string, SessionState>> {
  const berth = useBerth();
  const external = useExternalSessions();
  const map = shallowRef<ReadonlyMap<string, SessionState>>(new Map());
  let last = new Map<string, Input>();
  const inputs = () => {
    const connected = berth?.connection.value.state === 'connected';
    const linked = external?.linked.value;
    const found = new Map<string, Input>();
    for (const node of berth?.sessions.value.values() ?? []) found.set(node.key, 'Agent' in node.ref ? agentInput(node, connected) : terminalInput(node, linked?.get(node.key), connected));
    for (const session of external?.byKey.value.values() ?? []) found.set(externalKey(session), externalInput(session));
    return found;
  };
  watch(
    inputs,
    (found) => {
      const previous = map.value;
      const following = new Map<string, SessionState>();
      for (const [key, input] of found) {
        const known = previous.get(key);
        const before = last.get(key);
        const state = known && before ? eventsBetween(before, input).reduce(next, known) : start(key, input);
        following.set(key, state.name === input.name ? state : { ...state, name: input.name });
      }
      last = found;
      map.value = following;
    },
    { immediate: true, flush: 'sync' },
  );
  return map;
}

export function useSessionStates(): ShallowRef<ReadonlyMap<string, SessionState>> {
  states ??= createStates();
  return states;
}

export function sessionStateOf(key: string): SessionState | undefined {
  return useSessionStates().value.get(key);
}
