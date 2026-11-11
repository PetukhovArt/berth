import { shallowRef } from 'vue';
import { worktreeChanges } from '../../../shared/api/fs';
import { Lru } from '../../../shared/lib/lru';

const TTL_MS = 5_000;
const LIMIT = 200;
const requests = new Lru<string, number>(LIMIT);
const known = shallowRef<ReadonlyMap<string, boolean>>(new Map());

export const hasChanges = (path: string): boolean => known.value.get(path) ?? false;

export function lookupChanges(path: string): void {
  const at = Date.now();
  if (at - (requests.get(path) ?? 0) < TTL_MS) return;
  requests.set(path, at);
  void worktreeChanges(path).then(
    (files) => {
      if (requests.peek(path) !== at || hasChanges(path) === files.length > 0) return;
      const next = new Map(known.value);
      next.delete(path);
      next.set(path, files.length > 0);
      for (const oldest of next.keys()) {
        if (next.size <= LIMIT) break;
        next.delete(oldest);
      }
      known.value = next;
    },
    () => undefined,
  );
}
