<script setup lang="ts">
import { computed } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import type { ChangeRequestRef } from '../../connectors/api';
import WorktreeBadge from '../../forge/WorktreeBadge.vue';
import { useDock } from '../../workbench/dock';
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

function open(ref: ChangeRequestRef) {
  props.panel.setActive();
  dock.openReview(ref);
}
</script>

<template>
  <span v-if="target" class="tab-mr" @pointerdown.stop @dblclick.stop>
    <WorktreeBadge :path="target.path" :branch="target.branch" @open="open" />
  </span>
</template>

<style scoped>
.tab-mr {
  display: inline-flex;
  flex: none;
  align-items: center;
}
</style>
