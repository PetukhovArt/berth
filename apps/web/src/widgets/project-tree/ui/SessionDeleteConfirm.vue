<script setup lang="ts">
import { VButton } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';

defineProps<{ name: string; busy: boolean; alive?: boolean }>();
const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <div class="confirm" role="group" :aria-label="t('agents.session.confirmDeleteLabel')" data-testid="session-delete-confirm">
    <span class="text">{{ t(alive ? 'agents.session.confirmDelete' : 'agents.session.confirmDeleteStopped', { name }) }}</span>
    <VButton :label="t('common.cancel')" size="small" text data-testid="session-delete-no" @click="emit('cancel')" />
    <VButton :label="t('agents.session.delete')" size="small" severity="danger" :loading="busy" data-testid="session-delete-yes" @click="emit('confirm')" />
  </div>
</template>

<style scoped>
.confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1) var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-xs);
}

.text {
  overflow-wrap: anywhere;
}
</style>
