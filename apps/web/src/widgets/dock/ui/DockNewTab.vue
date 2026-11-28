<script setup lang="ts">
import { Plus } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { startNewSession } from '../../../features/new-session';
</script>

<template>
  <VButton v-tooltip="t('agents.newSession.action')" class="new-tab" text severity="secondary" size="small" :aria-label="t('agents.newSession.action')" data-testid="dock-new-session" @click="startNewSession">
    <template #icon><Plus :size="14" /></template>
  </VButton>
</template>

<style scoped>
.new-tab {
  flex: none;
  width: var(--vr-size-control);
  height: var(--vr-size-control);
}
</style>
