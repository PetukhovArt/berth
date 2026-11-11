<script setup lang="ts">
import { computed, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { pendingLeadsTo } from '../../../shared/lib/commands/dispatcher';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { numberLabel } from '../../../entities/merge-request/lib/format';
import { lookupChanges } from '../model/worktreeChanged';
import { knownMr, lookupMr } from '../../../entities/merge-request/model/worktreeMr';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth } from '../../../entities/session/model/berth';
import { isAvailable, showView, viewOf, worktreeOf, type SessionView } from '../model/sessionView';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

type Mode = { view: SessionView; letter: string; action: string };

const MODES: readonly Mode[] = [
  { view: 'terminal', letter: 'T', action: 'session.view.terminal' },
  { view: 'mr', letter: 'M', action: 'session.view.mr' },
  { view: 'diff', letter: 'D', action: 'session.view.diff' },
];

const berth = useBerth();
const worktree = computed(() => worktreeOf(props.panel.id));
const current = computed(() => viewOf(props.panel.id));
const modes = computed(() => MODES.filter((mode) => isAvailable(props.panel.id, mode.view)));

watch(
  () => [worktree.value?.path, worktree.value?.branch, berth?.sessions.value.get(props.panel.id)?.status, props.active] as const,
  ([path, branch]) => {
    if (!path || !branch) return;
    void lookupMr(path, branch);
    lookupChanges(path);
  },
  { immediate: true },
);

function label(mode: Mode): string {
  const name = t(`agents.sessionView.${mode.view}`);
  const shortcut = shortcutLabel(mode.action);
  return shortcut ? t('agents.sessionView.withShortcut', { name, shortcut }) : name;
}

function title(mode: Mode): string {
  const mr = mode.view === 'mr' && worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null;
  return mr ? `${label(mode)}\n${numberLabel(mr)} ${mr.title}` : label(mode);
}

function show(view: SessionView) {
  props.panel.setActive();
  showView(props.panel.id, view);
}
</script>

<template>
  <span v-if="modes.length > 1" class="modes" role="group" :aria-label="t('agents.sessionView.switcher')" data-testid="tab-mr" @pointerdown.stop @dblclick.stop>
    <button
      v-for="mode in modes"
      :key="mode.view"
      class="mode"
      :aria-pressed="current === mode.view"
      :aria-label="label(mode)"
      :title="title(mode)"
      :data-pending="active && pendingLeadsTo(mode.action)"
      :data-testid="`tab-view-${mode.view}`"
      @click.stop="show(mode.view)"
    >
      {{ mode.letter }}
    </button>
  </span>
</template>

<style scoped>
.modes {
  display: inline-flex;
  flex: none;
  align-items: center;
  overflow: hidden;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.mode {
  min-width: 14px;
  padding: 0 3px;
  color: var(--vr-color-text);
  font: inherit;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  line-height: 16px;
  background: none;
  border: none;
  cursor: pointer;
}

.mode:hover:not([aria-pressed='true']) {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.mode[aria-pressed='true'] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.mode[data-pending='true'] {
  color: var(--vr-color-text);
  box-shadow: inset 0 0 0 1px var(--vr-color-accent);
}

.mode[aria-pressed='true'][data-pending='true'] {
  color: var(--vr-color-accent-contrast);
}
</style>
