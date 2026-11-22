<script setup lang="ts">
import { MIN_MESSAGE_QUERY } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { UiButton } from '../../../shared/ui/UiButton';

defineProps<{ query: string; canWiden: boolean }>();
const emit = defineEmits<{ widen: []; clear: [] }>();

const { t } = useI18n();
</script>

<template>
  <div class="block" data-testid="search-empty">
    <p class="title">{{ t('search.empty.title', { query }) }}</p>
    <p class="hint">{{ t('search.empty.hint', { count: MIN_MESSAGE_QUERY }) }}</p>
    <div class="actions">
      <UiButton v-if="canWiden" type="button" class="action" data-testid="search-empty-everywhere" @click="emit('widen')">{{ t('search.empty.everywhere') }}</UiButton>
      <UiButton type="button" class="action" data-testid="search-empty-clear" @click="emit('clear')">{{ t('search.empty.clear') }}</UiButton>
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

.action {
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  cursor: pointer;
  background: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
}

.action:hover {
  background: var(--vr-color-nav-hover);
}

.action:focus-visible {
  outline: 2px solid var(--vr-color-accent);
  outline-offset: 1px;
}
</style>
