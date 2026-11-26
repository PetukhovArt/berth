<script setup lang="ts">
import { computed } from 'vue';
import { Eye } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { useBerth } from '../../../entities/session';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { isWatching, toggleWatch } from '../model/watch';

const props = defineProps<{ target: ChangeRequestRef }>();
const berth = useBerth();
const on = computed(() => isWatching(props.target));
</script>

<template>
  <VButton
    v-if="berth"
    v-tooltip="t('attention.watch.title')"
    size="small"
    severity="secondary"
    :label="on ? t('attention.watch.actionOn') : t('attention.watch.action')"
    :aria-pressed="on"
    data-testid="cr-watch"
    @click="toggleWatch(target)"
  >
    <template #icon><Eye :size="14" /></template>
  </VButton>
</template>

<style scoped>
button[aria-pressed='true'] {
  --p-button-secondary-background: var(--vr-color-nav-active);
  --p-button-secondary-hover-background: var(--vr-color-nav-active);
}
</style>
