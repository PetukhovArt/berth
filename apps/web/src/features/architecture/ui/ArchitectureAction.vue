<script setup lang="ts">
import { computed } from 'vue';
import { Network } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { numberLabel } from '../../../entities/merge-request';
import { openArchitecture, useArchitecture } from '../model/stratum';

type Item = { connector: string; project: string; number: number; target_branch: string; url: string };

const props = withDefaults(defineProps<{ item: Item; variant?: 'button' | 'icon' }>(), { variant: 'button' });

const { availability } = useArchitecture(() => props.item);
const shown = computed(() => (props.variant === 'icon' ? availability.value.status === 'ready' : availability.value.status !== 'hidden'));
const reason = computed(() => {
  const state = availability.value;
  if (state.status === 'disabled') return state.reason;
  if (state.status === 'checking') return t('forge.stratum.checking');
  return t('forge.stratum.title');
});

function open() {
  const state = availability.value;
  if (state.status === 'ready') openArchitecture(state.params, numberLabel(props.item));
}
</script>

<template>
  <template v-if="shown && variant === 'button'">
    <VButton v-tooltip="reason" size="small" severity="secondary" :label="t('forge.stratum.action')" :disabled="availability.status !== 'ready'" data-testid="cr-architecture" @click="open">
      <template #icon><Network :size="14" /></template>
    </VButton>
    <span v-if="availability.status === 'disabled'" v-tooltip="reason" class="why" data-testid="cr-architecture-why">{{ reason }}</span>
  </template>
  <VButton v-else-if="shown" v-tooltip="t('forge.stratum.title')" size="small" severity="secondary" text :aria-label="t('forge.stratum.action')" data-testid="mr-architecture" @click.stop="open">
    <template #icon><Network :size="14" /></template>
  </VButton>
</template>

<style scoped>
.why {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 240px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
