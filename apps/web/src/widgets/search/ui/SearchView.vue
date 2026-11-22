<script setup lang="ts">
import { computed, nextTick, onMounted, useTemplateRef, watch } from 'vue';
import { Search } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import { VChipGroup } from '../../../shared/ui/kit';
import { usePickerList } from '../../../shared/lib/usePickerList';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { searchFocus, searchQuery, searchScope } from '../model/panel';
import { SCOPE_KINDS, type SearchScope } from '../model/rows';
import { useSearch } from '../model/useSearch';
import SearchEmpty from './SearchEmpty.vue';
import SearchNotes from './SearchNotes.vue';
import SearchResult from './SearchResult.vue';

const SCOPES: readonly SearchScope[] = ['all', ...SCOPE_KINDS];
const GROUP_CAP = 50;

const { t, formatNumber } = useI18n();
const search = useSearch({ query: searchQuery, scope: searchScope, limit: () => GROUP_CAP });
const { text, groups, rows } = search;
const input = useTemplateRef<{ focus: () => void }>('input');
const list = useTemplateRef<HTMLElement>('list');

const { index, onKey } = usePickerList({
  results: rows,
  query: searchQuery,
  scope: searchScope,
  idOf: (row) => row.id,
  list,
  choose: (row) => void search.choose(row),
  close: () => {
    searchQuery.value = '';
  },
});

const positions = computed(() => new Map(rows.value.map((row, at) => [row.id, at])));
const scopeOptions = computed(() => SCOPES.map((id) => ({ value: id, label: t(`search.scope.${id}`), count: search.chipCount(id) })));
const scope = computed({ get: () => searchScope.value, set: (value: string) => (searchScope.value = SCOPES.find((id) => id === value) ?? 'all') });

function clearQuery() {
  searchQuery.value = '';
  input.value?.focus();
}

watch(searchFocus, () => void nextTick(() => input.value?.focus()));
onMounted(() => input.value?.focus());
</script>

<template>
  <section class="search" data-testid="search-view">
    <header class="bar">
      <div class="field">
        <Search :size="14" class="field-icon" />
        <UiInput ref="input" v-model="searchQuery" class="query" type="text" autocomplete="off" :placeholder="t('search.placeholder')" :aria-label="t('search.query')" data-testid="search-input" @keydown="onKey" />
      </div>
      <VChipGroup v-model="scope" :options="scopeOptions" :label="t('search.scopeLabel')" test-id="search-scope" />
    </header>
    <SearchNotes :too-short="search.tooShort.value" :searching="search.searching.value" :progress="search.progress.value" :error="search.messageError.value" @retry="search.retryMessages" />
    <p v-if="!text" class="empty" data-testid="search-hint">{{ t('search.hint') }}</p>
    <SearchEmpty v-else-if="search.nothing.value" :query="text" :can-widen="searchScope !== 'all'" @widen="searchScope = 'all'" @clear="clearQuery" />
    <ul v-show="rows.length" ref="list" class="results" role="listbox" :aria-label="t('search.title')">
      <template v-for="group in groups" :key="group.kind">
        <li class="group" role="presentation" :data-testid="`search-group-${group.kind}`">
          <span class="group-name">{{ search.groupLabel(group) }}</span>
          <span class="group-count">{{ formatNumber(group.total) }}</span>
        </li>
        <SearchResult
          v-for="row in group.rows"
          :key="row.id"
          testid="search-result"
          :row="row"
          :query="text"
          :current="positions.get(row.id) === index"
          :blocked="search.blockedReason(row)"
          :heading="row.kind === 'messages' ? search.hitTitle(row.head) : ''"
          :folded="search.foldedInto(row)"
          @hover="index = positions.get(row.id) ?? 0"
          @open="search.choose(row)"
          @open-conversation="search.choose(search.foldedInto(row) ?? row)"
        />
        <li v-if="group.hidden > 0" role="presentation" class="more-row">
          <UiButton type="button" class="more" :data-testid="`search-show-all-${group.kind}`" @click="search.showAll(group.kind)">{{ t('search.showAll', { count: formatNumber(group.total) }) }}</UiButton>
        </li>
      </template>
    </ul>
  </section>
</template>

<style scoped>
.search {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-color-surface);
}

.bar {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-page-gutter);
  border-bottom: 1px solid var(--vr-color-border);
}

.field {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
  box-sizing: border-box;
  padding: 0 var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.field:focus-within {
  border-color: var(--vr-color-accent);
}

.field-icon {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.query {
  flex: 1;
  min-width: 0;
  box-sizing: border-box;
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.more:focus-visible {
  outline: 2px solid var(--vr-color-accent);
  outline-offset: 1px;
}

.empty {
  margin: 0;
  padding: var(--vr-space-4) var(--vr-page-gutter);
  color: var(--vr-color-text-muted);
}

.results {
  container: search-list / inline-size;
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-3);
  overflow-y: auto;
  list-style: none;
}

.group {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-3) var(--vr-space-1);
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
}

.group-count {
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

.more-row {
  padding: var(--vr-space-1) var(--vr-space-3);
}

.more {
  padding: var(--vr-space-1) var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  cursor: pointer;
  background: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
}
</style>
