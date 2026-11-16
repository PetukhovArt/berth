<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from 'vue';
import { RefreshCw } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { useBerth, type SessionRef, type SessionStatus } from '../../../entities/session';
import { knownMr } from '../../../entities/merge-request';
import { GitStepButton } from '../../../features/git-step';
import { clearChangeFocus, focusedChange, quoteToPrompt } from '../../../features/session-view-switch';
import { worktreeRelative } from '../../../shared/lib/worktreePath';
import { describeError, type DiffFile, type Discussion } from '../../../shared/api/connectors/api';
import { worktreeChanges, type ChangesScope } from '../../../shared/api/fs';
import { t } from '../../../shared/lib/i18n/i18n';
import { DiffView } from '../../../features/diff-review';
import ChangesScopePicker from './ChangesScopePicker.vue';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ path: string; status?: SessionStatus; session?: SessionRef }>();

type Changes = { kind: 'loading' } | { kind: 'failed'; message: string } | { kind: 'ready'; files: DiffFile[]; signature: string };

const NO_DISCUSSIONS: Discussion[] = [];

const changes = shallowRef<Changes>({ kind: 'loading' });
const refreshing = shallowRef(false);
const scope = shallowRef<ChangesScope>('uncommitted');

let generation = 0;

const berth = useBerth();
const normal = (path: string) => path.replace(/\\/g, '/').replace(/\/+$/, '');
const worktree = computed(() => [...(berth?.entities.value.worktrees.values() ?? [])].find((entry) => normal(entry.path) === normal(props.path)));
const request = computed(() => (worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null));

function quote(text: string) {
  if (props.session) quoteToPrompt(props.session, text);
}

async function load() {
  const ticket = ++generation;
  refreshing.value = true;
  try {
    const files = await worktreeChanges(props.path, scope.value);
    if (ticket !== generation) return;
    const signature = JSON.stringify(files);
    const current = changes.value;
    if (current.kind !== 'ready' || current.signature !== signature) changes.value = { kind: 'ready', files, signature };
  } catch (e) {
    if (ticket === generation) changes.value = { kind: 'failed', message: describeError(e) };
  } finally {
    if (ticket === generation) refreshing.value = false;
  }
}

watch(
  () => props.path,
  () => {
    scope.value = 'uncommitted';
    changes.value = { kind: 'loading' };
    void load();
  },
  { immediate: true },
);

watch(scope, () => {
  changes.value = { kind: 'loading' };
  void load();
});

const diff = useTemplateRef<{ focusFile: (path: string) => boolean }>('diff');

watch(
  [() => focusedChange(props.path), changes],
  async ([file, current]) => {
    if (!file || current.kind !== 'ready') return;
    await nextTick();
    const wanted = worktreeRelative(file, props.path);
    const match = current.files.find((entry) => entry.new_path === wanted || entry.old_path === wanted) ?? current.files.find((entry) => wanted.endsWith(`/${entry.new_path}`) || entry.new_path.endsWith(`/${wanted}`));
    if (match) diff.value?.focusFile(match.new_path);
    clearChangeFocus(props.path);
  },
  { immediate: true },
);

watch(
  () => props.status,
  (status) => {
    if (status === 'finished' || status === 'needs_feedback') void load();
  },
);
</script>

<template>
  <div class="changes" data-testid="worktree-changes">
    <div v-if="changes.kind !== 'ready' || !changes.files.length" class="bar">
      <ChangesScopePicker v-model="scope" :path="path" />
    </div>
    <p v-if="changes.kind === 'loading'" class="state" data-testid="worktree-changes-loading">{{ t('agents.changes.loading') }}</p>
    <div v-else-if="changes.kind === 'failed'" class="state" role="alert" data-testid="worktree-changes-error">
      <p class="failure">{{ t('agents.changes.failed', { error: changes.message }) }}</p>
      <VButton size="small" severity="secondary" :label="t('agents.changes.retry')" :loading="refreshing" data-testid="worktree-changes-retry" @click="load" />
    </div>
    <div v-else-if="!changes.files.length" class="state" data-testid="worktree-changes-empty">
      <p>{{ scope === 'uncommitted' ? t('agents.changes.emptyUncommitted') : t('agents.changes.empty') }}</p>
      <VButton size="small" severity="secondary" :label="t('agents.changes.refresh')" :loading="refreshing" data-testid="worktree-changes-refresh" @click="load" />
    </div>
    <DiffView v-else ref="diff" :files="changes.files" :discussions="NO_DISCUSSIONS" :root="path" :quotable="!!session" @quote="quote">
      <template #scope>
        <ChangesScopePicker v-model="scope" :path="path" />
      </template>
      <template #actions>
        <UiButton class="refresh" :disabled="refreshing" :aria-busy="refreshing" :title="t('agents.changes.refresh')" :aria-label="t('agents.changes.refresh')" data-testid="worktree-changes-refresh" @click="load">
          <RefreshCw :size="14" :class="{ spin: refreshing }" />
        </UiButton>
        <GitStepButton v-if="worktree?.branch" quiet testid="changes-git-step" :path="worktree.path" :branch="worktree.branch" :has-request="!!request" :refresh-key="status" />
      </template>
    </DiffView>
  </div>
</template>

<style scoped>
.changes {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.bar {
  display: flex;
  flex: none;
  align-items: center;
  padding: var(--vr-space-2) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-3);
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: var(--vr-space-6);
  color: var(--vr-color-text-muted);
  text-align: center;
}

.state p {
  margin: 0;
}

.failure {
  color: var(--vr-color-danger);
}

.refresh {
  display: inline-flex;
  align-items: center;
  padding: 2px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  background: none;
  border: 1px solid transparent;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.refresh:hover:not(:disabled) {
  color: var(--vr-color-text);
}

.refresh:disabled {
  cursor: default;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}

:root.vr-reduce-motion .spin {
  animation: none;
}
</style>
