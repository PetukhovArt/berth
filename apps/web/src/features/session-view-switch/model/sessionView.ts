import { shallowRef } from 'vue';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { hasChanges } from './worktreeChanged';
import { knownMr } from '../../../entities/merge-request';
import { useDock } from '../../../shared/model/dock';
import { useSessionDock, useBerth, externalDot, sessionDot, useExternalSessions, externalKey, type Harness, type SessionRef } from '../../../entities/session';

export type SessionWorktree = { path: string; branch: string };

export function worktreeOf(panelId: string): SessionWorktree | null {
  const worktreeId = useSessionDock().paths.value.get(panelId)?.worktree.id;
  const worktree = worktreeId ? useBerth()?.entities.value.worktrees.get(worktreeId) : undefined;
  if (worktree) return { path: worktree.path, branch: worktree.branch };
  const conversation = useExternalSessions()?.anyByKey.value.get(panelId);
  return conversation?.branch ? { path: conversation.cwd, branch: conversation.branch } : null;
}

function mrOf(panelId: string): ChangeRequestRef | null {
  const worktree = worktreeOf(panelId);
  const summary = worktree ? knownMr(worktree.path, worktree.branch) : null;
  return summary ? { connector: summary.connector, project: summary.project, number: summary.number } : null;
}

function statusOf(panelId: string) {
  const external = useExternalSessions();
  const node = useBerth()?.sessions.value.get(panelId);
  if (node) return sessionDot(node, external?.linked.value.get(panelId));
  const conversation = external?.anyByKey.value.get(panelId);
  return conversation ? externalDot(conversation) : undefined;
}

function acpAgentOf(panelId: string): string | null {
  const ref = useBerth()?.sessions.value.get(panelId)?.ref;
  if (!ref || !('Agent' in ref)) return null;
  return useBerth()?.entities.value.agents.get(ref.Agent)?.kind === 'acp' ? ref.Agent : null;
}

function chatOf(panelId: string): { harness: Harness | 'acp'; sessionId: string; terminal: SessionRef } | null {
  const acp = acpAgentOf(panelId);
  if (acp) return { harness: 'acp', sessionId: acp, terminal: { Agent: acp } };
  const external = useExternalSessions();
  const berth = useBerth();
  const session = berth?.sessions.value.get(panelId);
  if (!session) return null;
  const linked = external?.linked.value.get(panelId);
  if (linked) return { harness: linked.harness, sessionId: linked.id, terminal: session.ref };
  if (!('Agent' in session.ref)) return null;
  const agent = berth?.entities.value.agents.get(session.ref.Agent);
  if (agent?.kind !== 'claude' || !agent.session_id) return null;
  const own = external?.anyByKey.value.get(externalKey({ harness: 'claude', id: agent.session_id }));
  return own ? { harness: own.harness, sessionId: own.id, terminal: session.ref } : null;
}

const SESSION_VIEWS = {
  terminal: {
    letter: 'T',
    available: (panelId: string) => acpAgentOf(panelId) === null,
    props: () => ({}),
  },
  chat: {
    letter: 'C',
    available: (panelId: string) => chatOf(panelId) !== null,
    props: (panelId: string) => chatOf(panelId),
  },
  mr: {
    letter: 'M',
    available: (panelId: string) => mrOf(panelId) !== null,
    props: (panelId: string) => {
      const mr = mrOf(panelId);
      return mr ? { mr } : null;
    },
  },
  diff: {
    letter: 'D',
    available: (panelId: string) => {
      const worktree = worktreeOf(panelId);
      return worktree !== null && hasChanges(worktree.path);
    },
    props: (panelId: string) => {
      const path = worktreeOf(panelId)?.path;
      return path ? { path, status: statusOf(panelId), session: useBerth()?.sessions.value.get(panelId)?.ref } : null;
    },
  },
};

export type SessionView = keyof typeof SESSION_VIEWS;
export type SessionViewSlots = { [V in SessionView]: (props: NonNullable<ReturnType<(typeof SESSION_VIEWS)[V]['props']>>) => unknown };
type Intersect<U> = (U extends unknown ? (value: U) => void : never) extends (value: infer I) => void ? I : never;
export type SessionViewBinding = Intersect<NonNullable<ReturnType<(typeof SESSION_VIEWS)[SessionView]['props']>>>;
export type SessionViewMode = { view: SessionView; letter: string; action: string };

export const sessionViewModes: readonly SessionViewMode[] = (Object.keys(SESSION_VIEWS) as SessionView[]).map((view) => ({ view, letter: SESSION_VIEWS[view].letter, action: `session.view.${view}` }));

const chosen = shallowRef<ReadonlyMap<string, SessionView>>(new Map());

export function isAvailable(panelId: string, view: SessionView): boolean {
  return SESSION_VIEWS[view].available(panelId);
}

export function viewProps(panelId: string, view: SessionView): Record<string, unknown> | null {
  return SESSION_VIEWS[view].props(panelId);
}

function homeView(panelId: string): SessionView {
  return acpAgentOf(panelId) === null ? 'terminal' : 'chat';
}

export function viewOf(panelId: string): SessionView {
  const home = homeView(panelId);
  const view = chosen.value.get(panelId) ?? home;
  return isAvailable(panelId, view) ? view : home;
}

export function showView(panelId: string, view: SessionView): void {
  if (!isAvailable(panelId, view)) return;
  const next = new Map(chosen.value);
  if (view === homeView(panelId) || viewOf(panelId) === view) next.delete(panelId);
  else next.set(panelId, view);
  chosen.value = next;
}

export function showViewInActivePanel(view: SessionView): void {
  const panelId = useDock().activeKey.value;
  if (panelId) showView(panelId, view);
}
