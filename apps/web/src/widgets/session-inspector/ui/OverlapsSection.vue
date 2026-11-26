<script setup lang="ts">
import { computed } from 'vue';
import { TriangleAlert } from '../../../shared/ui/icons';
import { vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useOverlaps } from '../../../features/overlaps';
import InspectorSection from './InspectorSection.vue';

const props = defineProps<{ agentId: string }>();

const overlaps = useOverlaps();
const overlap = computed(() => overlaps.value.get(props.agentId));
const modules = computed(() => overlap.value?.modules.join(', ') ?? '');
</script>

<template>
  <InspectorSection :title="t('inspector.overlaps.title')" :meta="overlap ? String(overlap.names.length) : undefined" testid="inspector-overlaps">
    <template v-if="overlap">
      <div v-for="name in overlap.names" :key="name" class="row" data-testid="inspector-overlap">
        <TriangleAlert class="icon" :size="12" />
        <span v-tooltip="name" class="grow">{{ name }}</span>
      </div>
      <p v-tooltip="modules" class="note modules">{{ t('inspector.overlaps.modules', { modules }) }}</p>
    </template>
    <p v-else class="note">{{ t('inspector.overlaps.none') }}</p>
  </InspectorSection>
</template>

<style scoped>
.modules {
  overflow-wrap: anywhere;
}
</style>
