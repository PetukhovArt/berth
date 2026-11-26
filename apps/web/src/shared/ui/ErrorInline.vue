<script setup lang="ts">
import { VButton, VMessage } from './kit';
import { t } from '../lib/i18n/i18n';

defineProps<{ message: string }>();
defineEmits<{ retry: [] }>();
</script>

<template>
  <VMessage class="error" severity="error" size="small">
    <span class="text">{{ message }}</span>
    <VButton :label="t('common.retry')" size="small" severity="secondary" @click="$emit('retry')" />
  </VMessage>
</template>

<style scoped>
.error {
  margin: var(--vr-space-4);
}

.text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
