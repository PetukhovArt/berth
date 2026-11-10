import { shallowRef } from 'vue';
import { forgeForWorktree, type ChangeRequestSummary } from '../connectors/api';

const TTL_MS = 60_000;
const requests = new Map<string, { at: number; value: Promise<ChangeRequestSummary | null> }>();
const known = shallowRef<ReadonlyMap<string, ChangeRequestSummary | null>>(new Map());

const keyOf = (path: string, branch: string) => `${path}\0${branch}`;

export const knownMr = (path: string, branch: string): ChangeRequestSummary | null => known.value.get(keyOf(path, branch)) ?? null;

export function lookupMr(path: string, branch: string): Promise<ChangeRequestSummary | null> {
  const key = keyOf(path, branch);
  const hit = requests.get(key);
  if (hit && Date.now() - hit.at < TTL_MS) return hit.value;
  const value = forgeForWorktree(path, branch).catch(() => null);
  requests.set(key, { at: Date.now(), value });
  void value.then((summary) => {
    if (requests.get(key)?.value !== value) return;
    const next = new Map(known.value);
    next.set(key, summary);
    known.value = next;
  });
  return value;
}
