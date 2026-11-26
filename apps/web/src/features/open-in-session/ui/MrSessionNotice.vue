<script setup lang="ts">
import { LoaderCircle, TriangleAlert, X } from '../../../shared/ui/icons';
import { VButton, VMessage, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import type { RepoRef } from '../../../entities/repo';
import { useOpenInSession, type MrRun } from '../model/openInSession';

defineProps<{ run: MrRun }>();
defineSlots<{ project(props: { repo: RepoRef }): unknown }>();

const { open, dismiss } = useOpenInSession();
</script>

<template>
  <div class="notice" :data-state="run.busy ? 'busy' : run.error ? 'error' : run.needsProject ? 'project' : run.endedChoice ? 'choice' : 'warning'" role="status" data-testid="mr-session-notice">
    <div class="head">
      <LoaderCircle v-if="run.busy" :size="12" class="spin" />
      <TriangleAlert v-else :size="12" />
      <span v-tooltip="run.label" class="label">{{ run.label }}</span>
      <VButton v-if="!run.busy" v-tooltip="t('forge.openSession.dismiss')" class="close" text severity="secondary" size="small" :aria-label="t('forge.openSession.dismiss')" data-testid="mr-session-dismiss" @click="dismiss(run.id)">
        <template #icon><X :size="12" /></template>
      </VButton>
    </div>
    <p v-if="run.message" class="text" data-testid="mr-session-message">{{ run.message }}</p>
    <VMessage v-if="run.error" severity="error" size="small" data-testid="mr-session-error">{{ run.error }}</VMessage>
    <p v-if="run.warning" class="text warning" data-testid="mr-session-warning">{{ run.warning }}</p>
    <div v-if="!run.busy && run.endedChoice" class="actions">
      <VButton :label="t('forge.openSession.continueEnded')" size="small" severity="secondary" data-testid="mr-session-continue" @click="open(run.item, run.refs, 'continue')" />
      <VButton :label="t('forge.openSession.newSession')" size="small" text severity="secondary" data-testid="mr-session-new" @click="open(run.item, run.refs, 'new')" />
    </div>
    <div v-else-if="!run.busy && (run.error || run.needsProject)" class="actions">
      <slot v-if="run.needsProject" name="project" :repo="run.needsProject" />
      <VButton :label="t('forge.openSession.retry')" size="small" text severity="secondary" data-testid="mr-session-retry" @click="open(run.item, run.refs)" />
    </div>
  </div>
</template>

<style scoped>
.notice {
  margin: 0 0 var(--vr-space-2);
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-weight: 500;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.close {
  flex: none;
  width: 18px;
  height: 18px;
}

.text {
  margin: var(--vr-space-1) 0 0;
  overflow-wrap: anywhere;
  color: var(--vr-color-text-muted);
}

.warning {
  color: var(--vr-color-conflict);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  margin-top: var(--vr-space-1);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
