<script setup lang="ts">
import { defineAsyncComponent, watch } from 'vue';
import { AgentsView } from '../../pages/agents';
import { CleanupDialog } from '../../features/cleanup';
import { CommandPalette } from '../../widgets/command-palette';
import { paletteOpen } from '../../shared/lib/commands/context';
import { UnsavedDialog } from '../../features/unsaved-changes';
import { HotHost } from '../../widgets/hot';
import { MobileNav } from '../../widgets/mobile-nav';
import { useIsPhone } from '../../shared/ui/kit';
import { Sidebar } from '../../widgets/project-tree';
import { TitleBar } from '../../widgets/title-bar';
import { ChangesDialog, UpdateDialog } from '../../features/update';
import { view, type View } from '../../shared/model/nav';
import { isDark, toggleTheme } from '../../shared/ui/appearance';
import { openReview, reviveReview } from '../../entities/merge-request';
import { useDock } from '../../shared/model/dock';
import { QuickOpen } from '../../widgets/quick-open';

const SettingsView = defineAsyncComponent(() => import('../../pages/settings').then((m) => m.SettingsView));
const ConfluenceView = defineAsyncComponent(() => import('../../pages/confluence').then((m) => m.ConfluenceView));
const JiraView = defineAsyncComponent(() => import('../../pages/jira').then((m) => m.JiraView));

const inTauri = '__TAURI_INTERNALS__' in window;
const mobile = useIsPhone();
const viewKey = 'berth.view.v2';

useDock().revivable(reviveReview);

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
    <TitleBar v-if="inTauri" />
    <div class="body" :data-mobile="mobile">
      <Sidebar v-if="!mobile" :dark="isDark" @theme="toggleTheme" />
      <main class="pane vt-main">
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

  .body[data-mobile='true'] > .pane {
    padding-bottom: var(--vr-safe-bottom);
  }
}

.body > .sidebar {
  background: var(--vr-color-surface);
  border-right: 1px solid var(--vr-color-border);
}

.pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  background: var(--vr-color-canvas);
}
</style>
