<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, FileText, GitPullRequest, Search, Ticket, Zap } from '../../../shared/ui/icons';
import { subsequenceScore } from '../../../shared/lib/fuzzy';
import { usePickerList } from '../../../shared/lib/usePickerList';
import { PipelineIcon } from '../../../entities/merge-request';
import { t } from '../../../shared/lib/i18n/i18n';
import BackBar from '../../../shared/ui/BackBar.vue';
import type { QuickItem, WorkNode } from '../../../features/work-tree';
import { externalDot, HarnessIcon, StatusIcon } from '../../../entities/session';
import type { MarkdownEntry } from '../../../shared/lib/markdownFiles';

import UiInput from '../../../shared/ui/UiInput.vue';
type QuickAction = { key: string; label: string };
type Item = { kind: 'leaf'; key: string; entry: QuickItem } | { kind: 'file'; key: string; file: MarkdownEntry } | { kind: 'action'; key: string; action: QuickAction };
type Group = 'sessions' | 'mr' | 'issue' | 'files' | 'actions';

const props = withDefaults(defineProps<{ entries: readonly QuickItem[]; files?: readonly MarkdownEntry[]; fileScope?: string; actions?: readonly QuickAction[] }>(), { files: () => [], fileScope: '', actions: () => [] });
const emit = defineEmits<{ select: [node: WorkNode]; 'select-file': [path: string]; 'select-action': [key: string]; close: [] }>();

const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');

const ICONS = { mr: GitPullRequest, issue: Ticket } as const;
const iconOf = (kind: string) => (kind in ICONS ? ICONS[kind as keyof typeof ICONS] : undefined);

const GROUP_LABELS = {
  sessions: 'files.switcher.groups.sessions',
  mr: 'files.switcher.groups.mergeRequests',
  issue: 'files.switcher.groups.tasks',
  files: 'files.switcher.groups.files',
  actions: 'files.switcher.groups.actions',
} as const;

function groupOf(item: Item): Group {
  if (item.kind === 'file') return 'files';
  if (item.kind === 'action') return 'actions';
  const kind = item.entry.node.kind;
  if (kind === 'session' || kind === 'external') return 'sessions';
  return kind === 'mr' ? 'mr' : 'issue';
}

function groupItems(sorted: readonly Item[]): Item[] {
  const order = new Map<Group, number>();
  for (const item of sorted) {
    const group = groupOf(item);
    if (!order.has(group)) order.set(group, order.size);
  }
  return [...sorted].sort((a, b) => order.get(groupOf(a))! - order.get(groupOf(b))!);
}

const results = computed<Item[]>(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  const leaves: Item[] = props.entries.map((entry) => ({ kind: 'leaf', key: entry.key, entry }));
  if (!words.length) return leaves;
  const files: Item[] = props.files.map((file) => ({ kind: 'file', key: file.path, file }));
  const actions: Item[] = props.actions.map((action) => ({ kind: 'action', key: `action:${action.key}`, action }));
  const sorted = [...leaves, ...files, ...actions]
    .map((item, order) => {
      const text = item.kind === 'leaf' ? item.entry.search : item.kind === 'file' ? item.file.rel.toLowerCase() : item.action.label.toLowerCase();
      let total = 0;
      for (const word of words) {
        const score = subsequenceScore(word, text);
        if (score === null) return null;
        total += score;
      }
      return { item, order, total };
    })
    .filter((match): match is { item: Item; order: number; total: number } => match !== null)
    .sort((a, b) => b.total - a.total || a.order - b.order)
    .map((match) => match.item);
  return groupItems(sorted);
});

const groups = computed(() => {
  const out: { key: Group; items: { item: Item; position: number }[] }[] = [];
  results.value.forEach((item, position) => {
    const key = groupOf(item);
    const last = out[out.length - 1];
    if (last?.key === key) last.items.push({ item, position });
    else out.push({ key, items: [{ item, position }] });
  });
  return out;
});

function choose(item: Item | undefined) {
  if (!item) return;
  if (item.kind === 'leaf') emit('select', item.entry.node);
  else if (item.kind === 'file') emit('select-file', item.file.path);
  else emit('select-action', item.action.key);
}

const { index, onKey } = usePickerList({ results, query, list, choose, close: () => emit('close') });

onMounted(() => input.value?.focus());
</script>

<template>
  <div class="overlay" data-testid="quick-switcher" @pointerdown.self="emit('close')">
    <div class="panel vr-sheet" role="dialog" :aria-label="t('files.switcher.dialog')">
      <BackBar class="vr-phone-only" :title="t('files.switcher.dialog')" @back="emit('close')" />
      <div class="search">
        <Search :size="14" class="search-icon" />
        <UiInput
          ref="input"
          v-model="query"
          class="query"
          type="text"
          :placeholder="fileScope ? t('files.switcher.placeholderFiles', { scope: fileScope }) : t('workbench.quickPath')"
          :aria-label="t('files.switcher.search')"
          data-testid="quick-switcher-input"
          @keydown="onKey"
        />
      </div>
      <ul ref="list" v-auto-animate class="results vr-sheet-body" role="listbox">
        <template v-for="group in groups" :key="group.key">
          <li class="group" role="presentation">{{ t(GROUP_LABELS[group.key]) }}</li>
          <li
            v-for="{ item, position } in group.items"
            :key="item.key"
            class="item"
            role="option"
            :aria-selected="position === index"
            :data-current="position === index"
            :data-testid="item.kind === 'leaf' ? 'quick-switcher-item' : item.kind === 'file' ? 'quick-switcher-file' : 'quick-switcher-action'"
            :data-kind="item.kind === 'leaf' ? item.entry.node.kind : item.kind"
            :data-session-id="item.kind === 'leaf' ? item.entry.node.data?.session?.id : undefined"
            @pointermove="index = position"
            @click="choose(item)"
          >
            <template v-if="item.kind === 'leaf'">
              <StatusIcon v-if="item.entry.node.data?.session" :status="item.entry.node.data.session.status" :unseen="item.entry.node.data.session.unseen" />
              <template v-else-if="item.entry.node.data?.external">
                <StatusIcon :status="externalDot(item.entry.node.data.external)" :unseen="false" />
                <HarnessIcon :harness="item.entry.node.data.external.harness" />
              </template>
              <component :is="iconOf(item.entry.node.kind)" v-else-if="iconOf(item.entry.node.kind)" :size="14" class="file-icon" />
              <span class="text">
                <span class="session">{{ item.entry.node.label }}</span>
                <span v-if="item.entry.crumbs.length" class="sub">
                  <template v-for="crumb in item.entry.crumbs" :key="crumb">
                    <span class="path">{{ crumb }}</span>
                    <ChevronRight :size="11" />
                  </template>
                </span>
              </span>
              <PipelineIcon v-if="item.entry.node.data?.item && item.entry.node.data.item.pipeline !== 'none'" :status="item.entry.node.data.item.pipeline" :size="12" />
            </template>
            <template v-else-if="item.kind === 'action'">
              <Zap :size="14" class="file-icon" />
              <span class="text">
                <span class="session">{{ item.action.label }}</span>
              </span>
            </template>
            <template v-else>
              <FileText :size="14" class="file-icon" />
              <span class="text">
                <span class="session">{{ item.file.name }}</span>
                <span class="sub mono path">{{ item.file.rel }}</span>
              </span>
            </template>
          </li>
        </template>
        <li v-if="!results.length" class="none">{{ t('common.noResults') }}</li>
      </ul>
      <footer class="footer" data-testid="quick-switcher-hint">{{ t('files.switcher.hint') }}</footer>
    </div>
  </div>
</template>

<style scoped>
.qs-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.qs-enter-active .panel {
  transition: transform var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-enter-from,
.qs-leave-to {
  opacity: 0;
}

.qs-enter-from .panel {
  transform: scale(0.98);
}

.overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  justify-content: center;
  padding-top: 70px;
  background: var(--vr-overlay);
}

.panel {
  align-self: flex-start;
  width: min(620px, 90%);
  overflow: hidden;
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-xl);
  box-shadow: var(--vr-shadow-popover);
}

.search {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 46px;
  box-sizing: border-box;
  padding: 0 var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.search-icon {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.query {
  flex: 1;
  min-width: 0;
  box-sizing: border-box;
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.results {
  max-height: 50vh;
  margin: 0;
  padding: var(--vr-space-1);
  overflow-y: auto;
  list-style: none;
}

.group {
  padding: var(--vr-space-2) 10px var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 36px;
  box-sizing: border-box;
  padding: var(--vr-space-1) 10px;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.sub {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.path {
  overflow: hidden;
  text-overflow: ellipsis;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.session {
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text);
  text-overflow: ellipsis;
}

.file-icon {
  flex: none;
}

.none {
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}

.footer {
  padding: var(--vr-space-2) var(--vr-space-3);
  border-top: 1px solid var(--vr-color-border);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  text-align: center;
}
</style>
