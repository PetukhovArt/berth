<script setup lang="ts">
import { computed } from 'vue';
import ProgressBar from 'primevue/progressbar';

const props = withDefaults(defineProps<{ value?: number | null; label: string; severity?: 'success' | 'warn' | 'danger' | 'info' }>(), { value: null, severity: undefined });

const indeterminate = computed(() => props.value === null);
</script>

<template>
  <ProgressBar
    class="vr-progress"
    :class="severity && `vr-progress-${severity}`"
    :value="indeterminate ? undefined : (value ?? undefined)"
    :mode="indeterminate ? 'indeterminate' : 'determinate'"
    :show-value="false"
    :aria-label="label"
  />
</template>
