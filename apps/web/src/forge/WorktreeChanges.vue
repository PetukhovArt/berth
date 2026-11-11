<script setup lang="ts">
import { shallowRef, watch } from 'vue';
import { RefreshCw } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import type { SessionStatus } from '../entities/session/model/berth';
import { describeError, type DiffFile, type Discussion } from '../shared/api/connectors/api';
import { worktreeChanges } from '../shared/api/fs';
import { t } from '../shared/lib/i18n/i18n';
import DiffView from './DiffView.vue';

const props = defineProps<{ path: string; status?: SessionStatus }>();

type Changes = { kind: 'loading' } | { kind: 'failed'; message: string } | { kind: 'ready'; files: DiffFile[]; signature: string };

const NO_DISCUSSIONS: Discussion[] = [];

const changes = shallowRef<Changes>({ kind: 'loading' });
const refreshing = shallowRef(false);

let generation = 0;

async function load() {
  const ticket = ++generation;
  refreshing.value = true;
  try {
    const files = await worktreeChanges(props.path);
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
    changes.value = { kind: 'loading' };
    void load();
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
    <p v-if="changes.kind === 'loading'" class="state" data-testid="worktree-changes-loading">{{ t('agents.changes.loading') }}</p>
    <div v-else-if="changes.kind === 'failed'" class="state" role="alert" data-testid="worktree-changes-error">
      <p class="failure">{{ t('agents.changes.failed', { error: changes.message }) }}</p>
      <VButton size="small" severity="secondary" :label="t('agents.changes.retry')" :loading="refreshing" data-testid="worktree-changes-retry" @click="load" />
    </div>
    <div v-else-if="!changes.files.length" class="state" data-testid="worktree-changes-empty">
      <p>{{ t('agents.changes.empty') }}</p>
      <VButton size="small" severity="secondary" :label="t('agents.changes.refresh')" :loading="refreshing" data-testid="worktree-changes-refresh" @click="load" />
    </div>
    <DiffView v-else :files="changes.files" :discussions="NO_DISCUSSIONS" :root="path">
      <template #actions>
        <button class="refresh" :disabled="refreshing" :aria-busy="refreshing" :title="t('agents.changes.refresh')" :aria-label="t('agents.changes.refresh')" data-testid="worktree-changes-refresh" @click="load">
          <RefreshCw :size="14" :class="{ spin: refreshing }" />
        </button>
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

.state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-3);
  height: 100%;
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
</style>
