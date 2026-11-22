<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ArrowRight, Search } from '../../../shared/ui/icons';
import { usePickerList } from '../../../shared/lib/usePickerList';
import { listWorktreeMarkdown, type MarkdownEntry } from '../../../shared/lib/markdownFiles';
import { isDesktop } from '../../../bridge';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import BackBar from '../../../shared/ui/BackBar.vue';
import UiInput from '../../../shared/ui/UiInput.vue';
import { useSessionDock } from '../../../entities/session';
import { openSearch } from '../model/panel';
import type { SearchAction, SearchKind, SearchRow, SearchScope } from '../model/rows';
import { useSearch } from '../model/useSearch';
import SearchEmpty from './SearchEmpty.vue';
import SearchNotes from './SearchNotes.vue';
import SearchResult from './SearchResult.vue';

type PaletteItem = SearchRow | { id: string; kind: 'all' };

const SESSION_LIMIT = 8;
const GROUP_LIMIT = 5;
const ALL: PaletteItem = { id: 'all', kind: 'all' };
const TESTIDS = { sessions: 'quick-switcher-item', mr: 'quick-switcher-item', tasks: 'quick-switcher-item', files: 'quick-switcher-file', actions: 'quick-switcher-action', channels: 'quick-switcher-message', messages: 'quick-switcher-transcript' } as const;

const emit = defineEmits<{ close: [] }>();

const { t, formatNumber } = useI18n();
const { activeWorktree } = useSessionDock();
const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');
const scope = shallowRef<SearchScope>('all');
const files = shallowRef<readonly MarkdownEntry[]>([]);
const actions = computed<readonly SearchAction[]>(() => (isDesktop ? [{ key: 'cleanup', label: t('cleanup.action') }] : []));

const search = useSearch({ query, scope, files, actions, browse: true, limit: (kind: SearchKind) => (kind === 'sessions' ? SESSION_LIMIT : GROUP_LIMIT) });
const { text, groups, rows } = search;

const showAll = computed(() => text.value.length > 0 && search.total.value > 0);
const results = computed<readonly PaletteItem[]>(() => (showAll.value ? [...rows.value, ALL] : rows.value));
const positions = computed(() => new Map(results.value.map((item, at) => [item.id, at])));

function clear() {
  query.value = '';
  input.value?.focus();
}

function openAll() {
  const typed = text.value;
  emit('close');
  openSearch(typed);
}

function choose(item: PaletteItem | undefined) {
  if (!item) return;
  if (item.kind === 'all') openAll();
  else if (search.choose(item)) emit('close');
}

function onPanelKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || !(event.ctrlKey || event.metaKey)) return;
  event.preventDefault();
  event.stopPropagation();
  if (text.value) openAll();
}

const { index, onKey } = usePickerList({ results, query, scope, idOf: (item) => item.id, list, choose, close: () => emit('close') });

async function loadFiles() {
  const worktree = activeWorktree.value;
  if (!worktree) return;
  try {
    files.value = await listWorktreeMarkdown(worktree.path);
  } catch {
    files.value = [];
  }
}

onMounted(() => {
  input.value?.focus();
  void loadFiles();
});
</script>

<template>
  <div class="overlay" data-testid="quick-switcher" @pointerdown.self="emit('close')">
    <div class="panel vr-sheet" role="dialog" :aria-label="t('search.title')" @keydown.capture="onPanelKey">
      <BackBar class="vr-phone-only" :title="t('search.title')" @back="emit('close')" />
      <div class="search">
        <Search :size="14" class="search-icon" />
        <UiInput
          ref="input"
          v-model="query"
          class="query"
          type="text"
          autocomplete="off"
          :placeholder="t('search.placeholder')"
          :aria-label="t('search.query')"
          data-testid="quick-switcher-input"
          @keydown="onKey"
        />
      </div>
      <SearchNotes
        :too-short="search.tooShort.value"
        :searching="search.searching.value"
        :progress="search.progress.value"
        :error="search.messageError.value"
        compact
        @retry="search.retryMessages"
      />
      <ul ref="list" v-auto-animate class="results vr-sheet-body" role="listbox">
        <template v-for="group in groups" :key="group.kind">
          <li class="group" role="presentation">
            <span>{{ search.groupLabel(group) }}</span>
            <span class="group-count">{{ formatNumber(group.total) }}</span>
          </li>
          <SearchResult
            v-for="row in group.rows"
            :key="row.id"
            :testid="TESTIDS[row.kind]"
            :row="row"
            :query="text"
            :current="positions.get(row.id) === index"
            :blocked="search.blockedReason(row)"
            :heading="row.kind === 'messages' ? search.hitTitle(row.conversation) : ''"
            :folded="search.foldedInto(row)"
            compact
            @hover="index = positions.get(row.id) ?? 0"
            @open="choose(row)"
            @open-conversation="choose(search.foldedInto(row) ?? row)"
          />
        </template>
        <li v-if="search.nothing.value" class="none" role="presentation">
          <p v-if="!text" class="hint" data-testid="quick-switcher-hint-empty">{{ t('search.hint') }}</p>
          <SearchEmpty v-else :query="text" :can-widen="scope !== 'all'" @widen="scope = 'all'" @clear="clear" />
        </li>
        <li v-if="showAll" class="all" role="option" :aria-selected="positions.get(ALL.id) === index" :data-current="positions.get(ALL.id) === index" data-testid="quick-switcher-all" @pointermove="index = positions.get(ALL.id) ?? 0" @click="choose(ALL)">
          <ArrowRight :size="14" class="all-icon" />
          <span>{{ t('search.allResults', { count: formatNumber(search.total.value) }) }}</span>
        </li>
      </ul>
      <footer class="footer vr-wide-only" data-testid="quick-switcher-hint">{{ t('files.switcher.hint') }}</footer>
    </div>
  </div>
</template>

<style scoped>
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
  container: search-list / inline-size;
  --all-row: calc(var(--vr-size-row) + var(--vr-space-1));
  position: relative;
  max-height: 50vh;
  margin: 0;
  padding: var(--vr-space-1) var(--vr-space-1) 0;
  overflow-y: auto;
  scroll-padding-bottom: var(--all-row);
  list-style: none;
}

.group {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) 10px var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
}

.group-count {
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

.all {
  position: sticky;
  bottom: 0;
  z-index: 1;
  background: var(--vr-color-surface-raised);
  box-shadow: 0 -1px 0 var(--vr-color-border);
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  min-height: var(--vr-size-row);
  box-sizing: border-box;
  margin-top: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-text);
  cursor: pointer;
  border-radius: var(--vr-radius-md);
}

.all[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.all-icon {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.none {
  display: flex;
}

.hint {
  flex: 1;
  margin: 0;
  padding: var(--vr-space-4) var(--vr-space-3);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  text-align: center;
}

.footer {
  padding: var(--vr-space-2) var(--vr-space-3);
  border-top: 1px solid var(--vr-color-border);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  text-align: center;
}
</style>
