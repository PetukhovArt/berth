<script setup lang="ts">
import { watch } from 'vue';
import { AgentsView } from '../../pages/agents';
import { CleanupDialog, installCleanup } from '../../features/cleanup';
import { CommandPalette } from '../../widgets/command-palette';
import { paletteOpen } from '../../shared/lib/commands/context';
import { UnsavedDialog } from '../../features/unsaved-changes';
import { HotHost } from '../../widgets/hot';
import { MobileNav } from '../../widgets/mobile-nav';
import { useIsPhone } from '../../shared/ui/kit';
import { Sidebar } from '../../widgets/project-tree';
import { TitleBar } from '../../widgets/title-bar';
import { installTray } from '../tray';
import { ChangesDialog, UpdateDialog, installAutoUpdate, installUpdate } from '../../features/update';
import { view, type View } from '../../shared/model/nav';
import { isDark, toggleTheme } from '../../shared/ui/appearance';
import { SettingsView } from '../../pages/settings';
import { ConfluenceView } from '../../pages/confluence';
import { JiraView } from '../../pages/jira';
import { openReview } from '../../entities/merge-request';
import { QuickOpen } from '../../widgets/quick-open';

const inTauri = '__TAURI_INTERNALS__' in window;
const mobile = useIsPhone();
const viewKey = 'berth.view.v2';
installTray();
installCleanup();
installUpdate();
installAutoUpdate();

try {
  view.value = JSON.parse(localStorage.getItem(viewKey) ?? '') as View;
} catch {
  view.value = { kind: 'agents' };
}
watch(
  view,
  (value) => {
    if (value.kind === 'reviews') {
      view.value = { kind: 'agents' };
      if (value.open) openReview(value.open);
      return;
    }
    localStorage.setItem(viewKey, JSON.stringify(value));
  },
  { immediate: true },
);
</script>

<template>
  <div class="shell">
    <TitleBar v-if="inTauri && mobile" />
    <div class="body" :data-mobile="mobile">
      <Sidebar v-if="!mobile" :dark="isDark" @theme="toggleTheme">
        <template v-if="inTauri" #head>
          <TitleBar class="sidebar-titlebar" />
        </template>
      </Sidebar>
      <main class="card vt-main">
        <KeepAlive include="AgentsView">
          <AgentsView v-if="view.kind === 'agents'" />
          <JiraView v-else-if="view.kind === 'jira'" />
          <ConfluenceView v-else-if="view.kind === 'confluence'" />
          <SettingsView v-else />
        </KeepAlive>
      </main>
      <MobileNav v-if="mobile" class="mobile-nav" />
    </div>
    <QuickOpen />
    <UnsavedDialog />
    <UpdateDialog v-if="inTauri" />
    <ChangesDialog v-if="inTauri" />
    <CleanupDialog v-if="inTauri" />
    <HotHost />
    <Transition name="palette">
      <CommandPalette v-if="paletteOpen" />
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.body {
  display: grid;
  flex: 1;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 0;
  min-width: 0;
}

.body[data-mobile='true'] {
  grid-template-rows: minmax(0, 1fr) auto;
  grid-template-columns: minmax(0, 1fr);
}

@media (orientation: landscape) {
  .body[data-mobile='true'] {
    grid-template-rows: minmax(0, 1fr);
    grid-template-columns: auto minmax(0, 1fr);
  }

  .body[data-mobile='true'] > .mobile-nav {
    order: -1;
  }

  .body[data-mobile='true'] > .card {
    padding-bottom: var(--vr-safe-bottom);
  }
}

.body[data-mobile='true'] .card {
  margin: 0;
  border-radius: 0;
  box-shadow: none;
}

.body .sidebar-titlebar {
  flex-wrap: wrap;
  height: auto;
  min-height: 32px;
  margin: calc(-1 * var(--vr-space-2)) calc(-1 * var(--vr-space-2)) var(--vr-space-1);
}

.body .sidebar-titlebar :deep(.update) {
  min-width: 0;
  overflow: hidden;
}

.body .sidebar-titlebar :deep(.ctl) {
  width: 34px;
  height: 32px;
}

.body .collapsed .sidebar-titlebar {
  flex: none;
  flex-direction: column;
  flex-wrap: nowrap;
  align-items: stretch;
  padding-left: 0;
}

.body .collapsed .sidebar-titlebar :deep(.update) {
  flex: none;
  justify-content: center;
  padding-right: 0;
}

.body .collapsed .sidebar-titlebar :deep(.mark) {
  align-self: center;
  margin-top: var(--vr-space-2);
}

.body .collapsed .sidebar-titlebar :deep(.controls) {
  flex-direction: column;
}

.body .collapsed .sidebar-titlebar :deep(.ctl) {
  width: 100%;
  height: 28px;
}

.card {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  margin: var(--vr-space-2) var(--vr-space-2) var(--vr-space-2) 1px;
  overflow: hidden;
  background: var(--vr-color-surface);
  border-radius: var(--vr-radius-2xl);
  box-shadow: var(--vr-elevation-card);
}
</style>
