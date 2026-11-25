import { computed, onScopeDispose, shallowRef, watch, type Ref } from 'vue';
import { listWorktreeFiles } from '../../../entities/session';

const MENTION = /(?:^|\s)@([^\s@]*)$/;
const LIMIT = 20;
const DEBOUNCE_MS = 150;

export function useMention(draft: Ref<string>, worktree: () => string | null) {
  const query = computed(() => (worktree() ? (MENTION.exec(draft.value)?.[1] ?? null) : null));
  const files = shallowRef<readonly string[]>([]);
  let run = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  watch(query, (next) => {
    const mine = ++run;
    clearTimeout(timer);
    const id = worktree();
    if (next === null || !id) {
      files.value = [];
      return;
    }
    timer = setTimeout(async () => {
      const found = await listWorktreeFiles(id, next, LIMIT).catch(() => []);
      if (mine === run) files.value = found;
    }, DEBOUNCE_MS);
  });

  onScopeDispose(() => clearTimeout(timer));

  return {
    query,
    files,
    strip: () => draft.value.replace(/@[^\s@]*$/, ''),
  };
}
