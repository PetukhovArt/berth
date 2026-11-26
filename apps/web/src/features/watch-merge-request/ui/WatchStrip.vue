<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { Eye } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { itemId, numberLabel } from '../../../entities/merge-request';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import { lastWake, refreshWatches, stopWatching } from '../model/watch';

const REFRESH_MS = 30_000;

const props = defineProps<{ target: ChangeRequestRef; url: string; sessionName?: string }>();
const scope = useScope();
const refresh = () => void refreshWatches().catch(() => undefined);
onMounted(() => {
  refresh();
  visibleInterval(scope, REFRESH_MS, refresh);
});
const text = computed(() => {
  const wake = lastWake(props.target);
  const name = props.sessionName;
  if (wake) return name ? t('attention.watch.sentTo', { name, text: wake }) : t('attention.watch.sent', { text: wake });
  return name ? t('attention.watch.idleNamed', { name }) : t('attention.watch.idle');
});
</script>

<template>
  <div :key="itemId(target)" class="strip" data-testid="cr-watch-strip">
    <Eye :size="14" class="eye" />
    <span class="label">{{ t('attention.watch.strip', { label: numberLabel({ url, number: target.number }) }) }}</span>
    <span class="text">{{ text }}</span>
    <VButton link size="small" :label="t('attention.watch.stop')" data-testid="cr-watch-stop" @click="stopWatching(target)" />
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
</style>
