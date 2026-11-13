<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import type { ChangeRequestRef, ChangeRequestSummary } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { numberLabel, pipelineLabel } from '../lib/format';
import { lookupMr, mrLookupFailed, MR_RETRY_MS } from '../model/worktreeMr';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ path: string; branch: string; summary?: ChangeRequestSummary }>();
const emit = defineEmits<{ open: [ref: ChangeRequestRef] }>();

const looked = shallowRef<ChangeRequestSummary | null>(null);
const summary = computed(() => props.summary ?? looked.value);

watch(
  () => [props.path, props.branch, props.summary] as const,
  async ([path, branch, known], _, onCleanup) => {
    looked.value = null;
    if (known || !path || !branch) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    onCleanup(() => {
      alive = false;
      clearTimeout(timer);
    });
    const run = async () => {
      const result = await lookupMr(path, branch);
      if (!alive) return;
      looked.value = result;
      if (mrLookupFailed(path, branch)) timer = setTimeout(() => void run(), MR_RETRY_MS);
    };
    await run();
  },
  { immediate: true },
);

const title = computed(() => {
  const s = summary.value;
  if (!s) return '';
  const parts = [`${numberLabel(s)} ${s.title}`, s.project, pipelineLabel(s.pipeline)];
  if (s.unresolved_threads) parts.push(t('forge.unresolvedThreads', { count: s.unresolved_threads }));
  return parts.join('\n');
});

function open() {
  const s = summary.value;
  if (s) emit('open', { connector: s.connector, project: s.project, number: s.number });
}
</script>

<template>
  <UiButton v-if="summary" class="badge" :title="title" :aria-label="title" data-testid="worktree-badge" @click.stop="open">
    <span class="pipe"><slot name="pipeline" :pipeline="summary.pipeline" /></span>
    <span class="num">{{ numberLabel(summary) }}</span>
  </UiButton>
</template>

<style scoped>
.badge {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  height: 18px;
  padding: 0 5px;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: 10.5px;
  line-height: 1;
  background: var(--vr-color-surface-muted);
  border: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.badge:hover {
  color: var(--vr-color-text);
}

.pipe {
  display: inline-flex;
  flex: none;
}

.pipe:empty {
  display: none;
}

.pipe :deep(.status-icon) {
  width: 10px;
  height: 10px;
}
</style>
