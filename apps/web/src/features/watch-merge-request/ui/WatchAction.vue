<script setup lang="ts">
import { computed } from 'vue';
import { Eye } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
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
    size="small"
    severity="secondary"
    :label="on ? t('attention.watch.actionOn') : t('attention.watch.action')"
    :title="t('attention.watch.title')"
    :aria-pressed="on"
    data-testid="cr-watch"
    @click="toggleWatch(target)"
  >
    <template #icon><Eye :size="14" /></template>
  </VButton>
</template>
