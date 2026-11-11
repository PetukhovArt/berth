import { shallowRef } from 'vue';
import { forgeForWorktree, type ChangeRequestSummary } from '../../../shared/api/connectors/api';
import { Lru } from '../../../shared/lib/lru';

const TTL_MS = 60_000;
const RETRY_MS = 15_000;
const LIMIT = 200;

type Request = { at: number; ttl: number; failed: boolean; value: Promise<ChangeRequestSummary | null> };

const requests = new Lru<string, Request>(LIMIT);
const known = shallowRef<ReadonlyMap<string, ChangeRequestSummary | null>>(new Map());

const keyOf = (path: string, branch: string) => `${path}\0${branch}`;

export const knownMr = (path: string, branch: string): ChangeRequestSummary | null => known.value.get(keyOf(path, branch)) ?? null;

export const mrLookupFailed = (path: string, branch: string): boolean => requests.peek(keyOf(path, branch))?.failed ?? false;

export const MR_RETRY_MS = RETRY_MS;

function remember(key: string, summary: ChangeRequestSummary | null) {
  const current = known.value;
  if (current.has(key) && JSON.stringify(current.get(key)) === JSON.stringify(summary)) return;
  const next = new Map(current);
  next.delete(key);
  next.set(key, summary);
  for (const oldest of next.keys()) {
    if (next.size <= LIMIT) break;
    next.delete(oldest);
  }
  known.value = next;
}

export function lookupMr(path: string, branch: string): Promise<ChangeRequestSummary | null> {
  const key = keyOf(path, branch);
  const hit = requests.get(key);
  if (hit && Date.now() - hit.at < hit.ttl) return hit.value;
  const request: Request = {
    at: Date.now(),
    ttl: TTL_MS,
    failed: false,
    value: forgeForWorktree(path, branch).then(
      (summary) => {
        if (requests.peek(key) === request) remember(key, summary);
        return summary;
      },
      () => {
        request.failed = true;
        request.ttl = RETRY_MS;
        return known.value.get(key) ?? null;
      },
    ),
  };
  requests.set(key, request);
  return request.value;
}
