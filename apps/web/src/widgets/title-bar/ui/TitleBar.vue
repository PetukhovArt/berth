<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { Copy, Minus, PanelRight, Search, Square, X } from '../../../shared/ui/icons';
import { hostWindow as win } from '../../../bridge';
import { settleAll } from '../../../entities/document';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { runAction } from '../../../shared/lib/commands/registry';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { hostPlatform } from '../../../shared/lib/platform';
import { toggleQuickOpen } from '../../../shared/model/dock';
import { inspectorOpen } from '../../../shared/model/inspector';
import { view } from '../../../shared/model/nav';
import { firstRunActive } from '../../../shared/model/firstRun';
import { UpdateChip } from '../../../features/update';
import { WorkspaceSwitcher } from '../../../features/manage-workspace';
import { ConnectorErrors } from '../../../features/work-tree';
import { VButton, VToggleButton, vTooltip } from '../../../shared/ui/kit';
import BrandMark from '../../../shared/ui/BrandMark.vue';
const { t } = useI18n();
const maximized = ref(false);
const nativeControls = computed(() => hostPlatform.value.windowChrome.nativeDecorations);
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
      <BrandMark size="18px" data-testid="titlebar-mark" />
      <WorkspaceSwitcher v-if="!firstRunActive" />
    </div>
    <VButton
      v-if="!firstRunActive"
      v-tooltip="t('common.search')"
      class="search vr-wide-only"
      severity="secondary"
      size="small"
      data-testid="titlebar-search"
      :aria-label="t('common.search')"
      @click="toggleQuickOpen()"
    >
      <Search :size="13" :stroke-width="1.6" />
      <span class="search-label">{{ t('search.placeholder') }}</span>
      <kbd v-if="shortcutLabel('agents.quickOpen')">{{ shortcutLabel('agents.quickOpen') }}</kbd>
    </VButton>
    <div class="end">
      <div class="update" data-tauri-drag-region><UpdateChip /></div>
      <div v-if="!firstRunActive" class="tools">
        <ConnectorErrors />
        <VToggleButton
          v-if="view.kind === 'agents'"
          v-tooltip="withShortcut(inspectorLabel, 'inspector.toggle')"
          class="tool"
          size="small"
          data-testid="titlebar-inspector"
          :model-value="inspectorOpen"
          :aria-label="inspectorLabel"
          @update:model-value="runAction('inspector.toggle')"
        >
          <PanelRight :size="15" :stroke-width="1.6" />
        </VToggleButton>
      </div>
      <div v-if="!nativeControls" class="controls">
        <VButton v-tooltip="t('shell.window.minimize')" class="ctl" text severity="secondary" data-testid="titlebar-minimize" :aria-label="t('shell.window.minimize')" @click="win.minimize()">
          <template #icon><Minus :size="16" :stroke-width="1.5" /></template>
        </VButton>
        <VButton
          v-tooltip="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
          class="ctl"
          text
          severity="secondary"
          data-testid="titlebar-maximize"
          :data-maximized="maximized"
          :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
          @click="win.toggleMaximize()"
        >
          <template #icon>
            <Copy v-if="maximized" :size="13" :stroke-width="1.5" />
            <Square v-else :size="12" :stroke-width="1.5" />
          </template>
        </VButton>
        <VButton v-tooltip="t('shell.window.close')" class="ctl close" text severity="secondary" data-testid="titlebar-close" :aria-label="t('shell.window.close')" @click="closeWindow">
          <template #icon><X :size="16" :stroke-width="1.5" /></template>
        </VButton>
      </div>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: grid;
  flex: none;
  grid-template-columns: minmax(min-content, 1fr) minmax(0, 420px) minmax(min-content, 1fr);
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
  height: 26px;
  min-width: 28px;
}

.search {
  container: titlebar-search / inline-size;
  gap: 8px;
  width: 100%;
  height: 26px;
  padding: 0 6px 0 10px;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-subtle);
  box-sizing: border-box;
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

@container titlebar-search (max-width: 22rem) {
  .search kbd {
    display: none;
  }
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
  width: 44px;
  height: 100%;
  padding: 0;
  border-radius: 0;
}

:root .ctl.close:hover {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}
</style>
