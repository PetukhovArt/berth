import { computed, shallowRef, watch } from 'vue';
import {
  describeError,
  forgeChangeRequest,
  forgeComment,
  forgeDiff,
  forgeDiscussions,
  forgePipeline,
  onConnectorRecovered,
  type ChangeRequest,
  type ChangeRequestRef,
  type DiffFile,
  type Discussion,
  type Pipeline,
} from '../../../shared/api/connectors/api';
import { itemId } from '../../../entities/merge-request';
import type { Scope } from '../../../shared/lib/scope';

type Tab = 'overview' | 'diff' | 'pipeline';

export function createChangeRequest(source: () => ChangeRequestRef, scope: Scope) {
  const cr = shallowRef<ChangeRequest | null>(null);
  const discussions = shallowRef<Discussion[]>([]);
  const pipeline = shallowRef<Pipeline | null>(null);
  const files = shallowRef<DiffFile[]>([]);
  const loading = shallowRef(false);
  const failure = shallowRef<string | null>(null);
  const sideFailure = shallowRef<string | null>(null);
  const diffLoading = shallowRef(false);
  const diffFailure = shallowRef<string | null>(null);
  const diffLoaded = shallowRef(false);
  const tab = shallowRef<Tab>('overview');
  const pipelineOpen = shallowRef(false);
  const commentDraft = shallowRef('');
  const commenting = shallowRef(false);
  const commentError = shallowRef<string | null>(null);

  const target = computed<ChangeRequestRef>(source);
  let generation = 0;

  async function loadDiff() {
    const ticket = generation;
    diffLoading.value = true;
    diffFailure.value = null;
    try {
      const result = await scope.load(forgeDiff(target.value));
      if (ticket === generation) {
        files.value = result;
        diffLoaded.value = true;
      }
    } catch (e) {
      if (ticket === generation) diffFailure.value = describeError(e);
    } finally {
      if (ticket === generation) diffLoading.value = false;
    }
  }

  async function load() {
    const ticket = ++generation;
    loading.value = true;
    failure.value = null;
    sideFailure.value = null;
    try {
      const [request, threads, run] = await scope.load(Promise.allSettled([forgeChangeRequest(target.value), forgeDiscussions(target.value), forgePipeline(target.value)]));
      if (ticket !== generation) return;
      if (request.status === 'rejected') {
        failure.value = describeError(request.reason);
        return;
      }
      cr.value = request.value;
      discussions.value = threads.status === 'fulfilled' ? threads.value : [];
      pipeline.value = run.status === 'fulfilled' ? run.value : null;
      const problems = [threads, run].filter((r): r is PromiseRejectedResult => r.status === 'rejected');
      sideFailure.value = problems.length ? problems.map((p) => describeError(p.reason)).join('; ') : null;
      if (tab.value === 'diff' || diffLoaded.value) await loadDiff();
    } finally {
      if (ticket === generation) loading.value = false;
    }
  }

  function selectTab(next: Tab) {
    tab.value = next;
    if (next === 'diff' && !diffLoaded.value && !diffLoading.value) void loadDiff();
  }

  function upsertDiscussion(updated: Discussion) {
    const list = discussions.value;
    const index = list.findIndex((d) => d.id === updated.id);
    discussions.value = index < 0 ? [...list, updated] : list.map((d, i) => (i === index ? updated : d));
  }

  async function sendComment() {
    const body = commentDraft.value.trim();
    if (!body || commenting.value) return;
    const key = itemId(target.value);
    commenting.value = true;
    commentError.value = null;
    try {
      const created = await scope.load(forgeComment(target.value, body));
      if (key !== itemId(target.value)) return;
      upsertDiscussion(created);
      commentDraft.value = '';
    } catch (e) {
      if (key === itemId(target.value)) commentError.value = describeError(e);
    } finally {
      commenting.value = false;
    }
  }

  scope.add(
    watch(
      () => [target.value.connector, target.value.project, target.value.number],
      () => {
        cr.value = null;
        discussions.value = [];
        pipeline.value = null;
        files.value = [];
        diffLoaded.value = false;
        diffFailure.value = null;
        pipelineOpen.value = false;
        commentDraft.value = '';
        commentError.value = null;
        tab.value = 'overview';
        void load();
      },
    ),
  );

  scope.add(
    onConnectorRecovered((id) => {
      if (id === target.value.connector && (failure.value !== null || sideFailure.value !== null)) void load();
    }),
  );

  return {
    target,
    cr,
    discussions,
    pipeline,
    files,
    loading,
    failure,
    sideFailure,
    diffLoading,
    diffFailure,
    diffLoaded,
    tab,
    pipelineOpen,
    commentDraft,
    commenting,
    commentError,
    generation: () => generation,
    load,
    loadDiff,
    selectTab,
    upsertDiscussion,
    sendComment,
  };
}

export type ChangeRequestStore = ReturnType<typeof createChangeRequest>;
