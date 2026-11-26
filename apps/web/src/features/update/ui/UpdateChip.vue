<script setup lang="ts">
import { computed } from 'vue';
import { RotateCw } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { busy, info, openUpdate, restart } from '../model/state';
import { totals } from '../model/stages';
import { installRelease, releaseStatus } from '../model/release';
import { VButton, VProgressBar, vTooltip } from '../../../shared/ui/kit';
const releaseState = computed(() => releaseStatus.value.state);
const mode = computed(() => {
  if (releaseState.value === 'ready' || releaseState.value === 'installing') return 'release';
  const state = info.value;
  if (!state?.running) return null;
  if (state.job?.state === 'running') return 'building';
  if (state.current && state.current.buildId !== state.running.buildId) return 'ready';
  return null;
});
</script>

<template>
  <VButton v-if="mode === 'building'" v-tooltip="t('update.chip.buildingHint')" size="small" severity="secondary" outlined class="chip" data-testid="update-chip-building" @click="openUpdate('update')">
    <VProgressBar class="track" :value="totals.progress" :label="t('update.chip.building')" />
    {{ t('update.chip.building') }}
  </VButton>
  <VButton v-else-if="mode === 'release'" v-tooltip="t('update.release.restartHint')" size="small" outlined class="chip" :label="t('update.chip.restart')" :disabled="releaseState === 'installing'" data-testid="release-chip-restart" @click="installRelease">
    <template #icon><RotateCw :size="12" :stroke-width="2" aria-hidden="true" /></template>
  </VButton>
  <VButton v-else-if="mode === 'ready'" v-tooltip="t('update.restartHint')" size="small" outlined class="chip" :label="t('update.chip.restart')" :disabled="busy" data-testid="update-chip-restart" @click="restart">
    <template #icon><RotateCw :size="12" :stroke-width="2" aria-hidden="true" /></template>
  </VButton>
</template>

<style scoped>
.chip {
  flex: none;
  height: 22px;
  white-space: nowrap;
}

.track {
  width: 40px;
}
</style>
