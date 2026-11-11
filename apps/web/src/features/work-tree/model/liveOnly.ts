import type { Sources } from './tree';

export function liveSources(sources: Sources): Sources {
  return {
    ...sources,
    workspaces: sources.workspaces.map((workspace) => ({
      ...workspace,
      projects: workspace.projects.map((project) => ({
        ...project,
        worktrees: project.worktrees.map((worktree) => ({ ...worktree, sessions: worktree.sessions.filter((session) => session.alive), archived: [] })),
      })),
    })),
    external: sources.external.filter((session) => session.running),
    archivedExternal: [],
  };
}
