<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from 'vue';
import { isMotionEnabled } from './motion';

type Fade = 'none' | 'left' | 'right' | 'both';

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
  event.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[index]?.focus({ preventScroll: true });
}

function onClick(event: MouseEvent, value: string) {
  model.value = value;
  if (event.currentTarget instanceof HTMLElement) revealChip(event.currentTarget, 'smooth');
}

const group = useTemplateRef<HTMLElement>('group');
const fade = ref<Fade>('none');

function updateFade() {
  const element = group.value;
  if (!element) return;
  const left = element.scrollLeft > 1;
  const right = element.scrollLeft < element.scrollWidth - element.clientWidth - 1;
  fade.value = left && right ? 'both' : left ? 'left' : right ? 'right' : 'none';
}

function revealChip(chip: HTMLElement, behavior: ScrollBehavior) {
  const element = group.value;
  if (!element) return;
  const inset = parseFloat(getComputedStyle(element).scrollPaddingLeft) || 0;
  const start = chip.getBoundingClientRect().left - element.getBoundingClientRect().left + element.scrollLeft;
  const end = start + chip.offsetWidth;
  const view = element.clientWidth;
  let left = element.scrollLeft;
  if (start - inset < left) left = start - inset;
  else if (end + inset > left + view) left = end + inset - view;
  left = Math.max(0, Math.min(left, element.scrollWidth - view));
  if (left !== element.scrollLeft) element.scrollTo({ left, behavior });
}

function revealChecked(behavior: ScrollBehavior) {
  const chip = group.value?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]');
  if (chip) revealChip(chip, behavior);
}

let resizeObserver: ResizeObserver | null = null;

onMounted(async () => {
  resizeObserver = new ResizeObserver(updateFade);
  if (group.value) resizeObserver.observe(group.value);
  await nextTick();
  revealChecked('auto');
  updateFade();
});

onBeforeUnmount(() => resizeObserver?.disconnect());

watch(model, () => revealChecked(isMotionEnabled() ? 'smooth' : 'auto'), { flush: 'post' });
watch(() => props.options, updateFade, { flush: 'post', deep: true });

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
  <div ref="group" class="chip-group" role="radiogroup" :aria-label="label" :data-fade="fade" @keydown="onKeydown" @wheel="onWheel" @scroll.passive="updateFade">
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
  --chip-fade: var(--vr-space-6);
  display: flex;
  flex-wrap: nowrap;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow-x: auto;
  scroll-padding-inline: var(--chip-fade);
  scroll-snap-type: x proximity;
  scrollbar-width: none;
}

.chip-group:not([data-fade='none']) {
  --fade-start: 0px;
  --fade-end: 0px;
  mask-image: linear-gradient(to right, transparent, var(--vr-color-text) var(--fade-start), var(--vr-color-text) calc(100% - var(--fade-end)), transparent);
}

.chip-group[data-fade='left'],
.chip-group[data-fade='both'] {
  --fade-start: var(--chip-fade);
}

.chip-group[data-fade='right'],
.chip-group[data-fade='both'] {
  --fade-end: var(--chip-fade);
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
