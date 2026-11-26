<script setup lang="ts">
import { ChevronLeft } from './icons';
import { VButton } from './kit';
import { useI18n } from '../lib/i18n/i18n';

defineProps<{ title?: string }>();
const emit = defineEmits<{ back: [] }>();

const { t } = useI18n();
</script>

<template>
  <div class="bar">
    <VButton class="back" text severity="secondary" :label="t('mobile.back')" data-testid="mobile-back" @click="emit('back')">
      <template #icon><ChevronLeft :size="18" /></template>
    </VButton>
    <span v-if="title" class="title">{{ title }}</span>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 2.75rem;
  padding: 0 var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
}

.back {
  flex: none;
  min-height: var(--vr-size-row-touch);
}

.title {
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
