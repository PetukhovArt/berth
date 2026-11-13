import type { GitState } from '../../../shared/api/git';

export type NextStep = { kind: 'run'; commit: boolean; push: boolean; create: boolean } | { kind: 'pull' } | { kind: 'idle'; reason: 'upToDate' | 'defaultBranch' };

export function nextStep(state: GitState, hasRequest: boolean): NextStep {
  if (!state.branch || !state.base) return { kind: 'idle', reason: 'upToDate' };
  if (state.branch === state.base) return { kind: 'idle', reason: state.dirty ? 'defaultBranch' : 'upToDate' };
  if (state.behind > 0) return { kind: 'pull' };
  const commit = state.dirty;
  const unpublished = state.upstream ? state.ahead > 0 : state.ahead_of_base > 0;
  const push = commit || unpublished;
  const create = !hasRequest && (commit || state.ahead_of_base > 0);
  if (!push && !create) return { kind: 'idle', reason: 'upToDate' };
  return { kind: 'run', commit, push, create };
}

export function stepLabelKey(step: NextStep) {
  if (step.kind === 'pull') return 'gitStep.pull' as const;
  if (step.kind === 'idle') return 'gitStep.commitAndPush' as const;
  if (step.commit) return step.create ? 'gitStep.commitPushCreate' as const : 'gitStep.commitAndPush' as const;
  if (step.push) return step.create ? 'gitStep.pushCreate' as const : 'gitStep.push' as const;
  return 'gitStep.create' as const;
}
