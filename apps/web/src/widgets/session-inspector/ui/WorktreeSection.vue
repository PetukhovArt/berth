<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { ArrowDown, ArrowUp, Folder, GitBranch } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { worktreeGitState, type GitState } from '../../../shared/api/git';
import { worktreeChanges } from '../../../shared/api/fs';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import InspectorSection from './InspectorSection.vue';

const REFRESH_MS = 30_000;

const props = defineProps<{ path: string; branch: string; refreshKey?: unknown }>();

type Changes = { files: number; additions: number; deletions: number };

const state = shallowRef<GitState | null>(null);
const changes = shallowRef<Changes | null>(null);
const scope = useScope();
let ticket = 0;

async function refresh() {
  const id = ++ticket;
  const path = props.path;
  const [git, files] = await Promise.allSettled([scope.load(worktreeGitState(path)), scope.load(worktreeChanges(path))]);
  if (id !== ticket) return;
  state.value = git.status === 'fulfilled' ? git.value : null;
  changes.value =
    files.status === 'fulfilled'
      ? { files: files.value.length, additions: files.value.reduce((sum, file) => sum + file.additions, 0), deletions: files.value.reduce((sum, file) => sum + file.deletions, 0) }
      : null;
}

const base = computed(() => (state.value?.base && state.value.base !== props.branch ? state.value.base : null));
const ahead = computed(() => state.value?.ahead_of_base ?? 0);
const behind = computed(() => state.value?.behind ?? 0);
const unpublished = computed(() => !!base.value && !!state.value && state.value.remote && !state.value.upstream && ahead.value > 0);

watch(
  () => [props.path, props.refreshKey] as const,
  ([path], previous) => {
    if (previous && previous[0] !== path) {
      state.value = null;
      changes.value = null;
    }
    void refresh();
  },
  { immediate: true },
);
visibleInterval(scope, REFRESH_MS, () => void refresh());
</script>

<template>
  <InspectorSection :title="t('inspector.worktree.title')" testid="inspector-worktree">
    <div class="row">
      <GitBranch class="icon" :size="13" />
      <span class="grow branch" :title="branch" data-testid="inspector-worktree-branch">{{ branch }}</span>
      <span v-if="base" class="end" data-testid="inspector-worktree-base">{{ t('inspector.worktree.from', { base }) }}</span>
    </div>
    <div v-if="changes && base" class="row">
      <span class="grow" data-testid="inspector-worktree-files">{{ changes.files ? t('inspector.worktree.files', { count: changes.files }) : t('inspector.worktree.clean', { base }) }}</span>
      <span v-if="changes.files" class="end diffstat">
        <span class="added">+{{ changes.additions }}</span>
        <span class="removed">−{{ changes.deletions }}</span>
      </span>
    </div>
    <div v-if="base && (ahead || behind)" class="row" data-testid="inspector-worktree-ahead-behind">
      <span class="grow counts">
        <span v-if="ahead" class="count"><ArrowUp :size="12" />{{ t('inspector.worktree.ahead', { count: ahead }) }}</span>
        <span v-if="behind" class="count"><ArrowDown :size="12" />{{ t('inspector.worktree.behind', { count: behind }) }}</span>
      </span>
      <span v-if="unpublished" class="end">{{ t('inspector.worktree.unpublished') }}</span>
    </div>
    <div class="row">
      <Folder class="icon" :size="13" />
      <span class="grow path" :title="path" :aria-label="`${t('inspector.worktree.path')}: ${path}`" data-testid="inspector-worktree-path"><bdi>{{ path }}</bdi></span>
    </div>
  </InspectorSection>
</template>

<style scoped>
.branch {
  font-family: var(--vr-font-mono);
  font-weight: 600;
}

.diffstat {
  display: inline-flex;
  gap: 6px;
}

.added {
  color: var(--vr-color-success);
}

.removed {
  color: var(--vr-color-danger);
}

.counts {
  display: inline-flex;
  gap: var(--vr-space-3);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.count {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-variant-numeric: tabular-nums;
}

.path {
  direction: rtl;
  text-align: left;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
