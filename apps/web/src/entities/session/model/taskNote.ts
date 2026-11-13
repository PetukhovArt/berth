import { computed, shallowRef, toValue, watch, type ComputedRef, type MaybeRefOrGetter } from 'vue';
import { acknowledged, BerthCallError, call, useBerth } from './berth';

const loaded = shallowRef<ReadonlyMap<string, string>>(new Map());

async function loadTaskNote(worktree: string): Promise<void> {
  const reply = await call({ GetTaskNote: { worktree } });
  if (!('TaskNote' in reply)) throw new BerthCallError(Object.keys(reply).join(''));
  loaded.value = new Map(loaded.value).set(worktree, reply.TaskNote.text);
}

export function useTaskNote(worktree: MaybeRefOrGetter<string | undefined>): ComputedRef<string> {
  const berth = useBerth();
  watch(
    () => toValue(worktree),
    (id) => {
      if (id) void loadTaskNote(id).catch(() => undefined);
    },
    { immediate: true },
  );
  return computed(() => {
    const id = toValue(worktree);
    return id ? (berth?.taskNotes.value.get(id) ?? loaded.value.get(id) ?? '') : '';
  });
}

export async function saveTaskNote(worktree: string, text: string): Promise<void> {
  await acknowledged({ SetTaskNote: { worktree, text } });
}
