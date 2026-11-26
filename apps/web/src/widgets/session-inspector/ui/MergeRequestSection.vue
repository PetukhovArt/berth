<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { ExternalLink, GitPullRequest, MessageSquare } from '../../../shared/ui/icons';
import { VButton, VTag, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import { openUrl, useConnectors } from '../../../shared/api/connectors/api';
import { knownMr, lookupMr, mrLookupFailed, numberLabel, pipelineLabel, pipelineTone } from '../../../entities/merge-request';
import { requestCreate, useCanCreate } from '../../../features/git-step';
import { showViewInActivePanel } from '../../../features/session-view-switch';
import InspectorSection from './InspectorSection.vue';

const POLL_MS = 30_000;
const props = defineProps<{ path: string; branch: string; refreshKey?: unknown }>();
const scope = useScope();

const { connectors } = useConnectors();
const hasForge = computed(() => connectors.value.some((connector) => connector.kind === 'github' || connector.kind === 'gitlab'));
const canCreate = useCanCreate(
  () => props.path,
  () => [props.branch, props.refreshKey],
);
const looked = shallowRef(false);
const mr = computed(() => knownMr(props.path, props.branch));
const failed = computed(() => (looked.value ? mrLookupFailed(props.path, props.branch) : false));
const state = computed(() => (mr.value ? (mr.value.state === 'open' && mr.value.draft ? 'draft' : mr.value.state) : 'open'));
const stateSeverity = computed(() => (state.value === 'merged' ? 'info' : state.value === 'closed' ? 'danger' : 'secondary'));

watch(
  () => [props.path, props.branch] as const,
  async ([path, branch]) => {
    looked.value = false;
    await lookupMr(path, branch);
    if (path === props.path && branch === props.branch) looked.value = true;
  },
  { immediate: true },
);

visibleInterval(scope, POLL_MS, () => {
  if (hasForge.value) void lookupMr(props.path, props.branch, true);
});
</script>

<template>
  <InspectorSection v-if="!mr && hasForge && looked && !failed" :title="t('inspector.mr.title')" testid="inspector-mr">
    <div class="row" data-testid="inspector-mr-none">
      <GitPullRequest class="icon" :size="13" />
      <span class="grow">{{ t('inspector.mr.none') }}</span>
      <VButton v-tooltip="canCreate ? t('inspector.mr.createTitle') : t('inspector.mr.createBlocked')" :label="t('inspector.mr.create')" text size="small" :disabled="!canCreate" data-testid="inspector-mr-create" @click="requestCreate(path)" />
    </div>
  </InspectorSection>

  <InspectorSection v-else-if="mr" :title="t('inspector.mr.title')" :meta="numberLabel(mr)" testid="inspector-mr">
    <div class="row">
      <GitPullRequest class="icon" :size="13" />
      <span v-tooltip="mr.title" class="grow" data-testid="inspector-mr-title">{{ mr.title }}</span>
      <VTag class="end" :severity="stateSeverity" rounded :value="t(`inspector.mr.state.${state}`)" data-testid="inspector-mr-state" />
    </div>
    <div v-if="mr.pipeline !== 'none'" class="row" data-testid="inspector-mr-pipeline">
      <span class="icon dot" :data-tone="pipelineTone(mr.pipeline)" aria-hidden="true" />
      <span class="grow">{{ t('inspector.mr.pipeline') }}</span>
      <span class="end">{{ pipelineLabel(mr.pipeline) }}</span>
    </div>
    <div v-if="mr.unresolved_threads !== null" class="row" data-testid="inspector-mr-threads">
      <MessageSquare class="icon" :size="13" />
      <span class="grow">{{ t('inspector.mr.threads') }}</span>
      <span class="end" :data-open="mr.unresolved_threads > 0">{{ mr.unresolved_threads > 0 ? t('inspector.mr.unresolved', { count: mr.unresolved_threads }) : t('inspector.mr.resolved') }}</span>
    </div>
    <div class="actions">
      <VButton :label="t('inspector.mr.open')" link size="small" data-testid="inspector-mr-open" @click="showViewInActivePanel('mr')" />
      <VButton v-tooltip="mr.url" :label="t('common.openInBrowser')" icon-pos="right" link size="small" data-testid="inspector-mr-browser" @click="openUrl(mr.url)">
        <template #icon><ExternalLink :size="11" /></template>
      </VButton>
    </div>
  </InspectorSection>
</template>

<style scoped>
.dot {
  width: 8px;
  height: 8px;
  margin: 0 2.5px;
  border-radius: var(--vr-radius-full);
  background: var(--vr-status-idle);
}

.dot[data-tone='success'] {
  background: var(--vr-color-success);
}

.dot[data-tone='crashed'] {
  background: var(--vr-status-crashed);
}

.dot[data-tone='running'] {
  background: var(--vr-status-running);
}

.end[data-open='true'] {
  color: var(--vr-status-feedback);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-3);
  padding-top: 2px;
}
</style>
