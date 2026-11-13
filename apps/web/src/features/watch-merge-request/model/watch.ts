import { shallowRef } from 'vue';
import { isDesktop } from '../../../bridge';
import { itemId, numberLabel } from '../../../entities/merge-request';
import { enqueuePrompt, sessionKey, submitPrompt, useBerth } from '../../../entities/session';
import { forgeChangeRequest, forgeDiscussions, forgePipeline, type ChangeRequest, type ChangeRequestRef, type Discussion, type Pipeline } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { createScope } from '../../../shared/lib/scope';

const STORE_KEY = 'berth.watchedMergeRequests';
const POLL_MS = 60_000;
const LIMIT = 50;
const SNIPPET_LIMIT = 200;

type Seen = { pipeline: string | null; threads: Set<string>; conflicts: boolean };
type Wake = { kind: 'pipeline' | 'thread' | 'conflicts'; prompt: string };

const refOf = (value: ChangeRequestRef): ChangeRequestRef => ({ connector: value.connector, project: value.project, number: value.number });

function load(): ChangeRequestRef[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]');
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is ChangeRequestRef => !!item && typeof item.connector === 'string' && typeof item.project === 'string' && typeof item.number === 'number').slice(-LIMIT);
  } catch {
    return [];
  }
}

const watched = shallowRef<readonly ChangeRequestRef[]>(load());
const wakes = shallowRef<ReadonlyMap<string, string>>(new Map());
const seen = new Map<string, Seen>();
let kick: (() => void) | null = null;

function persist(next: readonly ChangeRequestRef[]) {
  watched.value = next;
  localStorage.setItem(STORE_KEY, JSON.stringify(next));
}

export const isWatching = (target: ChangeRequestRef): boolean => watched.value.some((entry) => itemId(entry) === itemId(target));

export const lastWake = (target: ChangeRequestRef): string | null => wakes.value.get(itemId(target)) ?? null;

export function stopWatching(target: ChangeRequestRef) {
  const id = itemId(target);
  seen.delete(id);
  const next = new Map(wakes.value);
  next.delete(id);
  wakes.value = next;
  persist(watched.value.filter((entry) => itemId(entry) !== id));
}

export function toggleWatch(target: ChangeRequestRef) {
  if (isWatching(target)) {
    stopWatching(target);
    return;
  }
  persist([...watched.value, refOf(target)].slice(-LIMIT));
  kick?.();
}

const open = (discussion: Discussion) => !discussion.resolved && discussion.notes.some((note) => !note.system);

function failedJobs(pipeline: Pipeline): string {
  const names = pipeline.jobs.filter((job) => job.status === 'failed').map((job) => job.name);
  return names.length ? ` in ${names.join(', ')}` : '';
}

function snippet(text: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > SNIPPET_LIMIT ? `${flat.slice(0, SNIPPET_LIMIT).trimEnd()}…` : flat;
}

function threadPrompt(label: string, discussion: Discussion): string {
  const note = discussion.notes.find((entry) => !entry.system);
  const line = discussion.position?.new_line ? `:${discussion.position.new_line}` : '';
  const place = discussion.position ? ` on ${discussion.position.path}${line}` : '';
  const author = note ? note.author.name || note.author.username : 'Someone';
  return `${author} opened a thread${place} in ${label}: "${snippet(note?.body ?? '')}". Answer it or fix it and push.`;
}

function collect(label: string, request: ChangeRequest, pipeline: Pipeline | null, discussions: Discussion[], state: Seen | undefined): { found: Wake[]; state: Seen } {
  const threads = discussions.filter(open);
  const next: Seen = {
    pipeline: pipeline?.status === 'failed' ? pipeline.id : null,
    threads: new Set(threads.map((discussion) => discussion.id)),
    conflicts: request.mergeable === false,
  };
  if (!state) return { found: [], state: next };
  const found: Wake[] = [];
  if (pipeline && next.pipeline && next.pipeline !== state.pipeline) {
    found.push({ kind: 'pipeline', prompt: `Pipeline #${pipeline.id} of ${label} failed${failedJobs(pipeline)}. Read its log, fix the cause and push.` });
  }
  for (const discussion of threads) {
    if (!state.threads.has(discussion.id)) found.push({ kind: 'thread', prompt: threadPrompt(label, discussion) });
  }
  if (next.conflicts && !state.conflicts) {
    found.push({ kind: 'conflicts', prompt: `${label} now conflicts with ${request.target_branch}. Rebase on ${request.target_branch}, resolve the conflicts and push.` });
  }
  return { found, state: next };
}

export type Announce = (session: string, text: string) => void;

export function initWatchMergeRequest(announce: Announce): () => void {
  const scope = createScope();
  const berth = useBerth();
  if (!isDesktop || !berth) return () => scope.dispose();
  let running = false;

  const agentOf = (branch: string) => {
    const { agents, worktrees } = berth.entities.value;
    const ids = new Set([...worktrees.values()].filter((worktree) => worktree.branch === branch).map((worktree) => worktree.id));
    return [...agents.values()].filter((agent) => ids.has(agent.worktree_id) && !agent.archived && agent.alive).sort((a, b) => b.status_changed_at - a.status_changed_at)[0];
  };

  async function check(target: ChangeRequestRef) {
    const id = itemId(target);
    const [request, run, threads] = await Promise.allSettled([forgeChangeRequest(target), forgePipeline(target), forgeDiscussions(target)]);
    if (request.status === 'rejected' || !isWatching(target)) return;
    if (request.value.state !== 'open') {
      stopWatching(target);
      return;
    }
    if (run.status === 'rejected' || threads.status === 'rejected') return;
    const label = numberLabel(request.value);
    const { found, state } = collect(label, request.value, run.value, threads.value, seen.get(id));
    if (found.length) {
      const agent = agentOf(request.value.source_branch);
      if (!agent) return;
      const prompt = found.map((wake) => wake.prompt).join(' ');
      try {
        if (agent.status === 'running') enqueuePrompt(agent.id, prompt);
        else await submitPrompt({ Agent: agent.id }, prompt);
      } catch {
        return;
      }
      const next = new Map(wakes.value);
      next.set(id, prompt);
      wakes.value = next;
      announce(sessionKey({ Agent: agent.id }), t(`attention.watch.toast.${found[0]?.kind ?? 'pipeline'}`, { name: agent.name, label }));
    }
    seen.set(id, state);
  }

  async function sweep() {
    if (running || berth?.connection.value.state !== 'connected') return;
    running = true;
    try {
      for (const target of watched.value) {
        if (scope.disposed) return;
        await check(target).catch(() => undefined);
      }
    } finally {
      running = false;
    }
  }

  kick = () => void sweep();
  const timer = setInterval(() => void sweep(), POLL_MS);
  scope.add(() => {
    clearInterval(timer);
    kick = null;
  });
  void sweep();
  return () => scope.dispose();
}
