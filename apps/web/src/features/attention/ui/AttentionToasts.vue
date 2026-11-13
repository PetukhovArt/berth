<script setup lang="ts">
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { attentionToasts, dismissAttention, openAttention } from '../model/attention';
</script>

<template>
  <ul v-if="attentionToasts.length" class="toasts" data-testid="attention-toasts">
    <li v-for="toast in attentionToasts" :key="toast.id" class="toast" :data-kind="toast.kind">
      <UiButton type="button" class="open" data-testid="attention-toast" @click="openAttention(toast)">{{ toast.text }}</UiButton>
      <UiButton type="button" class="close" :aria-label="t('attention.dismiss')" :title="t('attention.dismiss')" @click="dismissAttention(toast.id)">×</UiButton>
    </li>
  </ul>
</template>

<style scoped>
.toasts {
  position: fixed;
  right: var(--vr-space-4);
  bottom: var(--vr-space-6);
  z-index: 1100;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.open,
.close {
  padding: 0;
  font: inherit;
  color: inherit;
  cursor: pointer;
  background: none;
  border: 0;
}

.close {
  color: var(--vr-color-text-muted);
}
</style>
