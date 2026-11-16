<script setup lang="ts">
import { computed, watch } from 'vue';
import { ExternalLink, GitPullRequest, MessageSquare } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { openUrl } from '../../../shared/api/connectors/api';
import { knownMr, lookupMr, numberLabel, pipelineLabel, pipelineTone } from '../../../entities/merge-request';
import { showViewInActivePanel } from '../../../features/session-view-switch';
import InspectorSection from './InspectorSection.vue';

const props = defineProps<{ path: string; branch: string }>();

const mr = computed(() => knownMr(props.path, props.branch));
const state = computed(() => (mr.value ? (mr.value.state === 'open' && mr.value.draft ? 'draft' : mr.value.state) : 'open'));

watch(
  () => [props.path, props.branch] as const,
  ([path, branch]) => void lookupMr(path, branch),
  { immediate: true },
);
</script>

<template>
  <InspectorSection v-if="mr" :title="t('inspector.mr.title')" :meta="numberLabel(mr)" testid="inspector-mr">
    <div class="row">
      <GitPullRequest class="icon" :size="13" />
      <span class="grow" :title="mr.title" data-testid="inspector-mr-title">{{ mr.title }}</span>
      <span class="end state" :data-state="state" data-testid="inspector-mr-state">{{ t(`inspector.mr.state.${state}`) }}</span>
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
      <UiButton type="button" class="link" data-testid="inspector-mr-open" @click="showViewInActivePanel('mr')">{{ t('inspector.mr.open') }}</UiButton>
      <UiButton type="button" class="link external" :title="mr.url" data-testid="inspector-mr-browser" @click="openUrl(mr.url)">
        {{ t('common.openInBrowser') }}
        <ExternalLink :size="11" />
      </UiButton>
    </div>
  </InspectorSection>
</template>

<style scoped>
.state {
  padding: 0 6px;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-surface-muted);
}

.state[data-state='merged'] {
  color: var(--vr-status-unseen);
}

.state[data-state='closed'] {
  color: var(--vr-status-crashed);
}

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

.external {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
</style>
