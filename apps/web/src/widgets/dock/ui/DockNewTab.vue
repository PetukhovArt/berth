<script setup lang="ts">
import { Plus } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { startNewSession } from '../../../features/new-session';
import { UiButton } from '../../../shared/ui/UiButton';
</script>

<template>
  <UiButton class="new-tab" :title="t('agents.newSession.action')" :aria-label="t('agents.newSession.action')" data-testid="dock-new-session" @click="startNewSession"><Plus :size="14" /></UiButton>
</template>

<style scoped>
.new-tab {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 100%;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  cursor: pointer;
}

.new-tab:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
