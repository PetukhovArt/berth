<script setup lang="ts">
import { computed } from 'vue';
import { TriangleAlert } from '../../../shared/ui/icons';
import { VTag, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useOverlaps } from '../model/overlaps';

const props = defineProps<{ agentId: string; compact?: boolean }>();

const overlap = computed(() => useOverlaps().value.get(props.agentId));
const label = computed(() => (overlap.value ? t('agents.overlap.label', { names: overlap.value.names.join(', ') }) : ''));
const title = computed(() => (overlap.value ? t('agents.overlap.title', { names: overlap.value.names.join(', '), modules: overlap.value.modules.join(', ') }) : ''));
</script>

<template>
  <VTag v-if="overlap" v-tooltip="title" class="overlap" :class="{ compact }" severity="secondary" rounded :aria-label="title" data-testid="overlap-chip">
    <TriangleAlert :size="12" />
    <span v-if="!compact" class="label">{{ label }}</span>
  </VTag>
</template>

<style scoped>
.overlap {
  flex: 0 1 auto;
  gap: 5px;
  min-width: 0;
  max-width: 260px;
  height: 22px;
  padding: 0 8px;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  font-weight: 400;
  white-space: nowrap;
}

.label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.overlap.compact {
  height: auto;
  padding: 0;
  background: none;
}
</style>
