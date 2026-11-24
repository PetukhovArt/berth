<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Component } from 'vue';
import { Activity, BookOpen, LayoutPanelLeft, LoaderCircle, Moon, PanelLeftClose, PanelLeftOpen, Search, Settings, Sun, Ticket } from '../../../shared/ui/icons';
import { useConnectors } from '../../../shared/api/connectors/api';
import { useForgeInbox } from '../../../entities/merge-request';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { navigate, view } from '../../../shared/model/nav';
import { requestSidebarSearch, sidebarCollapsed, sidebarSearchRequests, toggleSidebar } from '../../../shared/model/sidebar';
import ProjectTree from './ProjectTree.vue';
import { treeTab, useWorkTree } from '../../../features/work-tree';
import { toggleQuickOpen } from '../../../shared/model/dock';
import { tabIcons } from '../model/tabIcons';
import { useBerth, useSessionDock } from '../../../entities/session';
import { cleanupCount, openCleanup, removing as cleanupRemoving } from '../../../features/cleanup';
import { isDesktop } from '../../../bridge';

import { UiButton } from '../../../shared/ui/UiButton';
defineProps<{ dark: boolean }>();
const emit = defineEmits<{ theme: [] }>();

const { t, locale, setLocale } = useI18n();
const berth = useBerth();
const sessions = useSessionDock();
const sidebarEl = ref<HTMLElement | null>(null);

const MIN_WIDTH = 180;
const DEFAULT_WIDTH = 300;
const RAIL_WIDTH = 48;
const KEY_STEP = 16;
const WIDTH_KEY = 'berth.sidebar.tree.width';
const EXPLORER_WIDTH_KEY = 'berth.agents.a.width';

const viewportWidth = ref(window.innerWidth);
const maxWidth = computed(() => Math.max(MIN_WIDTH, Math.floor(viewportWidth.value / 2)));

function onWindowResize() {
  viewportWidth.value = window.innerWidth;
}

function clampWidth(value: number) {
  return Math.min(maxWidth.value, Math.max(MIN_WIDTH, Math.round(value)));
}

function readWidth() {
  const stored = Number(localStorage.getItem(WIDTH_KEY) ?? localStorage.getItem(EXPLORER_WIDTH_KEY));
  return Number.isFinite(stored) && stored > 0 ? clampWidth(stored) : DEFAULT_WIDTH;
}

const width = ref(readWidth());
const collapsed = sidebarCollapsed;
const dragging = ref(false);
const currentWidth = computed(() => (collapsed.value ? RAIL_WIDTH : Math.min(width.value, maxWidth.value)));
const collapseLabel = computed(() => (collapsed.value ? t('shell.expand') : t('shell.collapse')));
const nextLocale = computed(() => (locale.value === 'en' ? 'ru' : 'en'));

watch(width, (value) => localStorage.setItem(WIDTH_KEY, String(value)));

watch(sidebarSearchRequests, async () => {
  if (!collapsed.value) return;
  collapsed.value = false;
  await nextTick();
  requestSidebarSearch();
});

function shortcutTitle(label: string, actionId: string) {
  const shortcut = shortcutLabel(actionId);
  return shortcut ? `${label} (${shortcut})` : label;
}

function startResize(event: PointerEvent) {
  if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
  event.preventDefault();
  event.currentTarget.setPointerCapture(event.pointerId);
  dragging.value = true;
}

function moveResize(event: PointerEvent) {
  if (!dragging.value || !sidebarEl.value) return;
  const edge = event.clientX - sidebarEl.value.getBoundingClientRect().left;
  collapsed.value = edge < MIN_WIDTH;
  if (!collapsed.value) width.value = clampWidth(edge);
}

function endResize() {
  dragging.value = false;
}

function onHandleKey(event: KeyboardEvent) {
  const step = event.key === 'ArrowLeft' ? -KEY_STEP : event.key === 'ArrowRight' ? KEY_STEP : 0;
  if (!step) return;
  event.preventDefault();
  if (collapsed.value) {
    collapsed.value = step < 0;
    return;
  }
  const next = currentWidth.value + step;
  if (next < MIN_WIDTH) collapsed.value = true;
  else width.value = clampWidth(next);
}

const { connectors, list: listConnectors } = useConnectors();
const { startPolling } = useForgeInbox();
const { tabs: workTabs, activeTab } = useWorkTree();

const hasForge = computed(() => connectors.value.some((c) => c.kind === 'github' || c.kind === 'gitlab'));
const hasJira = computed(() => connectors.value.some((c) => c.kind === 'jira'));
const hasConfluence = computed(() => connectors.value.some((c) => c.kind === 'confluence'));

type NavItem = { kind: 'agents' | 'jira' | 'confluence'; label: string; icon: Component; testId: string };

const navItems = computed<NavItem[]>(() => {
  const items: NavItem[] = [
    { kind: 'agents', label: t('shell.nav.agents'), icon: LayoutPanelLeft, testId: 'open-agents' },
  ];
  if (hasJira.value) items.push({ kind: 'jira', label: t('shell.nav.jira'), icon: Ticket, testId: 'nav-jira' });
  if (hasConfluence.value) items.push({ kind: 'confluence', label: t('shell.nav.confluence'), icon: BookOpen, testId: 'nav-confluence' });
  return items;
});

const shownNav = computed(() => (collapsed.value ? navItems.value.filter((item) => item.kind !== 'agents') : navItems.value));
const showNav = computed(() => (collapsed.value ? shownNav.value.length > 0 : navItems.value.length > 1));

function openRailTab(id: (typeof workTabs.value)[number]['id']) {
  treeTab.value = id;
  collapsed.value = false;
  if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
}

function toggleSettings() {
  void navigate({ kind: view.value.kind === 'settings' ? 'agents' : 'settings' });
}

let stopPolling: (() => void) | undefined;
watch(hasForge, (on) => {
  stopPolling?.();
  stopPolling = on ? startPolling() : undefined;
}, { immediate: true });

onMounted(() => {
  void listConnectors();
  window.addEventListener('resize', onWindowResize);
});
onUnmounted(() => {
  stopPolling?.();
  window.removeEventListener('resize', onWindowResize);
});
</script>

<template>
  <nav
    ref="sidebarEl"
    class="sidebar"
    :class="{ collapsed, dragging }"
    :style="{ width: `${currentWidth}px` }"
    :aria-label="t('shell.nav.label')"
    data-testid="sidebar"
    :data-collapsed="collapsed"
  >
    <slot name="head" />
    <div v-if="collapsed" class="top">
      <UiButton class="item rail-item" :title="t('agents.layouts.quickOpenTitle')" :aria-label="t('agents.layouts.quickOpen')" data-testid="sidebar-search-open" @click="toggleQuickOpen">
        <Search :size="16" />
      </UiButton>
      <span class="rail-divider" role="separator" />
      <UiButton
        v-for="tab in workTabs"
        :key="tab.id"
        class="item rail-item"
        :data-active="view.kind === 'agents' && activeTab === tab.id"
        :title="tab.count ? `${tab.label} · ${tab.title}` : tab.label"
        :aria-label="tab.label"
        :data-testid="`rail-tab-${tab.id}`"
        @click="openRailTab(tab.id)"
      >
        <component :is="tabIcons[tab.id]" :size="16" />
        <span v-if="tab.count" class="count" :data-tone="tab.tone" :data-testid="`rail-tab-count-${tab.id}`">{{ tab.count }}</span>
      </UiButton>
    </div>

    <div v-if="showNav" class="primary">
      <UiButton
        v-for="item in shownNav"
        :key="item.kind"
        class="item vt-nav"
        :class="{ 'rail-item': collapsed }"
        :data-active="view.kind === item.kind"
        :title="collapsed ? item.label : undefined"
        :aria-label="collapsed ? item.label : undefined"
        :data-testid="item.testId"
        @click="navigate({ kind: item.kind })"
      >
        <component :is="item.icon" :size="16" />
        <span v-if="!collapsed">{{ item.label }}</span>
      </UiButton>
    </div>

    <div v-if="!collapsed" class="sections">
      <ProjectTree />
    </div>

    <footer>
      <UiButton
        class="icon"
        :data-active="view.kind === 'settings'"
        :title="shortcutTitle(t('shell.settings'), 'nav.settings')"
        :aria-label="t('shell.settings')"
        data-testid="open-settings"
        @click="toggleSettings"
      >
        <Settings :size="16" />
      </UiButton>
      <UiButton
        class="icon"
        :title="shortcutTitle(collapseLabel, 'sidebar.toggle')"
        :aria-label="collapseLabel"
        :aria-expanded="!collapsed"
        data-testid="sidebar-collapse"
        @click="toggleSidebar"
      >
        <PanelLeftOpen v-if="collapsed" :size="16" />
        <PanelLeftClose v-else :size="16" />
      </UiButton>
      <UiButton v-if="berth" class="icon" :title="t('agents.metrics.open')" :aria-label="t('agents.metrics.title')" data-testid="open-metrics" @click="sessions.openMetrics()"><Activity :size="16" /></UiButton>
      <UiButton class="icon" :title="dark ? t('shell.themeLight') : t('shell.themeDark')" :aria-label="dark ? t('shell.themeLight') : t('shell.themeDark')" data-testid="toggle-theme" @click="emit('theme')">
        <Sun v-if="dark" :size="16" />
        <Moon v-else :size="16" />
      </UiButton>
      <UiButton v-if="!collapsed" class="icon lang" :title="t(`common.language.${nextLocale}`)" :aria-label="t('common.language.label')" data-testid="toggle-language" @click="setLocale(nextLocale)">
        {{ locale.toUpperCase() }}
      </UiButton>
      <UiButton
        v-if="!collapsed && berth && isDesktop && (cleanupCount || cleanupRemoving)"
        class="cleanup-note"
        :title="cleanupRemoving ? t('cleanup.removing', { name: cleanupRemoving.name }) : t('cleanup.buttonCount', { count: cleanupCount })"
        :aria-label="t('cleanup.button')"
        data-testid="open-cleanup"
        @click="openCleanup"
      >
        <LoaderCircle v-if="cleanupRemoving" :size="12" class="spin" data-testid="cleanup-removing" />
        <span class="cleanup-text" data-testid="cleanup-count">{{ cleanupRemoving ? t('cleanup.removing', { name: cleanupRemoving.name }) : t('cleanup.buttonCount', { count: cleanupCount }) }}</span>
      </UiButton>
    </footer>

    <div
      class="resize-handle"
      role="separator"
      aria-orientation="vertical"
      :aria-label="t('shell.resize')"
      tabindex="0"
      :aria-valuenow="currentWidth"
      :aria-valuemin="RAIL_WIDTH"
      :aria-valuemax="maxWidth"
      data-testid="sidebar-resize-handle"
      @pointerdown="startResize"
      @pointermove="moveResize"
      @pointerup="endResize"
      @pointercancel="endResize"
      @keydown="onHandleKey"
    />
  </nav>
</template>

<style scoped>
.sidebar {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  box-sizing: border-box;
  min-height: 0;
  min-width: 0;
  padding: var(--vr-space-2) var(--vr-space-2) 0;
  overflow: hidden;
  color: var(--vr-color-text);
  transition: width var(--vr-motion-base) ease;
}

.sidebar.dragging {
  transition: none;
  user-select: none;
}

.top {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
}

.rail-divider {
  flex: none;
  width: 20px;
  height: 1px;
  margin: var(--vr-space-1) 0;
  background: var(--vr-color-border);
}


.item.rail-item {
  justify-content: center;
  height: 30px;
  padding: 0;
}

.collapsed .primary {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
}

.collapsed footer {
  flex-direction: column;
  margin-top: auto;
  padding: var(--vr-space-3) 0;
}

.resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 2;
  width: 6px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle::after {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 2px;
  background: transparent;
  transition: background var(--vr-motion-fast) ease;
}

.resize-handle:hover::after,
.resize-handle:focus-visible::after,
.dragging .resize-handle::after {
  background: var(--vr-color-accent);
}

.resize-handle:focus-visible {
  outline: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  height: 28px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.item {
  position: relative;
}

.count {
  min-width: 1.45em;
  padding: 0 var(--vr-space-1);
  margin-left: auto;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
  line-height: 1.45;
  text-align: center;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.rail-item .count {
  position: absolute;
  top: 1px;
  right: 0;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  margin-left: 0;
  font-size: calc(var(--vr-font-size-2xs) * 0.86);
  line-height: 1;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
}

.count[data-tone='alert'] {
  background: var(--vr-status-feedback);
}

.count[data-tone='muted'] {
  background: var(--vr-color-text-muted);
}

.item:hover {
  background: var(--vr-color-nav-hover);
}

.item[data-active='true'] {
  font-weight: 600;
  background: var(--vr-color-nav-active);
}

.sections {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  margin: var(--vr-space-2) calc(-1 * var(--vr-space-1)) 0;
  overflow: hidden;
}

.icon.lang {
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
}

footer {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-3) var(--vr-space-1);
}

.cleanup-note {
  display: inline-flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  height: 28px;
  padding: 0 var(--vr-space-1);
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  text-align: left;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.cleanup-note:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.cleanup-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.spin {
  flex: none;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.icon {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-chrome);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.icon:hover,
.icon[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
