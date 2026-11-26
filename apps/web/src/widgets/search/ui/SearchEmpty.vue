<script setup lang="ts">
import { MIN_MESSAGE_QUERY } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { VButton } from '../../../shared/ui/kit';

defineProps<{ query: string; canWiden: boolean }>();
const emit = defineEmits<{ widen: []; clear: [] }>();

const { t } = useI18n();
</script>

<template>
  <div class="block" data-testid="search-empty">
    <p class="title">{{ t('search.empty.title', { query }) }}</p>
    <p class="hint">{{ t('search.empty.hint', { count: MIN_MESSAGE_QUERY }) }}</p>
    <div class="actions">
      <VButton v-if="canWiden" outlined severity="secondary" size="small" :label="t('search.empty.everywhere')" data-testid="search-empty-everywhere" @click="emit('widen')" />
      <VButton outlined severity="secondary" size="small" :label="t('search.empty.clear')" data-testid="search-empty-clear" @click="emit('clear')" />
    </div>
  </div>
</template>

<style scoped>
.block {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-2);
  min-height: 0;
  padding: var(--vr-space-6) var(--vr-page-gutter);
  text-align: center;
}

.title,
.hint {
  max-width: 100%;
  margin: 0;
  overflow-wrap: anywhere;
}

.title {
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  max-width: 36em;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--vr-space-2);
  margin-top: var(--vr-space-2);
}
</style>
