<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';

const props = withDefaults(defineProps<{ active?: boolean; density?: 'compact' | 'regular'; stacked?: boolean; align?: 'center' | 'start'; depth?: number }>(), { density: 'regular', align: 'center', depth: 0 });

const button = useTemplateRef<HTMLButtonElement>('button');
const indent = computed(() => (props.depth ? { '--row-depth': props.depth } : undefined));

defineExpose({ focus: (options?: FocusOptions) => button.value?.focus(options), el: button });
</script>

<template>
  <button
    ref="button"
    class="row-button"
    type="button"
    :style="indent"
    :data-active="active || undefined"
    :data-density="density"
    :data-stacked="stacked || undefined"
    :data-align="align"
    :aria-current="active ? 'true' : undefined"
  >
    <span v-if="$slots.leading" class="leading"><slot name="leading" /></span>
    <span class="main"><slot /></span>
    <span v-if="$slots.trailing" class="trailing"><slot name="trailing" /></span>
  </button>
</template>

<style scoped>
.row-button {
  --row-depth: 0;
  display: flex;
  align-items: center;
  gap: 7px;
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: var(--vr-row-min);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-2) + var(--row-depth) * var(--vr-space-3));
  color: inherit;
  font: inherit;
  font-size: var(--vr-font-size-label);
  text-align: left;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.row-button[data-align='start'] {
  align-items: flex-start;
}

.row-button[data-density='compact'] {
  min-height: max(28px, var(--vr-row-min));
  padding-block: 0;
  font-size: var(--vr-font-size-xs);
  border-radius: var(--vr-radius-sm);
}

.row-button:hover:not(:disabled) {
  background: var(--vr-color-nav-hover);
}

.row-button[data-active] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.row-button:focus-visible {
  outline: 2px solid var(--vr-color-focus);
  outline-offset: -2px;
}

.row-button:disabled {
  cursor: default;
  opacity: 0.5;
}

.row-button[data-stacked] {
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  padding-inline: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  text-align: center;
}

.row-button[data-stacked][data-active] {
  color: var(--vr-color-text);
  background: none;
}

.leading,
.trailing {
  display: inline-flex;
  flex: none;
  align-items: center;
}

.main {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.row-button[data-stacked] .main {
  flex: none;
}
</style>
