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
  const chip = event.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[index];
  chip?.focus({ preventScroll: true });
  revealChip(chip);
}

function revealChip(chip: Element | null | undefined) {
  chip?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

function onClick(event: MouseEvent, value: string) {
  model.value = value;
  revealChip(event.currentTarget instanceof Element ? event.currentTarget : null);
}

function onWheel(event: WheelEvent) {
  const group = event.currentTarget;
  if (!(group instanceof HTMLElement) || Math.abs(event.deltaX) >= Math.abs(event.deltaY)) return;
  const max = group.scrollWidth - group.clientWidth;
  if (max <= 0) return;
  const next = Math.min(max, Math.max(0, group.scrollLeft + event.deltaY));
  if (next === group.scrollLeft) return;
  event.preventDefault();
  group.scrollLeft = next;
}
</script>

<template>
  <div class="chip-group" role="radiogroup" :aria-label="label" @keydown="onKeydown" @wheel="onWheel">
    <button
      v-for="option in options"
      :key="option.value"
      class="chip"
      :class="{ empty: option.count === 0 }"
      type="button"
      role="radio"
      :aria-checked="model === option.value"
      :tabindex="tabbable(option.value) ? 0 : -1"
      :title="option.title"
      :data-testid="`${testId}-${option.value}`"
      @click="onClick($event, option.value)"
    >
      <slot name="option" :option="option">{{ option.label }}</slot>
      <span v-if="option.count !== undefined" class="count">{{ option.count }}</span>
    </button>
  </div>
</template>

<style scoped>
.chip-group {
  display: flex;
  flex-wrap: nowrap;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow-x: auto;
  scroll-snap-type: x proximity;
  scrollbar-width: none;
}

.chip-group::-webkit-scrollbar {
  display: none;
}

.chip {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  scroll-snap-align: start;
  white-space: nowrap;
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

.chip.empty,
.chip.empty .count {
  color: var(--vr-color-text-subtle);
}

.chip[aria-checked='true'] {
  border-color: var(--vr-color-text-subtle);
  background: var(--vr-color-chip-bg);
  color: var(--vr-color-text);
}

.chip[aria-checked='true'] .count {
  color: inherit;
}

.chip.empty[aria-checked='true'] {
  color: var(--vr-color-text-muted);
}

.chip:focus-visible {
  outline: 2px solid var(--vr-color-focus);
  outline-offset: -2px;
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
