import { computed, shallowRef, watch, type Ref } from 'vue';
import { listWorktreeFiles } from '../../../entities/session';

const MENTION = /(?:^|\s)@([^\s@]*)$/;
const LIMIT = 20;

export function useMention(draft: Ref<string>, worktree: () => string | null) {
  const query = computed(() => (worktree() ? (MENTION.exec(draft.value)?.[1] ?? null) : null));
  const files = shallowRef<readonly string[]>([]);
  let run = 0;

  watch(query, async (next) => {
    const mine = ++run;
    const id = worktree();
    if (next === null || !id) {
      files.value = [];
      return;
    }
    const found = await listWorktreeFiles(id, next, LIMIT).catch(() => []);
    if (mine === run) files.value = found;
  });

  return {
    query,
    files,
    strip: () => draft.value.replace(/@[^\s@]*$/, ''),
  };
}
