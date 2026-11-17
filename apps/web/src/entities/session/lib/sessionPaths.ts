import type { SessionNode, SessionStatus, useBerth } from '../model/berth';
import type { SessionState } from '../model/sessionState';
import { urgencyOf } from './attention';

type Store = NonNullable<ReturnType<typeof useBerth>>;
export type WorkspaceNode = Store['tree']['value'][number];

export type SessionPath = {
  workspace: { id: string; name: string };
  project: { id: string; name: string };
  worktree: { id: string; branch: string };
  session: SessionNode;
};

export function sessionPaths(tree: readonly WorkspaceNode[]): Map<string, SessionPath> {
  const paths = new Map<string, SessionPath>();
  for (const workspace of tree)
    for (const project of workspace.projects)
      for (const worktree of project.worktrees)
        for (const session of worktree.sessions)
          paths.set(session.key, {
            workspace: { id: workspace.id, name: workspace.name },
            project: { id: project.id, name: project.name },
            worktree: { id: worktree.id, branch: worktree.branch },
            session,
          });
  return paths;
}

export type Rollup = { status: SessionStatus | undefined; unseen: boolean; running: number; feedback: number; total: number };

export function rollupOf(sessions: readonly Pick<SessionState, 'phase' | 'status' | 'unseen' | 'alive'>[]): Rollup {
  let status: SessionStatus | undefined;
  let top = -Infinity;
  let unseen = false;
  let running = 0;
  let feedback = 0;
  for (const session of sessions) {
    const urgency = urgencyOf(session);
    if (urgency > top) {
      top = urgency;
      status = session.status;
    }
    if (session.unseen) unseen = true;
    if (session.status === 'running') running++;
    if (session.status === 'needs_feedback') feedback++;
  }
  return { status, unseen, running, feedback, total: sessions.length };
}
