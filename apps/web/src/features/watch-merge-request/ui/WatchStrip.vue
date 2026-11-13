<script setup lang="ts">
import { computed } from 'vue';
import { Eye } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { itemId, numberLabel } from '../../../entities/merge-request';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { lastWake, stopWatching } from '../model/watch';

const props = defineProps<{ target: ChangeRequestRef; url: string }>();
const text = computed(() => {
  const wake = lastWake(props.target);
  return wake ? t('attention.watch.sent', { text: wake }) : t('attention.watch.idle');
});
</script>

<template>
  <div :key="itemId(target)" class="strip" data-testid="cr-watch-strip">
    <Eye :size="14" class="eye" />
    <span class="label">{{ t('attention.watch.strip', { label: numberLabel({ url, number: target.number }) }) }}</span>
    <span class="text">{{ text }}</span>
    <UiButton class="stop" type="button" data-testid="cr-watch-stop" @click="stopWatching(target)">{{ t('attention.watch.stop') }}</UiButton>
  </div>
</template>

<style scoped>
.strip {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-lg);
}

.eye {
  flex: none;
}

.label {
  font-weight: 600;
  white-space: nowrap;
}

.text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.stop {
  padding: 0;
  font: inherit;
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}
</style>
