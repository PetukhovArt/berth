<script setup lang="ts">
import Tab from 'primevue/tab';
import TabList from 'primevue/tablist';
import TabPanel from 'primevue/tabpanel';
import TabPanels from 'primevue/tabpanels';
import Tabs from 'primevue/tabs';
import Badge from './Badge.vue';
import { vTooltip } from './Tooltip';

interface TabOption {
  value: string;
  label: string;
  count?: number;
  tone?: 'info' | 'success' | 'warn' | 'danger';
  title?: string;
  disabled?: boolean;
}

withDefaults(defineProps<{ options: TabOption[]; label: string; testId: string; scrollable?: boolean }>(), { scrollable: false });
const model = defineModel<string>({ required: true });
</script>

<template>
  <Tabs v-model:value="model" class="vr-tabs" :scrollable="scrollable" lazy>
    <TabList :aria-label="label">
      <Tab v-for="option in options" :key="option.value" v-tooltip="option.title" :value="option.value" :disabled="option.disabled" :data-testid="`${testId}-${option.value}`">
        <slot name="option" :option="option">{{ option.label }}</slot>
        <Badge v-if="option.count" :value="option.count" :severity="option.tone ?? 'secondary'" size="small" :data-testid="`${testId}-count-${option.value}`" />
      </Tab>
    </TabList>
    <TabPanels v-if="$slots.panel">
      <TabPanel v-for="option in options" :key="option.value" :value="option.value">
        <slot name="panel" :option="option" />
      </TabPanel>
    </TabPanels>
  </Tabs>
</template>
