<script setup lang="ts">
import { X } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { baseNotice, dismissBaseNotice } from '../model/baseNotice';
</script>

<template>
  <div v-if="baseNotice" class="notice" role="status" data-testid="new-session-base-notice">
    <span class="text">{{ baseNotice }}</span>
    <VButton v-tooltip="t('common.close')" size="small" text severity="secondary" :aria-label="t('common.close')" data-testid="new-session-base-notice-close" @click="dismissBaseNotice">
      <template #icon><X :size="14" /></template>
    </VButton>
  </div>
</template>

<style scoped>
.notice {
  position: fixed;
  right: var(--vr-space-4);
  bottom: var(--vr-space-6);
  z-index: 1100;
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-2);
  max-width: 28rem;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-left: 3px solid var(--vr-color-conflict);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.text {
  overflow-wrap: anywhere;
}
</style>
