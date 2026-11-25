<script setup lang="ts">
import { computed } from 'vue';
import { VDialog } from '../../../shared/ui/kit';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { checkRelease, closeRelease, installRelease, releaseFailure, releaseOpen, releaseStatus } from '../model/release';

const status = computed(() => releaseStatus.value);

const progress = computed(() => {
  const value = status.value;
  if (value.state !== 'downloading' || !value.total) return null;
  return Math.min(100, Math.round((value.downloaded / value.total) * 100));
});

const actions = computed(() => {
  const state = status.value.state;
  if (state === 'ready') return { cancel: t('update.later'), cancelTestid: 'release-later', confirm: t('update.restart'), confirmTestid: 'release-restart' };
  if (state === 'failed' || state === 'upToDate') return { cancel: t('common.close'), cancelTestid: undefined, confirm: t('update.release.check'), confirmTestid: 'release-check' };
  return { cancel: t('common.close'), cancelTestid: 'release-close', confirm: undefined, confirmTestid: undefined };
});

function confirm() {
  if (status.value.state === 'ready') void installRelease();
  else void checkRelease();
}

function onVisible(visible: boolean) {
  if (!visible) closeRelease();
}
</script>

<template>
  <VDialog :visible="releaseOpen" modal :header="t('update.title')" :style="{ width: '560px' }" :draggable="false" data-testid="release-dialog" @update:visible="onVisible">
    <div class="body" :data-state="status.state" data-testid="release-body">
      <p v-if="status.state === 'idle' || status.state === 'checking'">{{ t('update.release.checking') }}</p>

      <p v-else-if="status.state === 'upToDate'" data-testid="release-up-to-date">{{ t('update.upToDate') }}</p>

      <template v-else-if="status.state === 'downloading'">
        <p data-testid="release-downloading">{{ t('update.release.downloading', { version: status.version }) }}</p>
        <div v-if="progress !== null" class="bar" role="progressbar" :aria-valuenow="progress" aria-valuemin="0" aria-valuemax="100">
          <div class="fill" :style="{ width: `${progress}%` }" />
        </div>
      </template>

      <template v-else-if="status.state === 'ready'">
        <p data-testid="release-ready">{{ t('update.release.ready', { version: status.version }) }}</p>
        <p class="muted">{{ t('update.release.restartHint') }}</p>
        <template v-if="status.notes">
          <p class="heading">{{ t('update.release.notes') }}</p>
          <div class="notes" data-testid="release-notes">{{ status.notes }}</div>
        </template>
      </template>

      <p v-else-if="status.state === 'installing'" data-testid="release-installing">{{ t('update.release.installing', { version: status.version }) }}</p>

      <p v-else-if="status.state === 'failed'" class="danger" data-testid="release-failed">{{ t('update.release.failed', { error: status.error }) }}</p>

      <p v-if="releaseFailure" class="danger" data-testid="release-error">{{ releaseFailure }}</p>
    </div>

    <DialogActions :cancel-label="actions.cancel" :confirm-label="actions.confirm" :cancel-testid="actions.cancelTestid" :confirm-testid="actions.confirmTestid" @cancel="closeRelease" @confirm="confirm" />
  </VDialog>
</template>

<style scoped>
.body {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin-bottom: var(--vr-space-4);
}

.body p {
  margin: 0;
  overflow-wrap: anywhere;
}

.muted {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-sm);
}

.danger {
  color: var(--vr-color-danger);
}

.heading {
  margin-top: var(--vr-space-2);
  font-weight: 600;
}

.notes {
  max-height: 240px;
  overflow: auto;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
  white-space: pre-wrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.bar {
  height: 6px;
  overflow: hidden;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.fill {
  height: 100%;
  background: var(--vr-color-accent);
  border-radius: inherit;
  transition: width var(--vr-motion-slow) var(--vr-motion-ease-standard);
}
</style>
