<script setup lang="ts">
import { computed } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import type { ChangeRequestRef } from '../../connectors/api';
import WorktreeBadge from '../../forge/WorktreeBadge.vue';
import { useDock } from '../../workbench/dock';
import { mrShownIn, toggleMr } from './tabMrView';
import { useBerth } from '../berth';
import { useExternalSessions } from '../external';

const props = defineProps<{ panel: DockviewPanelApi }>();

const berth = useBerth();
const external = useExternalSessions();
const dock = useDock();

const target = computed(() => {
  const worktreeId = dock.paths.value.get(props.panel.id)?.worktree.id;
  const worktree = worktreeId ? berth?.entities.value.worktrees.get(worktreeId) : undefined;
  if (worktree) return { path: worktree.path, branch: worktree.branch };
  const conversation = external?.anyByKey.value.get(props.panel.id);
  return conversation?.branch ? { path: conversation.cwd, branch: conversation.branch } : null;
});

const showing = computed(() => Boolean(mrShownIn(props.panel.id)));

function open(ref: ChangeRequestRef) {
  props.panel.setActive();
  toggleMr(props.panel.id, ref);
}
</script>

<template>
  <span v-if="target" class="tab-mr" :data-showing="showing" data-testid="tab-mr" @pointerdown.stop @dblclick.stop>
    <WorktreeBadge :path="target.path" :branch="target.branch" @open="open" />
  </span>
</template>

<style scoped>
.tab-mr {
  display: inline-flex;
  flex: none;
  align-items: center;
}

.tab-mr[data-showing='true'] :deep(.badge) {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}
</style>
