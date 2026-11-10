<script setup lang="ts">
import { computed } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { pendingLeadsTo } from '../../commands/dispatcher';
import { shortcutLabel } from '../../commands/keymap';
import WorktreeBadge from '../../forge/WorktreeBadge.vue';
import { t } from '../../i18n';
import { mrOf, showView, viewOf, worktreeOf, type SessionView } from './sessionView';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

type Mode = { view: SessionView; letter: string; action: string };

const MODES: readonly Mode[] = [
  { view: 'terminal', letter: 'T', action: 'session.view.terminal' },
  { view: 'mr', letter: 'M', action: 'session.view.mr' },
  { view: 'diff', letter: 'D', action: 'session.view.diff' },
];

const worktree = computed(() => worktreeOf(props.panel.id));
const hasMr = computed(() => mrOf(props.panel.id) !== null);
const current = computed(() => viewOf(props.panel.id));

const unavailable = (mode: Mode) => mode.view === 'mr' && !hasMr.value;

function label(mode: Mode): string {
  const name = t(`agents.sessionView.${mode.view}`);
  const shortcut = shortcutLabel(mode.action);
  return shortcut ? t('agents.sessionView.withShortcut', { name, shortcut }) : name;
}

function show(view: SessionView) {
  props.panel.setActive();
  showView(props.panel.id, view);
}
</script>

<template>
  <span v-if="worktree" class="switch" :data-showing="current === 'mr'" data-testid="tab-mr" @pointerdown.stop @dblclick.stop>
    <WorktreeBadge :path="worktree.path" :branch="worktree.branch" @open="show('mr')" />
    <span class="modes" role="group" :aria-label="t('agents.sessionView.switcher')">
      <button
        v-for="mode in MODES"
        :key="mode.view"
        class="mode"
        :aria-disabled="unavailable(mode)"
        :aria-pressed="current === mode.view"
        :aria-label="label(mode)"
        :title="unavailable(mode) ? t('agents.sessionView.noMr') : label(mode)"
        :data-pending="active && !unavailable(mode) && pendingLeadsTo(mode.action)"
        :data-testid="`tab-view-${mode.view}`"
        @click.stop="show(mode.view)"
      >
        {{ mode.letter }}
      </button>
    </span>
  </span>
</template>

<style scoped>
.switch {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
}

.switch[data-showing='true'] :deep(.badge) {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.modes {
  display: inline-flex;
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

.mode:hover:not([aria-disabled='true'], [aria-pressed='true']) {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.mode[aria-disabled='true'] {
  opacity: 0.4;
  cursor: default;
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
