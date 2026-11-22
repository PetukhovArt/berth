<script setup lang="ts">
import { useId } from 'vue';

defineProps<{ label: string; fixed?: boolean }>();

const labelId = useId();
const controlId = useId();
</script>

<template>
  <div class="chip-field">
    <label :id="labelId" :for="fixed ? undefined : controlId"class="chip-field-label">{{ label }}</label>
    <slot :label-id="labelId" :control-id="controlId" />
  </div>
</template>

<style scoped>
.chip-field {
  display: contents;
}

.chip-field-label {
  display: none;
}

:root[data-vr-viewport='phone'] .chip-field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
}

:root[data-vr-viewport='phone'] .chip-field-label {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
