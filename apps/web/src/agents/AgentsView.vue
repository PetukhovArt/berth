<script setup lang="ts">
import { t } from '../shared/lib/i18n/i18n';
import BackBar from '../shared/ui/BackBar.vue';
import TerminalKeys from '../mobile/TerminalKeys.vue';
import { useIsPhone } from '@ui/kit';
import { useDock } from '../shared/model/dock';
import ProjectTree from '../workbench/ProjectTree.vue';
import NewSessionForm from './NewSessionForm.vue';
import DockArea from './proto/DockArea.vue';

localStorage.removeItem('berth.agents.layout');
localStorage.removeItem('berth.agents.explorer.expanded');

const dock = useDock();
const { onReady } = dock.useHost();
const mobile = useIsPhone();
const { activeKey, activeTitle, activeSession, detail, creatingIn } = dock;
</script>

<template>
  <div v-if="!dock.berth" class="unavailable" data-testid="berth-banner" data-state="unavailable">{{ t('agents.unavailable') }}</div>
  <div v-else class="workbench vr-master-detail" data-testid="workbench" :data-detail="detail">
    <ProjectTree v-if="mobile" class="vr-master" with-filter />
    <BackBar v-if="mobile && detail" :title="activeSession?.name ?? activeTitle" @back="detail = false" />
    <DockArea class="vr-detail" :active-key="activeKey" @ready="onReady" @open-file="(path, hash, from) => dock.openFile(path, { hash, from })" />
    <TerminalKeys v-if="mobile && detail && activeSession" :session="activeSession.ref" />
    <NewSessionForm v-if="creatingIn" :key="creatingIn.worktree ?? ''" :worktree="creatingIn.worktree" @close="dock.cancelCreate" @created="dock.onCreated" />
  </div>
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.workbench {
  flex: 1;
}
</style>
