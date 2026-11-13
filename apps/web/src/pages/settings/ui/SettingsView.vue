<script setup lang="ts">
import { computed, shallowRef, watch, type Component } from 'vue';
import { Bot, ExternalLink, FileJson, FileText, History, Keyboard, Palette, Plug, RefreshCw, Search, SlidersHorizontal, Smartphone, SquareTerminal, Waypoints } from '../../../shared/ui/icons';
import { VButton, VInputText, VMessage } from '../../../shared/ui/kit';
import { ConnectorsPanel } from '../../../features/manage-connectors';
import { RouterPanel } from '../../../features/edit-router';
import BackBar from '../../../shared/ui/BackBar.vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { withViewTransition } from '../../../shared/ui/motion/motion';
import { requestedSettingsCategory } from '../../../shared/model/nav';
import SettingRow from './SettingRow.vue';
import { KeymapPanel } from '../../../features/keymap-editor';
import { RemotePanel } from '../../../features/remote-access';
import { ProvidersPanel } from '../../../features/manage-providers';
import { PushToggle } from '../../../features/web-push';
import { isDesktop } from '../../../bridge';
import SettingsJsonEditor from '../../../shared/ui/SettingsJsonEditor.vue';
import { CATEGORIES, SETTINGS, SETTING_KEYS } from '../../../shared/config/settings/schema';
import { useSettings } from '../../../shared/config/settings/store';
import { describeFailure } from '../../../shared/lib/config-file/parse';

import { UiButton } from '../../../shared/ui/UiButton';
const { t } = useI18n();
const { parseError, issues, ioError, text, saveText, openExternal, canOpenExternal } = useSettings();

const query = shallowRef('');
type CategoryId = (typeof CATEGORIES)[number]['id'];
const category = shallowRef<CategoryId>('appearance');
const rawMode = shallowRef(false);
const picked = shallowRef(false);

const CATEGORY_ICONS: Record<CategoryId, Component> = { appearance: Palette, terminal: SquareTerminal, sessions: History, providers: Bot, router: Waypoints, files: FileText, connectors: Plug, remote: Smartphone, app: RefreshCw, keymap: Keyboard };

const normalizedQuery = computed(() => query.value.trim().toLowerCase());
const showDetail = computed(() => picked.value || rawMode.value || Boolean(normalizedQuery.value));
const headerHidden = computed(() => showDetail.value && !normalizedQuery.value);
const detailTitle = computed(() => {
  if (rawMode.value) return 'settings.json';
  if (normalizedQuery.value) return t('settings.search');
  return t(CATEGORIES.find((entry) => entry.id === category.value)?.label ?? 'settings.title');
});
watch(normalizedQuery, (value) => {
  if (value) rawMode.value = false;
});
const showKeymap = computed(() => !normalizedQuery.value && category.value === 'keymap');
const showRemote = computed(() => !normalizedQuery.value && category.value === 'remote');
function panelShown(id: CategoryId, keywords: string) {
  if (!normalizedQuery.value) return category.value === id;
  return normalizedQuery.value.split(/\s+/).every((word) => keywords.includes(word));
}
const showRouter = computed(() => panelShown('router', `${t('settings.category.router').toLowerCase()} ${t('settings.routerKeywords')}`));
const showProviders = computed(() => panelShown('providers', `${t('settings.category.providers').toLowerCase()} ${t('settings.providerKeywords')}`));
const showConnectors = computed(() => panelShown('connectors', t('settings.connectorKeywords')));
const groups = computed(() =>
  CATEGORIES.map((entry) => ({
    ...entry,
    keys: SETTING_KEYS.filter((key) => {
      const def = SETTINGS[key];
      if (def.category !== entry.id) return false;
      if (!normalizedQuery.value) return entry.id === category.value;
      return [t(def.label), key, t(def.description)].some((field) => field.toLowerCase().includes(normalizedQuery.value));
    }),
  })).filter((group) => group.keys.length),
);

function applyCategory(id: CategoryId) {
  category.value = id;
  query.value = '';
  rawMode.value = false;
  picked.value = true;
}

function backToCategories() {
  picked.value = false;
  query.value = '';
  rawMode.value = false;
}

function pickCategory(id: CategoryId) {
  void withViewTransition(() => applyCategory(id));
}

function toggleRaw() {
  void withViewTransition(() => {
    rawMode.value = !rawMode.value;
  });
}

watch(
  requestedSettingsCategory,
  (id) => {
    if (!id) return;
    requestedSettingsCategory.value = null;
    const entry = CATEGORIES.find((item) => item.id === id);
    if (entry) applyCategory(entry.id);
  },
  { immediate: true },
);

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') query.value = '';
}
</script>

<template>
  <section class="page" data-testid="settings-view" :data-detail="headerHidden" :aria-label="t('settings.title')">
    <header class="header vr-toolbar vr-master-only">
      <h1><SlidersHorizontal :size="18" />{{ t('settings.title') }}</h1>
      <label class="search">
        <Search :size="16" />
        <VInputText v-model="query" size="small" :placeholder="t('settings.search')" :aria-label="t('settings.search')" data-testid="settings-search" @keydown="onSearchKey" />
      </label>
      <VButton
        :label="rawMode ? t('settings.backToSettings') : t('settings.openJson')"
        size="small"
        severity="secondary"
        data-testid="settings-open-json"
        @click="toggleRaw"
      >
        <template #icon><FileJson :size="14" /></template>
      </VButton>
      <VButton
        :label="t('settings.openExternal')"
        size="small"
        severity="secondary"
        text
        :disabled="!canOpenExternal"
        :title="canOpenExternal ? t('settings.openExternalTitle') : t('settings.appOnly')"
        data-testid="settings-open-external"
        @click="openExternal"
      >
        <template #icon><ExternalLink :size="14" /></template>
      </VButton>
    </header>

    <div class="notices">
      <VMessage v-if="parseError" severity="error" size="small" data-testid="settings-parse-error">
        {{ t('settings.parseError') }} {{ describeFailure(parseError) }}
      </VMessage>
      <VMessage v-if="issues.length" severity="warn" size="small" data-testid="settings-issues">{{ t('settings.issues') }} {{ issues.join('; ') }}</VMessage>
      <VMessage v-if="ioError" severity="error" size="small" data-testid="settings-io-error">{{ ioError }}</VMessage>
    </div>

    <div class="body vr-master-detail" :data-detail="showDetail">
      <nav class="categories vr-master" :aria-label="t('settings.categoriesLabel')">
        <UiButton
          v-for="entry in CATEGORIES"
          :key="entry.id"
          class="category"
          :data-active="!rawMode && !normalizedQuery && entry.id === category"
          :data-testid="`settings-category-${entry.id}`"
          @click="pickCategory(entry.id)"
        >
          <component :is="CATEGORY_ICONS[entry.id]" :size="14" />
          {{ t(entry.label) }}
        </UiButton>
      </nav>

      <div class="content vr-detail vt-detail">
        <BackBar class="vr-phone-only" :title="detailTitle" @back="backToCategories" />
        <SettingsJsonEditor v-if="rawMode" name="settings.json" :text="text" :save="saveText" />
        <KeymapPanel v-else-if="showKeymap" />
        <div v-else class="list" :class="{ locked: parseError }" data-testid="settings-list">
          <section v-if="showConnectors" class="group" data-testid="settings-connectors">
            <ConnectorsPanel />
          </section>
          <section v-if="showProviders" class="group" data-testid="settings-providers">
            <h2 v-if="normalizedQuery">{{ t('settings.category.providers') }}</h2>
            <ProvidersPanel />
          </section>
          <section v-if="showRouter" class="group" data-testid="settings-router">
            <h2 v-if="normalizedQuery">{{ t('settings.category.router') }}</h2>
            <RouterPanel />
          </section>
          <section v-if="showRemote" class="group" data-testid="settings-remote">
            <RemotePanel />
            <PushToggle v-if="!isDesktop" />
          </section>
          <section v-for="group in groups" :key="group.id" v-auto-animate class="group">
            <h2 v-if="normalizedQuery">{{ t(group.label) }}</h2>
            <SettingRow v-for="key in group.keys" :key="key" :setting-key="key" />
          </section>
          <p v-if="!groups.length && !showConnectors && !showProviders && !showRouter" class="empty" data-testid="settings-empty">{{ t('settings.empty') }}</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  color: var(--vr-color-text);
}

.header {
  display: flex;
  align-items: center;
  padding: var(--vr-space-4) var(--vr-page-gutter);
  border-bottom: 1px solid var(--vr-color-border);
}

h1 {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0 var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
}

.search {
  position: relative;
  display: flex;
  flex: 1 1 12rem;
  align-items: center;
  max-width: 28rem;
  color: var(--vr-color-text-muted);
}

.search svg {
  position: absolute;
  left: var(--vr-space-2);
  pointer-events: none;
}

.search :deep(input) {
  width: 100%;
  padding-left: calc(var(--vr-space-2) * 3.5);
}

.notices {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.notices:not(:empty) {
  padding: var(--vr-space-3) var(--vr-page-gutter) 0;
}

.body {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(12rem, max-content) minmax(0, 1fr);
  min-height: 0;
}

.categories {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: var(--vr-space-4) var(--vr-space-3);
  border-right: 1px solid var(--vr-color-border);
}

.category {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-3);
  font: inherit;
  text-align: left;
  white-space: nowrap;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.category:hover {
  background: var(--vr-color-nav-hover);
}

.category[data-active='true'] {
  font-weight: 600;
  background: var(--vr-color-nav-active);
}

.content {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.list {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-2) var(--vr-page-gutter) var(--vr-space-6);
  overflow-y: auto;
}

.list.locked {
  opacity: 0.6;
  pointer-events: none;
}

.group h2 {
  margin: var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  text-transform: uppercase;
}

.empty {
  margin: var(--vr-space-6) 0;
  color: var(--vr-color-text-muted);
}
</style>
