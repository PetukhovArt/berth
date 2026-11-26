<script setup lang="ts">
import { computed, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { pendingLeadsTo } from '../../../shared/lib/commands/dispatcher';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { numberLabel, knownMr, lookupMr } from '../../../entities/merge-request';
import { lookupChanges } from '../model/worktreeChanged';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth } from '../../../entities/session';
import { VSelectButton, useIsPhone } from '../../../shared/ui/kit';
import { isAvailable, sessionViewModes, showView, viewOf, worktreeOf, type SessionView, type SessionViewMode } from '../model/sessionView';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const berth = useBerth();
const worktree = computed(() => worktreeOf(props.panel.id));
const current = computed(() => viewOf(props.panel.id));
const phone = useIsPhone();
const modes = computed(() => {
  const available = sessionViewModes.filter((mode) => isAvailable(props.panel.id, mode.view));
  return phone.value ? [...available].sort((a, b) => Number(b.view === 'chat') - Number(a.view === 'chat')) : available;
});

watch(
  () => [worktree.value?.path, worktree.value?.branch, berth?.sessions.value.get(props.panel.id)?.status, props.active] as const,
  ([path, branch]) => {
    if (!path || !branch) return;
    void lookupMr(path, branch);
    lookupChanges(path);
  },
  { immediate: true },
);

function label(mode: SessionViewMode): string {
  const name = t(`agents.sessionView.${mode.view}`);
  const shortcut = shortcutLabel(mode.action);
  return shortcut ? t('agents.sessionView.withShortcut', { name, shortcut }) : name;
}

function title(mode: SessionViewMode): string {
  const mr = mode.view === 'mr' && worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null;
  return mr ? `${label(mode)}\n${numberLabel(mr)} ${mr.title}` : label(mode);
}

const options = computed(() =>
  modes.value.map((mode) => ({ value: mode.view, label: t(`agents.sessionView.${mode.view}`), title: title(mode), testId: `tab-view-${mode.view}` })),
);

const selected = computed({
  get: () => current.value,
  set: (view: string | string[] | null | undefined) => {
    if (typeof view === 'string') showView(props.panel.id, view as SessionView);
  },
});

const modeOf = (view: string) => sessionViewModes.find((mode) => mode.view === view);
const actionOf = (view: string) => modeOf(view)?.action ?? '';
const letterOf = (view: string) => modeOf(view)?.letter ?? '';
</script>

<template>
  <span v-if="modes.length > 1" class="modes" data-testid="tab-mr" @pointerdown.stop @dblclick.stop @click.stop="panel.setActive()">
    <VSelectButton v-model="selected" class="switch" :options="options" :label="t('agents.sessionView.switcher')">
      <template #option="{ option }">
        <span class="mode" :data-pending="active && pendingLeadsTo(actionOf(option.value)) || undefined">
          <span class="name">{{ option.label }}</span>
          <span class="key">{{ letterOf(option.value) }}</span>
        </span>
      </template>
    </VSelectButton>
  </span>
</template>

<style scoped>
.modes {
  display: inline-flex;
  flex: none;
  align-items: center;
}

.mode {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-1);
  white-space: nowrap;
  border-radius: var(--vr-radius-sm);
}

.key {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-subtle);
}

.mode[data-pending='true'] {
  box-shadow: inset 0 0 0 1px var(--vr-color-accent);
}

:root[data-vr-viewport='phone'] .modes {
  display: flex;
  flex: 1 1 100%;
  min-width: 0;
}

:root[data-vr-viewport='phone'] .switch {
  flex: 1 1 auto;
  min-width: 0;
}

:root[data-vr-viewport='phone'] .name {
  overflow: hidden;
  text-overflow: ellipsis;
}

:root[data-vr-viewport='phone'] .key {
  display: none;
}
</style>
