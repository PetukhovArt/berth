import { invoke } from '../../bridge';
import type { ChangeRequestSummary } from './connectors/api';

export type GitState = {
  branch: string | null;
  base: string | null;
  dirty: boolean;
  upstream: boolean;
  ahead: number;
  behind: number;
  ahead_of_base: number;
};

export type GitStepName = 'commit' | 'push' | 'pull';

export const worktreeGitState = (path: string) => invoke<GitState>('worktree_git_state', { path });
export const worktreeGitStep = (path: string, step: GitStepName, message?: string) => invoke<void>('worktree_git_step', { path, step, message: message ?? null });
export const forgeCreateChangeRequest = (path: string, branch: string, target: string, title: string, description: string) =>
  invoke<ChangeRequestSummary>('forge_create_change_request', { path, branch, target, title, description });
