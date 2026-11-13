import { createChannel, hasBackend, invoke } from '../../../bridge';
import { computed, shallowRef, type ComputedRef, type ShallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';

export type AgentKind = 'claude' | 'codex' | 'cursor' | 'opencode' | 'grok';
type AgentStatus = 'fresh' | 'running' | 'finished' | 'needs_feedback' | 'terminated' | 'disconnected';
export type SessionRef = { Agent: string } | { Terminal: string };
type ConnectionState = 'connecting' | 'connected' | 'offline' | 'incompatible';
type Connection = { state: ConnectionState; message: string | null };

type Workspace = { id: string; name: string };
type Project = { id: string; name: string; workspace_id: string; repo_path: string; sort_order: number; pin_order: number | null };
export type Worktree = { id: string; project_id: string; path: string; branch: string; is_main: boolean; sort_order: number };
type Agent = {
  id: string;
  worktree_id: string;
  name: string;
  status: AgentStatus;
  archived: boolean;
  archived_at: number;
  unseen: boolean;
  status_changed_at: number;
  kind: AgentKind;
  model: string | null;
  effort: string | null;
  session_id: string | null;
  cloud_session_id: string | null;
  sort_order: number;
  alive: boolean;
};
type Terminal = { id: string; worktree_id: string; name: string; sort_order: number; alive: boolean; session_id?: string | null };

type Entity = { Workspace: Workspace } | { Project: Project } | { Worktree: Worktree } | { Agent: Agent } | { Terminal: Terminal } | { Link: unknown };
type EntityId = { Workspace: string } | { Project: string } | { Worktree: string } | { Agent: string } | { Terminal: string } | { Link: string };

export type FeedbackRequest = { kind: 'Permission' | 'Question'; title: string; detail: string | null; options: string[] };

type ArchivedExternal = { kind: AgentKind; session_id: string; archived_at: number };

type WatchEvent =
  | ({ type: 'connection' } & Connection)
  | { type: 'snapshot'; workspaces: Workspace[]; projects: Project[]; worktrees: Worktree[]; agents: Agent[]; terminals: Terminal[]; archived_external: ArchivedExternal[] }
  | { type: 'upserted'; entity: Entity }
  | { type: 'external_archived'; session: ArchivedExternal }
  | { type: 'external_unarchived'; kind: AgentKind; session_id: string }
  | { type: 'removed'; id: EntityId }
  | { type: 'status'; agent: string; status: AgentStatus; changed_at: number; unseen: boolean }
  | { type: 'feedback_request'; agent: string; request: FeedbackRequest | null }
  | { type: 'error'; message: string };

export type SessionEvent = { type: 'reset'; data: string } | { type: 'output'; data: string } | { type: 'exited'; exit_code: number | null };

export type SessionStatus = AgentStatus | 'shell' | 'closed';
export type SessionNode = { key: string; id: string; ref: SessionRef; name: string; status: SessionStatus; unseen: boolean; alive: boolean };
export type WorktreeNode = { id: string; path: string; branch: string; main: boolean; sessions: SessionNode[]; archived: SessionNode[] };
type ProjectNode = { id: string; name: string; worktrees: WorktreeNode[] };
type WorkspaceNode = { id: string; name: string; projects: ProjectNode[] };

type BerthStore = {
  entities: ComputedRef<Entities>;
  connection: ShallowRef<Connection>;
  error: ShallowRef<string | null>;
  tree: ComputedRef<WorkspaceNode[]>;
  sessions: ComputedRef<ReadonlyMap<string, SessionNode>>;
  archivedExternal: ComputedRef<ReadonlyMap<string, number>>;
  feedback: ComputedRef<ReadonlyMap<string, FeedbackRequest>>;
};

export type Entities = {
  workspaces: ReadonlyMap<string, Workspace>;
  projects: ReadonlyMap<string, Project>;
  worktrees: ReadonlyMap<string, Worktree>;
  agents: ReadonlyMap<string, Agent>;
  terminals: ReadonlyMap<string, Terminal>;
};

type MutableEntities = { [K in keyof Entities]: Entities[K] extends ReadonlyMap<string, infer T> ? Map<string, T> : never };

const byId = <T extends { id: string }>(items: T[]): Map<string, T> => new Map(items.map((item) => [item.id, item]));
const drop = <T>(map: ReadonlyMap<string, T>, id: string): ReadonlyMap<string, T> => {
  const next = new Map(map);
  next.delete(id);
  return next;
};
const bySortOrder = (a: { sort_order: number }, b: { sort_order: number }) => a.sort_order - b.sort_order;

export const externalArchiveKey = (kind: string, sessionId: string) => `${kind}:${sessionId}`;

export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function worktreesOfProject(entities: Entities, project: string): Worktree[] {
  return [...entities.worktrees.values()].filter((w) => w.project_id === project).sort((a, b) => Number(b.is_main) - Number(a.is_main) || bySortOrder(a, b));
}

const EMPTY: Entities = { workspaces: new Map(), projects: new Map(), worktrees: new Map(), agents: new Map(), terminals: new Map() };

function upsert(entities: MutableEntities, entity: Entity): boolean {
  if ('Workspace' in entity) entities.workspaces.set(entity.Workspace.id, entity.Workspace);
  else if ('Project' in entity) entities.projects.set(entity.Project.id, entity.Project);
  else if ('Worktree' in entity) entities.worktrees.set(entity.Worktree.id, entity.Worktree);
  else if ('Agent' in entity) entities.agents.set(entity.Agent.id, entity.Agent);
  else if ('Terminal' in entity) entities.terminals.set(entity.Terminal.id, entity.Terminal);
  else return false;
  return true;
}

function remove(entities: MutableEntities, id: EntityId): boolean {
  if ('Workspace' in id) return entities.workspaces.delete(id.Workspace);
  if ('Project' in id) return entities.projects.delete(id.Project);
  if ('Worktree' in id) return entities.worktrees.delete(id.Worktree);
  if ('Agent' in id) return entities.agents.delete(id.Agent);
  if ('Terminal' in id) return entities.terminals.delete(id.Terminal);
  return false;
}

function setStatus(entities: MutableEntities, id: string, status: AgentStatus, unseen: boolean): boolean {
  const agent = entities.agents.get(id);
  if (!agent || (agent.status === status && agent.unseen === unseen)) return false;
  entities.agents.set(id, { ...agent, status, unseen });
  return true;
}

export function sessionKey(session: SessionRef): string {
  return 'Agent' in session ? `agent:${session.Agent}` : `terminal:${session.Terminal}`;
}

export function sessionId(session: SessionRef): string {
  return 'Agent' in session ? session.Agent : session.Terminal;
}

export function isSessionRef(value: unknown): value is SessionRef {
  if (typeof value !== 'object' || value === null) return false;
  const id = 'Agent' in value ? value.Agent : 'Terminal' in value ? value.Terminal : undefined;
  return typeof id === 'string';
}

function agentNode(agent: Agent): SessionNode {
  const ref = { Agent: agent.id };
  return { key: sessionKey(ref), id: agent.id, ref, name: agent.name, status: agent.status, unseen: agent.unseen, alive: agent.alive };
}

function terminalNode(terminal: Terminal): SessionNode {
  const ref = { Terminal: terminal.id };
  return { key: sessionKey(ref), id: terminal.id, ref, name: terminal.name, status: terminal.alive ? 'shell' : 'closed', unseen: false, alive: terminal.alive };
}

function buildTree(entities: Entities): WorkspaceNode[] {
  const agentsOf = (worktree: string) => [...entities.agents.values()].filter((a) => a.worktree_id === worktree);
  const sessionsOf = (worktree: string) => [
    ...agentsOf(worktree).filter((a) => !a.archived).sort(bySortOrder).map(agentNode),
    ...[...entities.terminals.values()].filter((t) => t.worktree_id === worktree).sort(bySortOrder).map(terminalNode),
  ];
  const archivedOf = (worktree: string) =>
    agentsOf(worktree)
      .filter((a) => a.archived)
      .sort((a, b) => b.archived_at - a.archived_at)
      .map(agentNode);
  const worktreesOf = (project: string) =>
    worktreesOfProject(entities, project).flatMap((w) => {
      const node = { id: w.id, path: w.path, branch: w.branch, main: w.is_main, sessions: sessionsOf(w.id), archived: archivedOf(w.id) };
      return w.is_main || node.sessions.length > 0 || node.archived.length > 0 ? [node] : [];
    });
  const projectsOf = (workspace: string) =>
    [...entities.projects.values()]
      .filter((p) => p.workspace_id === workspace)
      .sort(bySortOrder)
      .map((p) => ({ id: p.id, name: p.name, worktrees: worktreesOf(p.id) }));
  return [...entities.workspaces.values()].map((w) => ({ id: w.id, name: w.name, projects: projectsOf(w.id) }));
}

let store: BerthStore | null = null;
let watching: Promise<void> = Promise.resolve();

function createStore(): BerthStore {
  let state: MutableEntities = { workspaces: new Map(), projects: new Map(), worktrees: new Map(), agents: new Map(), terminals: new Map() };
  const entities = shallowRef<Entities>(EMPTY);
  const publish = (changed: boolean) => {
    if (changed) entities.value = { ...state };
  };
  const connection = shallowRef<Connection>({ state: 'connecting', message: null });
  const error = shallowRef<string | null>(null);
  const archivedExternal = shallowRef<ReadonlyMap<string, number>>(new Map());
  const feedback = shallowRef<ReadonlyMap<string, FeedbackRequest>>(new Map());

  function receive(event: WatchEvent): void {
    if (event.type !== 'error') error.value = null;
    switch (event.type) {
      case 'connection':
        connection.value = { state: event.state, message: event.message };
        return;
      case 'snapshot':
        state = {
          workspaces: byId(event.workspaces),
          projects: byId(event.projects),
          worktrees: byId(event.worktrees),
          agents: byId(event.agents),
          terminals: byId(event.terminals),
        };
        publish(true);
        feedback.value = new Map();
        archivedExternal.value = new Map(event.archived_external.map((mark) => [externalArchiveKey(mark.kind, mark.session_id), mark.archived_at]));
        return;
      case 'external_archived':
        archivedExternal.value = new Map(archivedExternal.value).set(externalArchiveKey(event.session.kind, event.session.session_id), event.session.archived_at);
        return;
      case 'external_unarchived':
        archivedExternal.value = drop(archivedExternal.value, externalArchiveKey(event.kind, event.session_id));
        return;
      case 'upserted':
        publish(upsert(state, event.entity));
        return;
      case 'removed':
        publish(remove(state, event.id));
        return;
      case 'status':
        publish(setStatus(state, event.agent, event.status, event.unseen));
        return;
      case 'feedback_request':
        feedback.value = event.request ? new Map(feedback.value).set(event.agent, event.request) : drop(feedback.value, event.agent);
        return;
      case 'error':
        error.value = event.message;
        return;
      default: {
        const unknown: never = event;
        return unknown;
      }
    }
  }

  watching = invoke<void>('berth_watch', { onEvent: createChannel<WatchEvent>(receive) }).catch((e: unknown) => {
    error.value = describeError(e);
  });

  const tree = computed(() => buildTree(entities.value));
  const sessions = computed(() => new Map(tree.value.flatMap((w) => w.projects.flatMap((p) => p.worktrees.flatMap((t) => t.sessions.map((s) => [s.key, s] as const))))));
  return { entities: computed(() => entities.value), connection, error, tree, sessions, archivedExternal: computed(() => archivedExternal.value), feedback: computed(() => feedback.value) };
}

export function useBerth(): BerthStore | null {
  if (!hasBackend) return null;
  store ??= createStore();
  return store;
}

export async function attachSession(session: SessionRef, cols: number, rows: number, onEvent: (event: SessionEvent) => void): Promise<void> {
  await watching;
  await invoke<void>('berth_attach', { session, cols, rows, onEvent: createChannel<SessionEvent>(onEvent) });
}

export function detachSession(session: SessionRef): Promise<void> {
  return invoke<void>('berth_detach', { session });
}

export function sendInput(session: SessionRef, data: string): Promise<void> {
  return invoke<void>('berth_input', { session, data });
}

export function resizeSession(session: SessionRef, cols: number, rows: number): Promise<void> {
  return invoke<void>('berth_resize', { session, cols, rows });
}

export function markSeen(agent: string): Promise<void> {
  return invoke<void>('berth_mark_seen', { agent });
}

export type BerthRequest = { [variant: string]: Record<string, unknown> };

type OrphanedSession = {
  session_id: string;
  project_id: string;
  kind: AgentKind;
  name: string;
  branch: string;
  worktree_path: string;
  created_at: number;
  orphaned_at: number;
  transcript_bytes: number | null;
  live: boolean;
};
type PrewarmInfo = { worktree: string; kind: AgentKind; model: string | null };
type SessionMetrics = { session: SessionRef; pid: number; rss_bytes: number; procs: number; prewarm: PrewarmInfo | null };
type MetricsSnapshot = { daemon_pid: number; daemon_rss_bytes: number; system_total_bytes: number; sessions: SessionMetrics[]; sampled_at_ms: number };
type EnterOutcome = 'AlreadyThere' | 'Relocating' | 'NextLaunch';
export type Branch = { name: string; worktree: string | null };

export type BerthReply =
  | { Ack: { req_id: number; created: EntityId | null } }
  | { OrphanedSessions: { req_id: number; sessions: OrphanedSession[] } }
  | { Branches: { req_id: number; branches: Branch[] } }
  | { WorktreeEntered: { req_id: number; worktree: Worktree; outcome: EnterOutcome } }
  | { Metrics: { req_id: number; snapshot: MetricsSnapshot } };

export class BerthCallError extends Error {}

const REPLY_VARIANTS = new Set(['Ack', 'OrphanedSessions', 'Branches', 'WorktreeEntered', 'Metrics']);

function isBerthReply(value: unknown): value is BerthReply {
  if (typeof value !== 'object' || value === null) return false;
  const keys = Object.keys(value);
  return keys.length === 1 && REPLY_VARIANTS.has(keys[0] ?? '');
}

export async function call(request: BerthRequest): Promise<BerthReply> {
  const reply = await invoke<unknown>('berth_call', { request }).catch((e: unknown) => {
    throw new BerthCallError(describeError(e));
  });
  if (!isBerthReply(reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: JSON.stringify(reply) }));
  return reply;
}

export async function acknowledged(request: BerthRequest): Promise<EntityId | null> {
  const reply = await call(request);
  if (!('Ack' in reply)) throw new BerthCallError(t('agents.errors.expectedAck', { reply: Object.keys(reply).join('') }));
  return reply.Ack.created;
}

export type NewAgent = { worktree: string; name: string; kind: AgentKind; model: string | null; effort: string | null; autoTitle: boolean; startingPrompt: string | null };

export function createAgent(agent: NewAgent): Promise<EntityId | null> {
  return acknowledged({
    CreateAgent: {
      worktree: agent.worktree,
      name: agent.name,
      kind: agent.kind,
      model: agent.model,
      effort: agent.effort,
      auto_title: agent.autoTitle,
      cloud_prompt: null,
      starting_prompt: agent.startingPrompt,
    },
  });
}

export function createTerminal(worktree: string, name: string | null): Promise<EntityId | null> {
  return acknowledged({ CreateTerminal: { worktree, name } });
}

export async function renameSession(session: SessionRef, name: string): Promise<void> {
  await acknowledged('Agent' in session ? { RenameAgent: { id: session.Agent, name } } : { RenameTerminal: { id: session.Terminal, name } });
}

export async function restartAgent(id: string): Promise<void> {
  await acknowledged({ RestartAgent: { id } });
}

export async function archiveAgent(id: string): Promise<void> {
  await acknowledged({ ArchiveAgent: { id } });
}

export async function stopAgent(id: string): Promise<void> {
  await acknowledged({ StopAgent: { id } });
}

export async function unarchiveAgent(id: string): Promise<void> {
  await acknowledged({ UnarchiveAgent: { id } });
}

export async function archiveExternalSession(kind: AgentKind, sessionId: string): Promise<void> {
  await acknowledged({ ArchiveExternalSession: { kind, session_id: sessionId } });
}

export async function unarchiveExternalSession(kind: AgentKind, sessionId: string): Promise<void> {
  await acknowledged({ UnarchiveExternalSession: { kind, session_id: sessionId } });
}

export async function deleteAgent(id: string): Promise<void> {
  await acknowledged({ DeleteAgent: { id } });
}

export async function closeTerminal(id: string): Promise<void> {
  await acknowledged({ CloseTerminal: { id } });
}

export function entitySessionRef(id: EntityId | null): SessionRef | null {
  if (!id) return null;
  if ('Agent' in id) return { Agent: id.Agent };
  if ('Terminal' in id) return { Terminal: id.Terminal };
  return null;
}
