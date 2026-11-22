<script setup lang="ts">
interface ChipOption {
  value: string;
  label: string;
  count?: number;
  title?: string;
}

const props = defineProps<{ options: ChipOption[]; label: string; testId: string }>();
const model = defineModel<string>({ required: true });

const tabbable = (value: string) => (props.options.some((option) => option.value === model.value) ? model.value === value : props.options[0]?.value === value);

function targetIndex(key: string): number | null {
  const last = props.options.length - 1;
  const at = props.options.findIndex((option) => option.value === model.value);
  if (key === 'ArrowRight' || key === 'ArrowDown') return at >= last ? 0 : at + 1;
  if (key === 'ArrowLeft' || key === 'ArrowUp') return at <= 0 ? last : at - 1;
  if (key === 'Home') return 0;
  if (key === 'End') return last;
  return null;
}

function onKeydown(event: KeyboardEvent) {
  const index = targetIndex(event.key);
  const option = index === null ? undefined : props.options[index];
  if (index === null || !option || !(event.currentTarget instanceof HTMLElement)) return;
  event.preventDefault();
  model.value = option.value;
  event.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[index]?.focus();
}
</script>

<template>
  <div class="chip-group" role="radiogroup" :aria-label="label" @keydown="onKeydown">
    <button
      v-for="option in options"
      :key="option.value"
      class="chip"
      type="button"
      role="radio"
      :aria-checked="model === option.value"
      :tabindex="tabbable(option.value) ? 0 : -1"
      :title="option.title"
      :data-testid="`${testId}-${option.value}`"
      @click="model = option.value"
    >
      <slot name="option" :option="option">{{ option.label }}</slot>
      <span v-if="option.count !== undefined" class="count">{{ option.count }}</span>
    </button>
  </div>
</template>

<style scoped>
.chip-group {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  min-width: 0;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  max-width: 100%;
  padding: var(--vr-space-1) var(--vr-space-2);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  background: none;
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-label);
  cursor: pointer;
}

.chip:hover {
  background: var(--vr-color-nav-hover);
}

.chip[aria-checked='true'] {
  border-color: var(--vr-color-border-strong);
  background: var(--vr-color-nav-active);
  color: var(--vr-color-text);
}

.chip:focus-visible {
  outline: 2px solid var(--vr-color-focus);
  outline-offset: 1px;
}

.count {
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

:root[data-vr-viewport='phone'] .chip {
  min-height: var(--vr-size-row);
}
</style>
