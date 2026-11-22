<script setup lang="ts">
import { MIN_MESSAGE_QUERY, type ConversationSearch } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import ErrorInline from '../../../shared/ui/ErrorInline.vue';

defineProps<{ tooShort: boolean; searching: boolean; progress: ConversationSearch | null; error: string | null; compact?: boolean }>();
const emit = defineEmits<{ retry: [] }>();

const { t, formatNumber } = useI18n();
</script>

<template>
  <div class="notes" :class="{ compact }">
    <p v-if="tooShort" class="note" data-testid="search-min-chars">{{ t('search.minChars', { count: MIN_MESSAGE_QUERY }) }}</p>
    <p v-if="searching" class="note" role="status" data-testid="search-searching">{{ t('search.searching') }}</p>
    <p v-if="progress?.remote" class="note" role="status" data-testid="search-remote">{{ t('search.remote') }}</p>
    <p v-else-if="progress" class="note" role="status" data-testid="search-indexing">{{ t('search.indexing', { indexed: formatNumber(progress.indexed), total: formatNumber(progress.total_files) }) }}</p>
    <ErrorInline v-if="error" :message="t('search.failed', { message: error })" data-testid="search-error" @retry="emit('retry')" />
  </div>
</template>

<style scoped>
.notes {
  flex: none;
}

.note {
  margin: 0;
  padding: var(--vr-space-2) var(--vr-page-gutter);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.compact .note {
  padding: var(--vr-space-1) var(--vr-space-3);
}

.compact :deep(.error) {
  margin: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}
</style>
