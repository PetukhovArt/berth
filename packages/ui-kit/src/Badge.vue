<script setup lang="ts">
import { computed } from 'vue';
import Badge from 'primevue/badge';

type Severity = 'secondary' | 'success' | 'info' | 'warn' | 'danger' | 'contrast';

const props = withDefaults(defineProps<{ value?: number | string; severity?: Severity; size?: 'small' | 'large' | 'xlarge'; max?: number; dot?: boolean }>(), { max: 99 });

const shown = computed(() => {
  if (props.dot || props.value === undefined || props.value === '') return undefined;
  return typeof props.value === 'number' && props.value > props.max ? `${props.max}+` : props.value;
});
</script>

<template>
  <Badge :value="shown" :severity="severity" :size="size" class="vr-badge" />
</template>
