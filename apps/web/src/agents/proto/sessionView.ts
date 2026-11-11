import { shallowRef } from 'vue';
import type { ChangeRequestRef } from '../../shared/api/connectors/api';
import { hasChanges } from '../../forge/worktreeChanged';
import { knownMr } from '../../forge/worktreeMr';
import { useDock } from '../../shared/model/dock';
import { useBerth } from '../berth';
import { useExternalSessions } from '../external';

export type SessionView = 'terminal' | 'mr' | 'diff';

export type SessionWorktree = { path: string; branch: string };

const chosen = shallowRef<ReadonlyMap<string, SessionView>>(new Map());

export function worktreeOf(panelId: string): SessionWorktree | null {
  const worktreeId = useDock().paths.value.get(panelId)?.worktree.id;
  const worktree = worktreeId ? useBerth()?.entities.value.worktrees.get(worktreeId) : undefined;
  if (worktree) return { path: worktree.path, branch: worktree.branch };
  const conversation = useExternalSessions()?.anyByKey.value.get(panelId);
  return conversation?.branch ? { path: conversation.cwd, branch: conversation.branch } : null;
}

export function mrOf(panelId: string): ChangeRequestRef | null {
  const worktree = worktreeOf(panelId);
  const summary = worktree ? knownMr(worktree.path, worktree.branch) : null;
  return summary ? { connector: summary.connector, project: summary.project, number: summary.number } : null;
}

export function isAvailable(panelId: string, view: SessionView): boolean {
  switch (view) {
    case 'terminal':
      return true;
    case 'mr':
      return mrOf(panelId) !== null;
    case 'diff': {
      const worktree = worktreeOf(panelId);
      return worktree !== null && hasChanges(worktree.path);
    }
    default: {
      const unknown: never = view;
      return unknown;
    }
  }
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
