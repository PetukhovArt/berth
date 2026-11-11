import { shallowRef } from 'vue';
import { worktreeChanges } from '../files/api';

const TTL_MS = 5_000;
const requests = new Map<string, number>();
const known = shallowRef<ReadonlyMap<string, boolean>>(new Map());

export const hasChanges = (path: string): boolean => known.value.get(path) ?? false;

export function lookupChanges(path: string): void {
  const at = Date.now();
  if (at - (requests.get(path) ?? 0) < TTL_MS) return;
  requests.set(path, at);
  void worktreeChanges(path).then(
    (files) => {
      if (requests.get(path) !== at || hasChanges(path) === files.length > 0) return;
      const next = new Map(known.value);
      next.set(path, files.length > 0);
      known.value = next;
    },
    () => undefined,
  );
}
