<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { VButton, VDialog } from '@ui/kit';
import { t } from '../shared/lib/i18n/i18n';
import { busy, cancelBuild, closeUpdate, daemonUpdated, dialogMode, failure, info, restart, rollBack, short, startBuild, updateDaemon } from './state';
import { clock, now, rows, STAGE_LABELS, totals } from './stages';

const INSTALL_COMMAND = 'powershell -ExecutionPolicy Bypass -File scripts\\install-app.ps1';

const confirmDaemon = ref(false);
watch(dialogMode, () => (confirmDaemon.value = false));

const running = computed(() => info.value?.running ?? null);
const current = computed(() => info.value?.current ?? null);
const previous = computed(() => info.value?.previous ?? null);
const job = computed(() => info.value?.job ?? null);
const head = computed(() => info.value?.head ?? null);

const phase = computed(() => {
  if (!info.value) return 'loading';
  if (!running.value) return 'notInstalled';
  if (job.value?.state === 'running') return 'building';
  if (current.value && current.value.buildId !== running.value.buildId) return 'pending';
  if (dialogMode.value === 'rollback') return 'rollback';
  if (job.value?.state === 'failed' || job.value?.state === 'cancelled') return job.value.state;
  if (!info.value.sourceRepo) return 'noRepo';
  if (!head.value) return 'headError';
  return head.value.commit === running.value.commit ? 'upToDate' : 'available';
});

const ticker = setInterval(() => (now.value = Date.now()), 1000);
onUnmounted(() => clearInterval(ticker));

const runningLabel = computed(() => {
  const row = rows.value.find((item) => item.state === 'running');
  return row ? t(STAGE_LABELS[row.stage]) : '';
});

function rowTime(state: string, ms: number): string {
  if (state === 'skipped') return t('update.skipped');
  if (state === 'pending') return `~${clock(ms)}`;
  return clock(ms);
}

const daemonMismatch = computed(() => {
  const daemon = info.value?.daemonProtocol;
  const build = current.value?.protocol;
  return daemon && build && daemon !== build ? { daemon, build } : null;
});

const daemonNotice = computed(() => {
  if (daemonMismatch.value) return t('update.daemonMismatch');
  const build = current.value;
  const daemon = info.value?.daemonBuild ?? null;
  if (!build || daemon?.buildId === build.buildId) return null;
  return daemon ? t('update.daemonBehind') : t('update.daemonUnknown');
});

async function start() {
  await startBuild();
  if (info.value?.job?.state === 'running') closeUpdate();
}

function onVisible(visible: boolean) {
  if (!visible) closeUpdate();
}
</script>

<template>
  <VDialog :visible="dialogMode !== null" modal :header="dialogMode === 'rollback' ? t('update.rollbackTitle') : t('update.title')" :style="{ width: '560px' }" :draggable="false" data-testid="update-dialog" @update:visible="onVisible">
    <div class="body" :data-phase="phase" data-testid="update-body">
      <template v-if="phase === 'notInstalled'">
        <p>{{ t('update.notInstalled') }}</p>
        <code class="command">{{ INSTALL_COMMAND }}</code>
      </template>

      <p v-else-if="phase === 'noRepo'">{{ t('update.noRepo') }}</p>

      <p v-else-if="phase === 'headError'" class="danger">{{ t('update.headError', { repo: info?.sourceRepo ?? '', error: info?.headError ?? '' }) }}</p>

      <p v-else-if="phase === 'upToDate'" data-testid="update-up-to-date">{{ t('update.upToDate') }}</p>

      <template v-else-if="phase === 'available'">
        <p data-testid="update-available">{{ t('update.available') }}</p>
        <p class="muted">{{ t('update.availableHint') }}</p>
      </template>

      <div v-else-if="phase === 'building'" class="progress" data-testid="update-building">
        <div class="bar" role="progressbar" :aria-valuenow="totals.progress" aria-valuemin="0" aria-valuemax="100" :aria-valuetext="runningLabel">
          <div class="fill" :style="{ width: `${totals.progress}%` }" />
        </div>
        <div class="summary muted">
          <span>{{ t('update.elapsed', { time: clock(totals.elapsed) }) }}</span>
          <span v-if="totals.left > 0">{{ t('update.left', { time: clock(totals.left) }) }}</span>
        </div>
        <ol class="stages" data-testid="update-stages">
          <li v-for="row in rows" :key="row.stage" class="stage" :data-state="row.state" :data-testid="`update-stage-${row.stage}`">
            <span class="mark" aria-hidden="true" />
            <span class="label">{{ t(STAGE_LABELS[row.stage]) }}</span>
            <span class="time">{{ rowTime(row.state, row.ms) }}</span>
          </li>
        </ol>
      </div>

      <template v-else-if="(phase === 'failed' || phase === 'cancelled') && job">
        <p :class="{ danger: phase === 'failed' }" data-testid="update-failed">{{ phase === 'failed' ? t('update.failed') : t('update.cancelled') }}</p>
        <p v-if="phase === 'failed' && job.log" class="muted path">{{ t('update.log', { path: job.log }) }}</p>
      </template>

      <template v-else-if="phase === 'pending'">
        <p data-testid="update-installed">{{ t('update.installed') }}</p>
        <p class="muted">{{ t('update.restartHint') }}</p>
      </template>

      <template v-else-if="phase === 'rollback'">
        <p v-if="previous" data-testid="update-previous">{{ t('update.rollbackHint', { build: previous.buildId, commit: short(previous.commit), subject: previous.subject }) }}</p>
        <p v-else>{{ t('update.noPrevious') }}</p>
      </template>

      <div v-if="daemonNotice" class="daemon" data-testid="update-daemon">
        <p>{{ daemonNotice }}</p>
        <p v-if="confirmDaemon" class="danger">{{ t('update.daemonConfirm') }}</p>
      </div>
      <p v-if="daemonUpdated" data-testid="update-daemon-done">{{ t('update.daemonDone') }}</p>

      <p v-if="failure" class="error" data-testid="update-error">{{ failure }}</p>
    </div>

    <div class="actions">
      <template v-if="daemonNotice && phase !== 'building'">
        <VButton v-if="!confirmDaemon" :label="t('update.daemonUpdate')" size="small" severity="danger" text :disabled="busy" data-testid="update-daemon-start" @click="confirmDaemon = true" />
        <VButton v-else :label="t('update.daemonUpdate')" size="small" severity="danger" :disabled="busy" data-testid="update-daemon-confirm" @click="updateDaemon().then(() => (confirmDaemon = false))" />
      </template>
      <span class="spacer" />
      <template v-if="phase === 'building'">
        <VButton :label="t('update.cancel')" size="small" severity="danger" text :disabled="busy" data-testid="update-cancel" @click="cancelBuild" />
        <VButton :label="t('update.minimize')" size="small" data-testid="update-minimize" @click="closeUpdate" />
      </template>
      <template v-else-if="phase === 'pending'">
        <VButton :label="t('update.later')" size="small" text data-testid="update-later" @click="closeUpdate" />
        <VButton :label="t('update.restart')" size="small" :disabled="busy" data-testid="update-restart" @click="restart" />
      </template>
      <template v-else-if="phase === 'rollback'">
        <VButton :label="t('common.close')" size="small" text @click="closeUpdate" />
        <VButton v-if="previous" :label="t('update.rollback', { build: previous.buildId })" size="small" :disabled="busy" data-testid="update-rollback" @click="rollBack" />
      </template>
      <template v-else-if="phase === 'available' || phase === 'failed' || phase === 'cancelled'">
        <VButton :label="t('common.close')" size="small" text @click="closeUpdate" />
        <VButton :label="phase === 'available' ? t('update.start') : t('update.retry')" size="small" :disabled="busy" data-testid="update-start" @click="start" />
      </template>
      <VButton v-else :label="t('common.close')" size="small" text data-testid="update-close" @click="closeUpdate" />
    </div>
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

.danger,
.error {
  color: var(--vr-color-danger);
}

.progress {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
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

.summary {
  display: flex;
  justify-content: space-between;
  font-variant-numeric: tabular-nums;
}

.stages {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  margin: var(--vr-space-2) 0 0;
  padding: 0;
  list-style: none;
}

.stage {
  display: grid;
  grid-template-columns: 8px 1fr auto;
  align-items: center;
  gap: var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.stage[data-state='running'] {
  color: var(--vr-color-text);
  font-weight: 600;
}

.stage[data-state='done'] .label {
  color: var(--vr-color-text);
}

.mark {
  width: 8px;
  height: 8px;
  border: 1px solid var(--vr-color-border);
  border-radius: 50%;
}

.stage[data-state='done'] .mark {
  background: var(--vr-color-accent);
  border-color: var(--vr-color-accent);
}

.stage[data-state='running'] .mark {
  border-color: var(--vr-color-accent);
  animation: pulse 1.2s ease-in-out infinite;
}

.stage[data-state='skipped'] .label {
  text-decoration: line-through;
}

.time {
  font-variant-numeric: tabular-nums;
}

@keyframes pulse {
  50% {
    background: var(--vr-color-accent);
  }
}

@media (prefers-reduced-motion: reduce) {
  .stage[data-state='running'] .mark {
    animation: none;
    background: var(--vr-color-accent);
  }
}

.command,
.path {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.command {
  padding: var(--vr-space-2);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
  user-select: all;
}

.daemon {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.actions {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.spacer {
  flex: 1;
}
</style>
