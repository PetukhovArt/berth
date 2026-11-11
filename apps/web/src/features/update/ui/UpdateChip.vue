<script setup lang="ts">
import { computed } from 'vue';
import { RotateCw } from 'lucide-vue-next';
import { t } from '../../../shared/lib/i18n/i18n';
import { busy, info, openUpdate, restart } from '../model/state';
import { totals } from '../model/stages';

const mode = computed(() => {
  const state = info.value;
  if (!state?.running) return null;
  if (state.job?.state === 'running') return 'building';
  if (state.current && state.current.buildId !== state.running.buildId) return 'ready';
  return null;
});
</script>

<template>
  <button v-if="mode === 'building'" type="button" class="chip" data-testid="update-chip-building" :title="t('update.chip.buildingHint')" @click="openUpdate('update')">
    <span class="track" aria-hidden="true"><span class="fill" :style="{ width: `${totals.progress}%` }" /></span>
    {{ t('update.chip.building') }}
  </button>
  <button v-else-if="mode === 'ready'" type="button" class="chip ready" data-testid="update-chip-restart" :title="t('update.restartHint')" :disabled="busy" @click="restart">
    <RotateCw :size="12" :stroke-width="2" aria-hidden="true" />
    {{ t('update.chip.restart') }}
  </button>
</template>

<style scoped>
.chip {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 22px;
  white-space: nowrap;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: transparent;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.chip:hover {
  color: var(--vr-color-text);
}

.chip:focus-visible {
  outline: 2px solid var(--vr-color-accent);
  outline-offset: 1px;
}

.ready {
  color: var(--vr-color-accent);
  border-color: var(--vr-color-accent);
}

.track {
  width: 40px;
  height: 4px;
  overflow: hidden;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.fill {
  display: block;
  height: 100%;
  background: var(--vr-color-accent);
  transition: width var(--vr-motion-slow) var(--vr-motion-ease-standard);
}
</style>
