<script setup lang="ts">
import { computed, nextTick, useTemplateRef, watch } from 'vue';
import ChangeRequestView from '../../../widgets/change-request/ui/ChangeRequestView.vue';
import WorktreeChanges from '../../../widgets/worktree-changes/ui/WorktreeChanges.vue';
import { useBerth } from '../../../entities/session/model/berth';
import { externalDot, sessionDot, useExternalSessions } from '../../../entities/session/model/external';
import { mrOf, viewOf, worktreeOf } from '../model/sessionView';

const props = defineProps<{ panelId: string }>();

const berth = useBerth();
const external = useExternalSessions();

const view = computed(() => viewOf(props.panelId));
const mr = computed(() => (view.value === 'mr' ? mrOf(props.panelId) : null));
const terminalPane = useTemplateRef<HTMLElement>('terminalPane');

watch(view, async (next, previous) => {
  if (next !== 'terminal' || previous === 'terminal') return;
  await nextTick();
  terminalPane.value?.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea')?.focus();
});

const diffRoot = computed(() => (view.value === 'diff' ? worktreeOf(props.panelId)?.path : undefined));

const status = computed(() => {
  const node = berth?.sessions.value.get(props.panelId);
  if (node) return sessionDot(node, external?.linked.value.get(props.panelId));
  const conversation = external?.anyByKey.value.get(props.panelId);
  return conversation ? externalDot(conversation) : undefined;
});
</script>

<template>
  <div class="host" :data-view="view === 'terminal' ? 'session' : view">
    <div v-show="view === 'terminal'" ref="terminalPane" class="pane"><slot /></div>
    <ChangeRequestView v-if="mr" class="pane vr-fade-in" :connector="mr.connector" :project="mr.project" :number="mr.number" data-testid="session-mr-view" />
    <WorktreeChanges v-if="diffRoot" class="pane" :path="diffRoot" :status="status" data-testid="session-diff-view" />
  </div>
</template>

<style scoped>
.host {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.pane {
  flex: 1;
  min-height: 0;
}
</style>
