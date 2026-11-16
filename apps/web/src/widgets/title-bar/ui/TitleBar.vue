<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { Copy, LayoutGrid, Minus, PanelRight, Search, Square, X } from '../../../shared/ui/icons';
import { hostWindow as win } from '../../../bridge';
import { settleAll } from '../../../entities/document';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { runAction } from '../../../shared/lib/commands/registry';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { hostPlatform } from '../../../shared/lib/platform';
import { toggleQuickOpen } from '../../../shared/model/dock';
import { inspectorOpen } from '../../../shared/model/inspector';
import { view } from '../../../shared/model/nav';
import { useSettings } from '../../../shared/config/settings/store';
import { UpdateChip } from '../../../features/update';
import { WorkspaceSwitcher } from '../../../features/manage-workspace';

import { UiButton } from '../../../shared/ui/UiButton';
const { t } = useI18n();
const { values } = useSettings();
const maximized = ref(false);
const nativeControls = computed(() => hostPlatform.value.windowChrome.nativeDecorations);
const hotOn = computed(() => (values.value as Record<string, unknown>)['workbench.hot'] !== 'off');
const hotKey = computed(() => shortcutLabel('hot.overview'));
const inspectorLabel = computed(() => (inspectorOpen.value ? t('inspector.hide') : t('inspector.show')));

function withShortcut(label: string, actionId: string) {
  const shortcut = shortcutLabel(actionId);
  return shortcut ? `${label} (${shortcut})` : label;
}
let stopResize: (() => void) | undefined;

async function closeWindow() {
  if (await settleAll()) await win.close();
}

async function sync() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  await sync();
  stopResize = await win.onResized(() => void sync());
});

onUnmounted(() => stopResize?.());
</script>

<template>
  <header class="titlebar" data-tauri-drag-region data-testid="titlebar" :data-native-controls="nativeControls">
    <div class="start" data-tauri-drag-region>
      <div class="mark" data-tauri-drag-region aria-hidden="true">b</div>
      <WorkspaceSwitcher />
    </div>
    <UiButton
      type="button"
      class="search vr-wide-only"
      data-testid="titlebar-search"
      :aria-label="t('common.search')"
      :title="t('common.search')"
      @click="toggleQuickOpen()"
    >
      <Search :size="13" :stroke-width="1.6" />
      <span class="search-label">{{ t('common.searchPlaceholder') }}</span>
      <kbd v-if="shortcutLabel('agents.quickOpen')">{{ shortcutLabel('agents.quickOpen') }}</kbd>
    </UiButton>
    <div class="end">
      <div class="update" data-tauri-drag-region><UpdateChip /></div>
      <div class="tools">
        <UiButton
          v-if="hotOn"
          type="button"
          class="tool"
          data-testid="titlebar-hot"
          :aria-label="t('titleBar.hot')"
          :title="withShortcut(t('titleBar.hot'), 'hot.overview')"
          @click="runAction('hot.overview')"
        >
          <LayoutGrid :size="14" :stroke-width="1.6" />
          <kbd v-if="hotKey">{{ hotKey }}</kbd>
        </UiButton>
        <UiButton
          v-if="view.kind === 'agents'"
          type="button"
          class="tool"
          data-testid="titlebar-inspector"
          :aria-label="inspectorLabel"
          :aria-pressed="inspectorOpen"
          :data-active="inspectorOpen"
          :title="withShortcut(inspectorLabel, 'inspector.toggle')"
          @click="runAction('inspector.toggle')"
        >
          <PanelRight :size="15" :stroke-width="1.6" />
        </UiButton>
      </div>
      <div v-if="!nativeControls" class="controls">
        <UiButton type="button" class="ctl" data-testid="titlebar-minimize" :aria-label="t('shell.window.minimize')" :title="t('shell.window.minimize')" @click="win.minimize()">
          <Minus :size="16" :stroke-width="1.5" />
        </UiButton>
        <UiButton
          type="button"
          class="ctl"
          data-testid="titlebar-maximize"
          :data-maximized="maximized"
          :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
          :title="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
          @click="win.toggleMaximize()"
        >
          <Copy v-if="maximized" :size="13" :stroke-width="1.5" />
          <Square v-else :size="12" :stroke-width="1.5" />
        </UiButton>
        <UiButton type="button" class="ctl close" data-testid="titlebar-close" :aria-label="t('shell.window.close')" :title="t('shell.window.close')" @click="closeWindow">
          <X :size="16" :stroke-width="1.5" />
        </UiButton>
      </div>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: grid;
  flex: none;
  grid-template-columns: minmax(0, 1fr) minmax(0, 420px) minmax(0, 1fr);
  align-items: center;
  height: 40px;
  padding-left: var(--vr-space-3);
  background: var(--vr-color-surface);
  border-bottom: 1px solid var(--vr-color-border);
  user-select: none;
  -webkit-user-select: none;
}

@media (pointer: coarse) {
  :root[data-vr-viewport='phone'] .titlebar {
    display: none;
  }
}

.titlebar[data-native-controls='true'] {
  padding-left: 78px;
}

.start {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 100%;
  min-width: 0;
}

.tools {
  display: flex;
  flex: none;
  align-items: center;
  gap: 2px;
  padding-right: var(--vr-space-2);
}

.tool {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  min-width: 28px;
  padding: 0 6px;
  justify-content: center;
  color: var(--vr-color-text-subtle);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.tool:hover,
.tool:focus-visible {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.tool[data-active='true'] {
  color: var(--vr-color-text);
}

.tool kbd {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.mark {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 5px;
  font-size: var(--vr-font-size-xs);
  font-weight: 700;
  color: var(--vr-color-brand-contrast);
  background: var(--vr-color-brand);
}

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 26px;
  padding: 0 6px 0 10px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-subtle);
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: 7px;
  box-sizing: border-box;
}

.search:hover {
  background: var(--vr-color-nav-active);
}

.search-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-align: left;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.search kbd {
  flex: none;
  padding: 0 5px;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 4px;
}

.end {
  display: flex;
  grid-column: 3;
  align-items: center;
  justify-content: flex-end;
  height: 100%;
  min-width: 0;
}

.update {
  display: flex;
  min-width: 0;
  padding-right: var(--vr-space-2);
}

.controls {
  display: flex;
  height: 100%;
}

.ctl {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 100%;
  padding: 0;
  color: var(--vr-color-text-subtle);
  background: transparent;
  border: 0;
  border-radius: 0;
}

.ctl:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.ctl:active {
  background: var(--vr-color-nav-active);
}

.ctl.close:hover {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}
</style>
