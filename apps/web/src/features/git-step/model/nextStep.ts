import type { GitState } from '../../../shared/api/git';

type IdleReason = 'upToDate' | 'defaultBranch' | 'detached';

export type NextStep = { kind: 'run'; commit: boolean; push: boolean; create: boolean } | { kind: 'pull' } | { kind: 'idle'; reason: IdleReason };

export function nextStep(state: GitState, hasRequest: boolean): NextStep {
  if (!state.branch) return { kind: 'idle', reason: state.dirty ? 'detached' : 'upToDate' };
  if (state.branch === state.base) return { kind: 'idle', reason: state.dirty ? 'defaultBranch' : 'upToDate' };
  if (state.behind > 0) return { kind: 'pull' };
  const commit = state.dirty;
  const unpublished = state.upstream ? state.ahead > 0 : state.ahead_of_base > 0;
  const push = state.remote && (commit || unpublished);
  const create = state.remote && !!state.base && !hasRequest && (commit || state.ahead_of_base > 0);
  if (!commit && !push && !create) return { kind: 'idle', reason: 'upToDate' };
  return { kind: 'run', commit, push, create };
}

export function stepShortLabelKey(step: NextStep) {
  if (step.kind === 'run' && !step.commit && step.create) return step.push ? ('gitStep.pushCreateShort' as const) : ('gitStep.createShort' as const);
  if (step.kind === 'run' && step.commit && step.create) return 'gitStep.commitPushCreateShort' as const;
  if (step.kind === 'run' && step.commit && step.push) return 'gitStep.commitPushShort' as const;
  return stepLabelKey(step);
}

export function idleHintKey(reason: IdleReason) {
  if (reason === 'defaultBranch') return 'gitStep.defaultBranch' as const;
  if (reason === 'detached') return 'gitStep.detached' as const;
  return 'gitStep.upToDate' as const;
}

export function stepLabelKey(step: NextStep) {
  if (step.kind === 'pull') return 'gitStep.pull' as const;
  if (step.kind === 'idle') return step.reason === 'upToDate' ? ('gitStep.commit' as const) : ('gitStep.commitAndPush' as const);
  if (step.commit) {
    if (step.create) return 'gitStep.commitPushCreate' as const;
    return step.push ? 'gitStep.commitAndPush' as const : 'gitStep.commit' as const;
  }
  if (step.push) return step.create ? 'gitStep.pushCreate' as const : 'gitStep.push' as const;
  return 'gitStep.create' as const;
}
