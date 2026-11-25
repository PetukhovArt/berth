<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watchEffect } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { INSPECTOR_MIN_WIDTH, inspectorOpen, inspectorShown, inspectorWidth } from '../../../shared/model/inspector';
import { limitKind, useBerth, useSessionDock } from '../../../entities/session';
import WorktreeSection from './WorktreeSection.vue';
import MergeRequestSection from './MergeRequestSection.vue';
import UsageSection from './UsageSection.vue';
import CheckpointsSection from './CheckpointsSection.vue';
import TaskNoteSection from './TaskNoteSection.vue';
import OverlapsSection from './OverlapsSection.vue';
import PeopleSection from './PeopleSection.vue';

const MAIN_MIN_WIDTH = 560;
const MAX_WIDTH = 520;
const KEY_STEP = 16;

const berth = useBerth();
const sessions = useSessionDock();
const root = useTemplateRef<HTMLElement>('root');
const hostWidth = shallowRef(0);
const dragging = shallowRef(false);

const maxWidth = computed(() => Math.max(INSPECTOR_MIN_WIDTH, Math.min(MAX_WIDTH, hostWidth.value - MAIN_MIN_WIDTH)));
const width = computed(() => Math.min(inspectorWidth.value, maxWidth.value));
const fits = computed(() => hostWidth.value > 0 && hostWidth.value - INSPECTOR_MIN_WIDTH >= MAIN_MIN_WIDTH);
const shown = computed(() => inspectorOpen.value && fits.value);

const session = computed(() => sessions.activeSession.value);
const agent = computed(() => {
  const ref = session.value?.ref;
  return ref && 'Agent' in ref ? berth?.entities.value.agents.get(ref.Agent) : undefined;
});
const worktree = computed(() => {
  const id = sessions.activeWorktreeId.value;
  return id ? berth?.entities.value.worktrees.get(id) : undefined;
});
const refreshKey = computed(() => session.value?.status);

watchEffect(() => {
  inspectorShown.value = shown.value;
});

let observer: ResizeObserver | undefined;

onMounted(() => {
  const host = root.value?.parentElement;
  if (!host) return;
  observer = new ResizeObserver(([entry]) => {
    if (entry) hostWidth.value = Math.round(entry.contentRect.width);
  });
  observer.observe(host);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  inspectorShown.value = false;
});

function clamp(value: number) {
  return Math.min(maxWidth.value, Math.max(INSPECTOR_MIN_WIDTH, Math.round(value)));
}

function startResize(event: PointerEvent) {
  if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
  event.preventDefault();
  event.currentTarget.setPointerCapture(event.pointerId);
  dragging.value = true;
}

function moveResize(event: PointerEvent) {
  if (!dragging.value || !root.value) return;
  inspectorWidth.value = clamp(root.value.getBoundingClientRect().right - event.clientX);
}

function endResize() {
  dragging.value = false;
}

function onHandleKey(event: KeyboardEvent) {
  const step = event.key === 'ArrowLeft' ? KEY_STEP : event.key === 'ArrowRight' ? -KEY_STEP : 0;
  if (!step) return;
  event.preventDefault();
  inspectorWidth.value = clamp(width.value + step);
}
</script>

<template>
  <aside
    v-show="shown"
    ref="root"
    class="inspector"
    :class="{ dragging }"
    :style="{ width: `${width}px` }"
    :aria-label="t('inspector.label')"
    data-testid="session-inspector"
  >
    <div
      class="resize-handle"
      role="separator"
      aria-orientation="vertical"
      tabindex="0"
      :aria-label="t('inspector.resize')"
      :aria-valuenow="width"
      :aria-valuemin="INSPECTOR_MIN_WIDTH"
      :aria-valuemax="maxWidth"
      data-testid="session-inspector-resize-handle"
      @pointerdown="startResize"
      @pointermove="moveResize"
      @pointerup="endResize"
      @pointercancel="endResize"
      @keydown="onHandleKey"
    />
    <div v-if="shown" class="scroll">
      <template v-if="session && worktree">
        <WorktreeSection :path="worktree.path" :branch="worktree.branch" :refresh-key="refreshKey" />
        <MergeRequestSection v-if="worktree.branch" :path="worktree.path" :branch="worktree.branch" :refresh-key="refreshKey" />
        <UsageSection v-if="agent" :panel-key="session.key" :kind="limitKind(agent)" />
        <CheckpointsSection v-if="agent" :agent-id="agent.id" :kind="agent.kind" :session-id="agent.session_id" :refresh-key="refreshKey" />
        <TaskNoteSection :worktree-id="worktree.id" />
        <OverlapsSection v-if="agent" :agent-id="agent.id" />
        <PeopleSection v-if="worktree.branch" :path="worktree.path" :branch="worktree.branch" />
      </template>
      <p v-else class="empty" data-testid="session-inspector-empty">{{ t('inspector.empty') }}</p>
    </div>
  </aside>
</template>

<style scoped>
.inspector {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  box-sizing: border-box;
  min-height: 0;
  background: var(--vr-color-surface);
  border-left: 1px solid var(--vr-color-border);
}

.inspector.dragging {
  user-select: none;
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.empty {
  margin: 0;
  padding: var(--vr-space-4) 14px;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-subtle);
}

.resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -3px;
  z-index: 2;
  width: 6px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle::after {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: 2px;
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
</style>
