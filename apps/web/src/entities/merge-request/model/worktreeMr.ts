import { shallowRef, watch } from 'vue';
import { forgeChangeRequest, forgeForWorktree, onConnectorRecovered, useConnectors, type ChangeRequest, type ChangeRequestSummary } from '../../../shared/api/connectors/api';
import { Lru } from '../../../shared/lib/lru';

const TTL_MS = 60_000;
const RETRY_MS = 15_000;
const RETRY_CAP_MS = 120_000;
const RETRIES = 8;
const LIMIT = 200;
const SEPARATOR = '\0';

type Request = { at: number; ttl: number; done: boolean; value: Promise<ChangeRequestSummary | null> };

const requests = new Lru<string, Request>(LIMIT);
const known = shallowRef<ReadonlyMap<string, ChangeRequestSummary | null>>(new Map());
const failures = shallowRef<ReadonlySet<string>>(new Set());
const retryTimers = new Map<string, ReturnType<typeof setTimeout>>();
const retryCounts = new Map<string, number>();
let watchingConnectors = false;

const keyOf = (path: string, branch: string) => `${path}${SEPARATOR}${branch}`;

export const knownMr = (path: string, branch: string): ChangeRequestSummary | null => known.value.get(keyOf(path, branch)) ?? null;

export const mrLookupFailed = (path: string, branch: string): boolean => failures.value.has(keyOf(path, branch));

export const MR_RETRY_MS = RETRY_MS;

export function rememberMr(path: string, branch: string, summary: ChangeRequestSummary | null) {
  remember(keyOf(path, branch), summary);
}

const sameSummary = (a: ChangeRequestSummary | null, b: ChangeRequestSummary | null) => JSON.stringify(a) === JSON.stringify(b);

function summaryOf(request: ChangeRequest): ChangeRequestSummary {
  return {
    connector: request.connector,
    project: request.project,
    number: request.number,
    title: request.title,
    url: request.url,
    state: request.state,
    draft: request.draft,
    author: request.author,
    source_branch: request.source_branch,
    target_branch: request.target_branch,
    updated_at: request.updated_at,
    pipeline: request.pipeline,
    unresolved_threads: request.unresolved_threads,
    approval: request.approval,
  };
}

function remember(key: string, summary: ChangeRequestSummary | null) {
  const current = known.value;
  if (current.has(key) && sameSummary(current.get(key) ?? null, summary)) return;
  const next = new Map(current);
  next.delete(key);
  next.set(key, summary);
  for (const oldest of next.keys()) {
    if (next.size <= LIMIT) break;
    next.delete(oldest);
  }
  known.value = next;
}

export function updateKnownMr(request: ChangeRequest) {
  const fresh = summaryOf(request);
  let next: Map<string, ChangeRequestSummary | null> | undefined;
  for (const [key, value] of known.value) {
    if (!value || value.connector !== fresh.connector || value.project !== fresh.project || value.number !== fresh.number || sameSummary(value, fresh)) continue;
    next ??= new Map(known.value);
    next.set(key, fresh);
  }
  if (next) known.value = next;
}

function markFailed(key: string, failed: boolean) {
  if (failures.value.has(key) === failed) return;
  const next = new Set(failures.value);
  if (failed) next.add(key);
  else next.delete(key);
  failures.value = next;
}

function scheduleRetry(key: string) {
  clearTimeout(retryTimers.get(key));
  const count = retryCounts.get(key) ?? 0;
  if (count >= RETRIES) return;
  retryCounts.set(key, count + 1);
  retryTimers.set(
    key,
    setTimeout(() => {
      retryTimers.delete(key);
      relookup(key);
    }, Math.min(RETRY_MS * 2 ** count, RETRY_CAP_MS)),
  );
}

function relookup(key: string) {
  const [path, branch] = key.split(SEPARATOR);
  if (path !== undefined && branch !== undefined) void lookupMr(path, branch, true);
}

function settled(key: string) {
  clearTimeout(retryTimers.get(key));
  retryTimers.delete(key);
  retryCounts.delete(key);
  markFailed(key, false);
}

async function fetchMr(path: string, branch: string, previous: ChangeRequestSummary | null): Promise<ChangeRequestSummary | null> {
  const found = await forgeForWorktree(path, branch);
  if (found || !previous) return found;
  if (previous.state !== 'open') return previous;
  return summaryOf(await forgeChangeRequest({ connector: previous.connector, project: previous.project, number: previous.number }));
}

function retryAfterConnectorChange(includeEmpty: boolean) {
  const keys = new Set(failures.value);
  if (includeEmpty) for (const [key, value] of known.value) if (!value) keys.add(key);
  for (const key of keys) {
    if (requests.peek(key)?.done === false) continue;
    relookup(key);
  }
}

function watchConnectors() {
  if (watchingConnectors) return;
  watchingConnectors = true;
  const { connectors } = useConnectors();
  watch(
    () =>
      connectors.value
        .filter((connector) => connector.kind === 'github' || connector.kind === 'gitlab')
        .map((connector) => connector.id)
        .sort()
        .join(','),
    () => retryAfterConnectorChange(true),
  );
  onConnectorRecovered(() => retryAfterConnectorChange(false));
}

export function lookupMr(path: string, branch: string, force = false): Promise<ChangeRequestSummary | null> {
  watchConnectors();
  const key = keyOf(path, branch);
  const hit = requests.get(key);
  if (!force && hit && Date.now() - hit.at < hit.ttl) return hit.value;
  const request: Request = {
    at: Date.now(),
    ttl: TTL_MS,
    done: false,
    value: fetchMr(path, branch, known.value.get(key) ?? null).then(
      (summary) => {
        request.done = true;
        if (requests.peek(key) === request) {
          remember(key, summary);
          settled(key);
        }
        return summary;
      },
      () => {
        request.done = true;
        request.ttl = RETRY_MS;
        if (requests.peek(key) === request) {
          markFailed(key, true);
          scheduleRetry(key);
        }
        return known.value.get(key) ?? null;
      },
    ),
  };
  requests.set(key, request);
  return request.value;
}
