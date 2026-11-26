<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { VButton, VMessage, VPopover, vTooltip } from '../../../shared/ui/kit';
import { FileText } from '../../../shared/ui/icons';
import { describeError } from '../../../shared/api/connectors/api';
import { subsequenceScore } from '../../../shared/lib/fuzzy';
import { usePickerList } from '../../../shared/lib/usePickerList';
import { t } from '../../../shared/lib/i18n/i18n';
import { listWorktreeMarkdown, type MarkdownEntry } from '../../../shared/lib/markdownFiles';

import UiInput from '../../../shared/ui/UiInput.vue';
const props = defineProps<{ root: string }>();
const emit = defineEmits<{ open: [path: string] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');
const files = shallowRef<MarkdownEntry[]>([]);
const loaded = shallowRef(false);
const failure = shallowRef<string | null>(null);

const results = computed(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return files.value;
  return files.value
    .map((file, order) => {
      const text = file.rel.toLowerCase();
      let total = 0;
      for (const word of words) {
        const score = subsequenceScore(word, text);
        if (score === null) return null;
        total += score;
      }
      return { file, order, total };
    })
    .filter((item): item is { file: MarkdownEntry; order: number; total: number } => item !== null)
    .sort((a, b) => b.total - a.total || a.order - b.order)
    .map((item) => item.file);
});

function close() {
  popover.value?.hide();
}

function choose(file: MarkdownEntry | undefined) {
  if (!file) return;
  close();
  emit('open', file.path);
}

const { index, onKey } = usePickerList({ results, query, list, choose, close });

async function load() {
  loaded.value = false;
  failure.value = null;
  try {
    files.value = await listWorktreeMarkdown(props.root);
  } catch (e) {
    files.value = [];
    failure.value = describeError(e);
  }
  loaded.value = true;
}

function toggle(event: Event) {
  popover.value?.toggle(event);
}

function onShow() {
  query.value = '';
  index.value = 0;
  void load();
  void nextTick(() => input.value?.focus());
}
</script>

<template>
  <VButton v-tooltip="t('files.picker.open')" class="trigger" text severity="secondary" size="small" :aria-label="t('files.picker.open')" aria-haspopup="dialog" data-testid="worktree-markdown" @click="toggle">
    <template #icon><FileText :size="14" /></template>
  </VButton>
  <VPopover ref="popover" @show="onShow">
    <div class="picker" role="dialog" :aria-label="t('files.picker.open')" data-testid="markdown-picker">
      <UiInput ref="input" v-model="query" class="query" type="text" :placeholder="t('files.picker.filter')" :aria-label="t('files.picker.filter')" autocomplete="off" data-testid="markdown-picker-input" @keydown="onKey" />
      <VMessage v-if="failure" class="failure" severity="error" size="small">{{ failure }}</VMessage>
      <ul ref="list" class="results" role="listbox">
        <li
          v-for="(file, position) in results"
          v-tooltip="file.rel"
          :key="file.path"
          class="item"
          role="option"
          :aria-selected="position === index"
          :data-current="position === index"
          data-testid="markdown-picker-item"
          @pointermove="index = position"
          @click="choose(file)"
        >
          <span class="name">{{ file.name }}</span>
          <span class="rel">{{ file.rel }}</span>
        </li>
        <li v-if="!loaded" class="note">{{ t('common.loading') }}</li>
        <li v-else-if="!results.length && !failure" class="note" data-testid="markdown-picker-empty">{{ files.length ? t('common.noResults') : t('files.picker.none') }}</li>
      </ul>
    </div>
  </VPopover>
</template>

<style scoped>
.trigger {
  flex: none;
  width: 22px;
  height: 22px;
}

.picker {
  display: flex;
  flex-direction: column;
  width: 340px;
}

.query {
  box-sizing: border-box;
  width: 100%;
}

.results {
  max-height: 320px;
  margin: 0;
  padding: var(--vr-space-1);
  overflow-x: hidden;
  overflow-y: auto;
  list-style: none;
}

.item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--vr-space-1) var(--vr-space-2);
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.name {
  font-weight: 600;
  color: var(--vr-color-text);
  overflow-wrap: anywhere;
}

.rel {
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.note {
  padding: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  list-style: none;
}

.failure {
  margin: var(--vr-space-1);
}
</style>
