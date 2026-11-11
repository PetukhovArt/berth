<script setup lang="ts">
import { t } from '../../../shared/lib/i18n/i18n';
import BackBar from '../../../shared/ui/BackBar.vue';
import { TerminalKeys, SessionPanel } from '../../../widgets/terminal';
import { useIsPhone } from '../../../shared/ui/kit';
import { useDock } from '../../../shared/model/dock';
import { useSessionDock, isSessionRef, useBerth, type Harness } from '../../../entities/session';
import { openIssue } from '../../../entities/jira-issue';
import { ProjectTree } from '../../../widgets/project-tree';
import { NewSessionForm, cancelCreate, creatingIn, onCreated } from '../../../features/new-session';
import { ArchitectureView } from '../../../features/architecture';
import { SessionViews } from '../../../features/session-view-switch';
import { DockArea } from '../../../widgets/dock';
import { ChangeRequestView } from '../../../widgets/change-request';
import { WorktreeChanges } from '../../../widgets/worktree-changes';
import { JiraIssueView } from '../../../widgets/jira';
import { ConversationView } from '../../../widgets/conversation';
import { MetricsPanel } from '../../../widgets/metrics';
import { MarkdownFile } from '../../../widgets/markdown-file';

localStorage.removeItem('berth.agents.layout');
localStorage.removeItem('berth.agents.explorer.expanded');

const berth = useBerth();
const dock = useDock();
const sessions = useSessionDock();
const { onReady } = dock.useHost();
const mobile = useIsPhone();
const { activeKey, activeTitle, detail } = dock;
const { activeSession } = sessions;
const isHarness = (value: unknown): value is Harness => value === 'claude' || value === 'codex';
const str = (value: unknown) => (typeof value === 'string' ? value : '');
</script>

<template>
  <div v-if="!berth" class="unavailable" data-testid="berth-banner" data-state="unavailable">{{ t('agents.unavailable') }}</div>
  <div v-else class="workbench vr-master-detail" data-testid="workbench" :data-detail="detail">
    <ProjectTree v-if="mobile" class="vr-master" with-filter />
    <BackBar v-if="mobile && detail" :title="activeSession?.name ?? activeTitle" @back="detail = false" />
    <DockArea class="vr-detail" :active-key="activeKey" @ready="onReady">
      <template #panel-session="{ params, api }">
        <SessionViews :panel-id="api.id">
          <template #terminal><SessionPanel v-if="isSessionRef(params.session)" class="vr-fade-in" :session="params.session" :panel="api" /></template>
          <template #mr="{ mr }"><ChangeRequestView class="pane vr-fade-in" :connector="mr.connector" :project="mr.project" :number="mr.number" data-testid="session-mr-view" /></template>
          <template #diff="{ path, status }"><WorktreeChanges class="pane" :path="path" :status="status" data-testid="session-diff-view" /></template>
        </SessionViews>
      </template>
      <template #panel-review="{ params }">
        <ChangeRequestView v-if="typeof params.connector === 'string' && typeof params.project === 'string' && typeof params.number === 'number'" class="vr-fade-in" :connector="params.connector" :project="params.project" :number="params.number" />
      </template>
      <template #panel-architecture="{ params }">
        <ArchitectureView
          v-if="str(params.connector) && str(params.project) && typeof params.number === 'number' && str(params.repo)"
          class="vr-fade-in"
          :connector="str(params.connector)"
          :project="str(params.project)"
          :number="params.number"
          :repo="str(params.repo)"
          :target="str(params.target)"
        />
      </template>
      <template #panel-issue="{ params }">
        <JiraIssueView v-if="typeof params.connector === 'string' && typeof params.key === 'string'" class="vr-fade-in" :connector="params.connector" :issue-key="params.key" @open="(ref) => openIssue(ref.connector, ref.key)" />
      </template>
      <template #panel-conversation="{ params, api }">
        <SessionViews :panel-id="api.id">
          <template #terminal><ConversationView v-if="isHarness(params.harness) && typeof params.id === 'string'" class="vr-fade-in" :harness="params.harness" :session-id="params.id" /></template>
          <template #mr="{ mr }"><ChangeRequestView class="pane vr-fade-in" :connector="mr.connector" :project="mr.project" :number="mr.number" data-testid="session-mr-view" /></template>
          <template #diff="{ path, status }"><WorktreeChanges class="pane" :path="path" :status="status" data-testid="session-diff-view" /></template>
        </SessionViews>
      </template>
      <template #panel-metrics>
        <MetricsPanel class="vr-fade-in" />
      </template>
      <template #panel-markdown="{ params, api }">
        <MarkdownFile v-if="typeof params.path === 'string'" class="vr-fade-in" :path="params.path" :hash="typeof params.hash === 'string' ? params.hash : ''" @open-file="(path, hash) => sessions.openFile(path, { hash, from: api.id })" />
      </template>
    </DockArea>
    <TerminalKeys v-if="mobile && detail && activeSession" :session="activeSession.ref" />
    <NewSessionForm v-if="creatingIn" :key="creatingIn.worktree ?? ''" :worktree="creatingIn.worktree" @close="cancelCreate" @created="onCreated" />
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
