<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue';
import type { DiffFile, Discussion } from '../../../shared/api/connectors/api';
import type { PromptQuote } from '../../../entities/session';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { escapeHtml, highlightLines } from '../../../shared/lib/content/highlight';
import { t } from '../../../shared/lib/i18n/i18n';
import { parsePatch, sideTexts, splitRows, type PatchLine, type SplitRow } from '../lib/patch';
import ThreadView from './ThreadView.vue';

const props = defineProps<{
  file: DiffFile;
  discussions: Discussion[];
  mode: 'unified' | 'split';
  expanded: boolean;
  wrap: boolean;
  flashId: string | null;
  connector?: string;
  pickable?: boolean;
  picked?: { from: number; to: number } | null;
}>();
const emit = defineEmits<{ expand: []; pick: [index: number, extend: boolean]; quote: [quote: PromptQuote]; clear: [] }>();

type Model = { lines: PatchLine[]; rows: SplitRow[]; html: string[]; threads: Map<number, Discussion[]>; orphans: Discussion[] };

const ROW_LIMIT = 2500;
const MIN_SPLIT_WIDTH = 720;
const EMPTY: Discussion[] = [];

function safeHighlight(code: string, path: string): string[] {
  if (!code) return [];
  try {
    return highlightLines(code, path);
  } catch {
    return code.split('\n').map(escapeHtml);
  }
}

const model = computed<Model>(() => {
  const file = props.file;
  const lines = parsePatch(file.patch);
  const { oldCode, newCode, oldIndex, newIndex } = sideTexts(lines);
  const oldHtml = file.status === 'added' ? [] : safeHighlight(oldCode, file.old_path);
  const newHtml = file.status === 'deleted' ? [] : safeHighlight(newCode, file.new_path);
  const html = lines.map((line) => {
    if (line.kind === 'del') return oldHtml[oldIndex.get(line.index) ?? -1] ?? escapeHtml(line.text);
    if (line.kind === 'add' || line.kind === 'ctx') return newHtml[newIndex.get(line.index) ?? -1] ?? escapeHtml(line.text);
    return escapeHtml(line.text);
  });
  const byNew = new Map<number, number>();
  const byOld = new Map<number, number>();
  for (const line of lines) {
    if (line.newLine != null) byNew.set(line.newLine, line.index);
    if (line.oldLine != null) byOld.set(line.oldLine, line.index);
  }
  const threads = new Map<number, Discussion[]>();
  const orphans: Discussion[] = [];
  for (const d of props.discussions) {
    const p = d.position;
    const at = (p?.new_line != null ? byNew.get(p.new_line) : undefined) ?? (p?.old_line != null ? byOld.get(p.old_line) : undefined);
    if (at === undefined) {
      orphans.push(d);
      continue;
    }
    const list = threads.get(at);
    if (list) list.push(d);
    else threads.set(at, [d]);
  }
  return { lines, rows: splitRows(lines), html, threads, orphans };
});

const bodyEl = useTemplateRef<HTMLElement>('body');
const bodyWidth = shallowRef(0);
let resizes: ResizeObserver | undefined;

onMounted(() => {
  const el = bodyEl.value;
  if (!el) return;
  bodyWidth.value = el.clientWidth;
  resizes = new ResizeObserver(([entry]) => {
    if (entry) bodyWidth.value = entry.contentRect.width;
  });
  resizes.observe(el);
});

onBeforeUnmount(() => resizes?.disconnect());

const shownMode = computed(() => (props.mode === 'split' && bodyWidth.value > 0 && bodyWidth.value < MIN_SPLIT_WIDTH ? 'unified' : props.mode));

const syncedLeft: Record<string, number> = {};

function syncSide(event: Event) {
  const cell = event.target as HTMLElement;
  const side = cell.dataset.side;
  if (!side) return;
  const left = cell.scrollLeft;
  if (Math.abs(left - (syncedLeft[side] ?? 0)) < 1) return;
  syncedLeft[side] = left;
  for (const other of (event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(`.code[data-side="${side}"]`)) {
    if (other !== cell) other.scrollLeft = left;
  }
}

const shownLines = computed(() => (props.expanded ? model.value.lines : model.value.lines.slice(0, ROW_LIMIT)));
const shownRows = computed(() => (props.expanded ? model.value.rows : model.value.rows.slice(0, ROW_LIMIT)));
const truncated = computed(() => !props.expanded && (shownMode.value === 'unified' ? model.value.lines.length : model.value.rows.length) > ROW_LIMIT);

function rowThreads(row: SplitRow): Discussion[] {
  const threads = model.value.threads;
  if (!threads.size) return EMPTY;
  const left = row.left ? (threads.get(row.left.index) ?? EMPTY) : EMPTY;
  if (row.right === row.left) return left;
  const right = row.right ? (threads.get(row.right.index) ?? EMPTY) : EMPTY;
  return left.length && right.length ? [...left, ...right] : left.length ? left : right;
}

const isPicked = (line: PatchLine | null) => !!line && !!props.picked && line.index >= props.picked.from && line.index <= props.picked.to;

function pick(line: PatchLine | null, event: MouseEvent) {
  if (props.pickable && line) emit('pick', line.index, event.shiftKey);
}

const quoted = computed(() => {
  const at = props.picked;
  if (!at) return null;
  const lines = model.value.lines.filter((l) => l.index >= at.from && l.index <= at.to && l.kind !== 'hunk' && l.kind !== 'meta');
  if (!lines.length) return null;
  const numbers = lines.map((l) => l.newLine ?? l.oldLine ?? 0);
  const first = Math.min(...numbers);
  const last = Math.max(...numbers);
  const path = props.file.status === 'deleted' ? props.file.old_path : props.file.new_path;
  const label = `${path}:${first === last ? first : `${first}-${last}`}`;
  const body = lines.map((l) => `${l.kind === 'add' ? '+' : l.kind === 'del' ? '-' : ' '}${l.text}`).join('\n');
  return { label, text: `${label}\n\`\`\`diff\n${body}\n\`\`\`\n` };
});

const endsPick = (line: PatchLine | null | undefined) => !!line && props.picked?.to === line.index;

function addToPrompt() {
  if (!quoted.value) return;
  emit('quote', quoted.value);
  emit('clear');
}

const codeOf = (line: PatchLine | null) => (line ? model.value.html[line.index] || ' ' : ' ');
</script>

<template>
  <div ref="body" class="body" :data-mode="shownMode" :data-wrap="wrap">
    <div v-for="d in model.orphans" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
      <ThreadView :discussion="d" :connector="connector" :quotable="pickable" show-position @quote="emit('quote', $event)" />
    </div>

    <div v-if="shownMode === 'unified'" class="lines">
      <template v-for="line in shownLines" :key="line.index">
        <div v-if="line.kind === 'hunk'" class="hunk">{{ line.text }}</div>
        <div v-else-if="line.kind === 'meta'" class="hunk meta">{{ line.text }}</div>
        <div v-else class="line" :data-kind="line.kind" :data-picked="isPicked(line)">
          <span class="num" :data-pickable="pickable" @click="pick(line, $event)">{{ line.oldLine ?? '' }}</span>
          <span class="num" :data-pickable="pickable" @click="pick(line, $event)">{{ line.newLine ?? '' }}</span>
          <span class="sign">{{ line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : '' }}</span>
          <span class="code" v-html="codeOf(line)" />
        </div>
        <div v-if="quoted && endsPick(line)" class="pick-bar" data-testid="diff-pick-bar">
          <span class="pick-label">{{ quoted.label }}</span>
          <span class="spacer" />
          <VButton v-tooltip="t('forge.diff.addToPromptTitle')" class="pick-action" size="small" :label="t('forge.diff.addToPrompt')" data-testid="diff-add-to-prompt" @click="addToPrompt" />
          <VButton class="pick-action" size="small" severity="secondary" text :label="t('forge.diff.clearPick')" data-testid="diff-clear-pick" @click="emit('clear')" />
        </div>
        <div v-for="d in model.threads.get(line.index)" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
          <ThreadView :discussion="d" :connector="connector" :quotable="pickable" @quote="emit('quote', $event)" />
        </div>
      </template>
    </div>

    <div v-else class="lines split" @scroll.capture="syncSide">
      <template v-for="(row, r) in shownRows" :key="r">
        <div v-if="row.full" class="hunk" :class="{ meta: row.full.kind === 'meta' }">{{ row.full.text }}</div>
        <div v-else class="line split-line">
          <span class="num" :data-kind="row.left?.kind" :data-picked="isPicked(row.left)" :data-pickable="pickable" @click="pick(row.left, $event)">{{ row.left?.oldLine ?? '' }}</span>
          <span class="code" data-side="old" :data-kind="row.left?.kind" :data-picked="isPicked(row.left)" v-html="codeOf(row.left)" />
          <span class="num" :data-kind="row.right?.kind" :data-picked="isPicked(row.right)" :data-pickable="pickable" @click="pick(row.right, $event)">{{ row.right?.newLine ?? '' }}</span>
          <span class="code" data-side="new" :data-kind="row.right?.kind" :data-picked="isPicked(row.right)" v-html="codeOf(row.right)" />
        </div>
        <div v-if="quoted && (endsPick(row.right) || endsPick(row.left))" class="pick-bar" data-testid="diff-pick-bar">
          <span class="pick-label">{{ quoted.label }}</span>
          <span class="spacer" />
          <VButton v-tooltip="t('forge.diff.addToPromptTitle')" class="pick-action" size="small" :label="t('forge.diff.addToPrompt')" data-testid="diff-add-to-prompt" @click="addToPrompt" />
          <VButton class="pick-action" size="small" severity="secondary" text :label="t('forge.diff.clearPick')" data-testid="diff-clear-pick" @click="emit('clear')" />
        </div>
        <div v-for="d in rowThreads(row)" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
          <ThreadView :discussion="d" :connector="connector" :quotable="pickable" @quote="emit('quote', $event)" />
        </div>
      </template>
    </div>

    <VButton v-if="truncated" class="more" size="small" severity="secondary" text :label="t('forge.diff.showAll')" data-testid="diff-show-all" @click="emit('expand')" />
  </div>
</template>

<style scoped>
.body {
  container-type: inline-size;
  overflow-x: auto;
}

.lines {
  min-width: max-content;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 1.75;
  background: var(--vr-color-surface-sunken);
}

.lines.split {
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 44px minmax(0, 1fr);
  min-width: 0;
}

.lines.split > .hunk,
.lines.split > .thread-row {
  grid-column: 1 / -1;
}

.lines.split > .hunk {
  overflow: hidden;
  text-overflow: ellipsis;
}

.lines.split .code {
  overflow-x: auto;
  scrollbar-width: none;
}

.hunk {
  padding: 0 var(--vr-space-3);
  color: var(--vr-status-running);
  white-space: pre;
  background: var(--vr-tint-running);
}

.hunk.meta {
  font-style: italic;
  background: none;
}

.line {
  display: grid;
  grid-template-columns: 44px 44px 18px 1fr;
}

.line[data-kind='add'] {
  background: var(--vr-diff-add);
}

.line[data-kind='del'] {
  background: var(--vr-diff-del);
}

.num {
  padding: 0 var(--vr-space-2);
  color: var(--vr-term-dim);
  text-align: right;
  user-select: none;
}

.num[data-pickable='true'] {
  cursor: pointer;
}

.num[data-pickable='true']:hover {
  color: var(--vr-color-text);
}

.sign {
  color: var(--vr-term-dim);
  text-align: center;
  user-select: none;
}

.line[data-kind='add'] .sign {
  color: var(--vr-color-success);
}

.line[data-kind='del'] .sign {
  color: var(--vr-color-danger);
}

.code {
  white-space: pre;
}

.line[data-picked='true'],
.split-line .num[data-picked='true'],
.split-line .code[data-picked='true'] {
  background: var(--vr-color-nav-active);
}

.split-line {
  grid-column: 1 / -1;
  grid-template-columns: subgrid;
}

.split-line [data-kind='add'] {
  background: var(--vr-diff-add);
}

.split-line [data-kind='del'] {
  background: var(--vr-diff-del);
}

.split-line .code + .num {
  border-left: 1px solid var(--vr-color-border);
}

.body[data-wrap='true'] .lines {
  min-width: 0;
}

.body[data-wrap='true'] .lines:not(.split) .line {
  grid-template-columns: 44px 44px 18px minmax(0, 1fr);
}

.body[data-wrap='true'] .code,
.body[data-wrap='true'] .hunk {
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.pick-bar {
  position: sticky;
  left: 0;
  box-sizing: border-box;
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100cqw;
  padding: 6px var(--vr-space-3) 6px 106px;
  font-family: var(--vr-font-family);
  font-size: var(--vr-font-size-sm);
  white-space: normal;
  background: var(--vr-color-surface);
  border-top: 1px solid var(--vr-color-border);
  border-bottom: 1px solid var(--vr-color-border);
}

.pick-action {
  flex: none;
  white-space: nowrap;
}

.lines.split > .pick-bar {
  padding-left: 56px;
}

.pick-label {
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.spacer {
  flex: 1;
}

.thread-row {
  position: sticky;
  left: 0;
  box-sizing: border-box;
  width: 100cqw;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-family: var(--vr-font-family);
  white-space: normal;
}

.thread-row[data-flash='true'] {
  background: var(--vr-color-nav-active);
}

.more {
  width: 100%;
  border-top: 1px solid var(--vr-color-border);
}
</style>
