import { computed, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue';
import { worktreeGitState, type GitState } from '../../../shared/api/git';
import { nextStep } from './nextStep';

export const createRequested = shallowRef<{ path: string; seq: number } | null>(null);
let seq = 0;

export function requestCreate(path: string) {
  createRequested.value = { path, seq: ++seq };
}

export function useCanCreate(path: MaybeRefOrGetter<string>, refreshKey?: MaybeRefOrGetter<unknown>) {
  const state = shallowRef<GitState | null>(null);
  let run = 0;

  async function load() {
    const mine = ++run;
    try {
      const next = await worktreeGitState(toValue(path));
      if (mine === run) state.value = next;
    } catch {
      if (mine === run) state.value = null;
    }
  }

  watch(() => [toValue(path), toValue(refreshKey)] as const, () => void load(), { immediate: true });

  return computed(() => {
    const step = state.value ? nextStep(state.value, false) : null;
    return step?.kind === 'run' && step.create;
  });
}
