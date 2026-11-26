<script setup lang="ts">
import { FileText, X } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';

defineProps<{ label: string; src?: string | null; removable?: boolean }>();
const emit = defineEmits<{ remove: [] }>();
</script>

<template>
  <span v-tooltip="label" class="chip" data-testid="chat-attachment">
    <img v-if="src" class="thumb" :src="src" :alt="label" />
    <FileText v-else :size="14" class="icon" />
    <span class="label">{{ label }}</span>
    <VButton v-if="removable" class="remove" text severity="secondary" size="small" :aria-label="t('chat.attach.remove', { name: label })" data-testid="chat-attachment-remove" @click="emit('remove')">
      <template #icon><X :size="12" /></template>
    </VButton>
  </span>
</template>

<style scoped>
.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  max-width: 220px;
  height: 28px;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.thumb {
  flex: none;
  width: 20px;
  height: 20px;
  object-fit: cover;
  border-radius: var(--vr-radius-chip);
}

.icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.label {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.remove {
  flex: none;
  width: 16px;
  height: 16px;
}
</style>
