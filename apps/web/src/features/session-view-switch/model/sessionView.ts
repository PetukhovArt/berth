import { shallowRef } from 'vue';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { hasChanges } from './worktreeChanged';
import { knownMr } from '../../../entities/merge-request';
import { useDock } from '../../../shared/model/dock';
import { useSessionDock, useBerth, externalDot, sessionDot, useExternalSessions } from '../../../entities/session';

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

const SESSION_VIEWS = {
  terminal: {
    letter: 'T',
    available: () => true,
    props: () => ({}),
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

export function viewOf(panelId: string): SessionView {
  const view = chosen.value.get(panelId) ?? 'terminal';
  return isAvailable(panelId, view) ? view : 'terminal';
}

export function showView(panelId: string, view: SessionView): void {
  if (!isAvailable(panelId, view)) return;
  const next = new Map(chosen.value);
  if (view === 'terminal' || viewOf(panelId) === view) next.delete(panelId);
  else next.set(panelId, view);
  chosen.value = next;
}

export function showViewInActivePanel(view: SessionView): void {
  const panelId = useDock().activeKey.value;
  if (panelId) showView(panelId, view);
}
