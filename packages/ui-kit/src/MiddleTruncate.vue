<script lang="ts">
type Job = { read: () => void; write: () => void };

const jobs = new WeakMap<Element, Job>();
const live = new Set<Job>();
const ELLIPSIS = '…';
const TAIL_UNITS = 8;
const MIN_HEAD_UNITS = 6;

let observer: ResizeObserver | undefined;
let context: CanvasRenderingContext2D | null | undefined;

function flush(batch: Iterable<Job>) {
  const list = [...batch];
  for (const job of list) job.read();
  for (const job of list) job.write();
}

function shared(): ResizeObserver | undefined {
  if (observer || typeof ResizeObserver === 'undefined') return observer;
  observer = new ResizeObserver((entries) => flush(entries.flatMap((entry) => jobs.get(entry.target) ?? [])));
  void document.fonts?.ready.then(() => flush(live));
  return observer;
}

function measurer(font: string): (text: string) => number {
  context ??= document.createElement('canvas').getContext('2d');
  if (!context) return (text) => text.length;
  context.font = font;
  const ctx = context;
  return (text) => ctx.measureText(text).width;
}

function weight(char: string): number {
  const code = char.codePointAt(0) ?? 0;
  return (code >= 0x2e80 && code <= 0xa4cf) || (code >= 0xac00 && code <= 0xd7a3) || (code >= 0xf900 && code <= 0xfaff) || (code >= 0xff00 && code <= 0xff60) || (code >= 0x20000 && code <= 0x3fffd) ? 2 : 1;
}

function cut(text: string, available: number, width: (text: string) => number): string | undefined {
  const chars = Array.from(text.replace(/\s+/g, ' '));
  let tailAt = chars.length;
  let units = 0;
  while (tailAt > 0 && units + weight(chars[tailAt - 1] ?? '') <= TAIL_UNITS) units += weight(chars[--tailAt] ?? '');
  const total = chars.reduce((sum, char) => sum + weight(char), 0);
  if (total <= TAIL_UNITS + MIN_HEAD_UNITS) return undefined;
  const ellipsis = width(ELLIPSIS);
  for (; tailAt < chars.length; tailAt++) {
    const tail = chars.slice(tailAt).join('');
    const room = available - ellipsis - width(tail);
    if (room < 0) continue;
    let low = 0;
    let high = tailAt;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if (width(chars.slice(0, middle).join('')) <= room) low = middle;
      else high = middle - 1;
    }
    return `${chars.slice(0, low).join('')}${ELLIPSIS}${tail}`;
  }
  return undefined;
}
</script>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, useTemplateRef, watch } from 'vue';

const props = defineProps<{ text: string }>();

const root = useTemplateRef<HTMLElement>('root');
const full = useTemplateRef<HTMLElement>('full');

let next: string | undefined;

const job: Job = {
  read() {
    const element = root.value;
    const inner = full.value;
    if (!element || !inner) return;
    const available = element.clientWidth - 1;
    if (available <= 0 || inner.offsetWidth <= available + 1) {
      next = undefined;
      return;
    }
    next = cut(props.text, available, measurer(getComputedStyle(element).font));
  },
  write() {
    if (next === undefined) root.value?.removeAttribute('data-shown');
    else root.value?.setAttribute('data-shown', next);
  },
};

onMounted(() => {
  if (!root.value) return;
  jobs.set(root.value, job);
  live.add(job);
  shared()?.observe(root.value);
});

onBeforeUnmount(() => {
  if (root.value) {
    observer?.unobserve(root.value);
    jobs.delete(root.value);
  }
  live.delete(job);
});

watch(
  () => props.text,
  () => flush([job]),
  { flush: 'post' },
);
</script>

<template>
  <span ref="root" class="middle-truncate"><span ref="full" class="full">{{ text }}</span></span>
</template>

<style scoped>
.middle-truncate {
  position: relative;
  display: block;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.middle-truncate[data-shown] {
  text-overflow: clip;
}

.middle-truncate[data-shown] .full {
  color: transparent;
}

.middle-truncate[data-shown]::before {
  content: attr(data-shown) / '';
  position: absolute;
  top: 0;
  left: 0;
  white-space: pre;
}
</style>
