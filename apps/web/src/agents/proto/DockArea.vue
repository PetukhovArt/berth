<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { onBeforeUnmount } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewPanelApi, type DockviewReadyEvent } from 'dockview-vue';
import { FileText, GitPullRequest, Network, Ticket, X } from 'lucide-vue-next';
import JiraIssueView from '../../atlassian/JiraIssueView.vue';
import ArchitectureView from '../../forge/ArchitectureView.vue';
import ChangeRequestView from '../../forge/ChangeRequestView.vue';
import { t } from '../../i18n';
import MarkdownFile from '../../files/MarkdownFile.vue';
import ConversationView from '../../workbench/ConversationView.vue';
import { METRICS_PANEL, useDock } from '../../workbench/dock';
import { externalDot, isExternalPanel, sessionDot, useExternalSessions, type Harness } from '../external';
import HarnessIcon from '../HarnessIcon.vue';
import { isSessionRef, useBerth } from '../berth';
import SessionPanel from '../SessionPanel.vue';
import StatusDot from '../StatusDot.vue';
import MetricsPanel from '../manage/MetricsPanel.vue';
import { isDirtyPath, settleDirty } from '../../files/documents';
import { isDesktop } from '../../bridge';
import { useSettings } from '../../settings/store';
import { FILE_PANEL_PREFIX, isFilePanel } from './markdownFiles';
import { startTabDrag, trackTabOrder } from './tabOrder';
import TabMrBadge from './TabMrBadge.vue';
import SessionOrMr from './SessionOrMr.vue';

defineProps<{ activeKey: string | null }>();
const emit = defineEmits<{ ready: [event: DockviewReadyEvent]; 'open-file': [path: string, hash: string, from: string] }>();

const berth = useBerth();
const external = useExternalSessions();
const dock = useDock();
const isHarness = (value: unknown): value is Harness => value === 'claude' || value === 'codex';
const tabHarness = (id: string): Harness => (id.startsWith('ext:codex:') ? 'codex' : 'claude');
const tabDot = (id: string) => {
  const session = external?.anyByKey.value.get(id);
  return session ? externalDot(session) : 'closed';
};
const tabSession = (id: string) => berth?.sessions.value.get(id);
const tabLinked = (id: string) => external?.linked.value.get(id);
const sessionTabDot = (id: string) => {
  const session = tabSession(id);
  return session ? sessionDot(session, tabLinked(id)) : undefined;
};
const { values } = useSettings();
const subscriptions: { dispose(): void }[] = [];
let dockApi: DockviewApi | null = null;

function onReady(event: DockviewReadyEvent) {
  const api = event.api;
  dockApi = api;
  subscriptions.push(...trackTabOrder(api));
  emit('ready', event);
}

function kindOf(id: string): 'metrics' | 'file' | 'review' | 'architecture' | 'issue' | 'conversation' | 'session' {
  if (id === METRICS_PANEL) return 'metrics';
  if (id.startsWith('arch:')) return 'architecture';
  if (isExternalPanel(id)) return 'conversation';
  if (isFilePanel(id)) return 'file';
  if (id.startsWith('mr:')) return 'review';
  if (id.startsWith('jira:')) return 'issue';
  return 'session';
}

async function closeTab(api: { id: string; close: () => void }) {
  if (isFilePanel(api.id) && !(await settleDirty([api.id.slice(FILE_PANEL_PREFIX.length)]))) return;
  api.close();
}

const str = (value: unknown) => (typeof value === 'string' ? value : '');

function onTabPointerDown(event: PointerEvent, panel: DockviewPanelApi) {
  if (isDesktop) startTabDrag(event, panel, dockApi);
}

onBeforeUnmount(() => {
  for (const subscription of subscriptions) subscription.dispose();
  dockApi = null;
});
</script>

<template>
  <div class="dock" :data-tabs="values['workbench.tabs']">
    <DockviewVue class="dockview" :theme="themeLight" :disable-dnd="isDesktop" @ready="onReady">
      <template #panel-session="{ params, api }">
        <SessionOrMr :panel-id="api.id"><SessionPanel v-if="isSessionRef(params.session)" class="vr-fade-in" :session="params.session" :panel="api" /></SessionOrMr>
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
        <JiraIssueView v-if="typeof params.connector === 'string' && typeof params.key === 'string'" class="vr-fade-in" :connector="params.connector" :issue-key="params.key" @open="(ref) => dock.openIssue(ref.connector, ref.key)" />
      </template>
      <template #panel-conversation="{ params, api }">
        <SessionOrMr :panel-id="api.id"><ConversationView v-if="isHarness(params.harness) && typeof params.id === 'string'" class="vr-fade-in" :harness="params.harness" :session-id="params.id" /></SessionOrMr>
      </template>
      <template #panel-metrics>
        <MetricsPanel class="vr-fade-in" />
      </template>
      <template #panel-markdown="{ params, api }">
        <MarkdownFile v-if="typeof params.path === 'string'" class="vr-fade-in" :path="params.path" :hash="typeof params.hash === 'string' ? params.hash : ''" @open-file="(path, hash) => emit('open-file', path, hash, api.id)" />
      </template>
      <template #defaultTab="{ api }">
        <span class="tab" @dblclick="dock.toggleMaximize(api.id)" @pointerdown="onTabPointerDown($event, api)":data-testid="`${kindOf(api.id)}-tab`" :data-panel-id="api.id" :data-session-id="berth?.sessions.value.get(api.id)?.id" :data-active="api.id === activeKey">
          <FileText v-if="kindOf(api.id) === 'file'" :size="12" />
          <GitPullRequest v-else-if="kindOf(api.id) === 'review'" :size="12" />
          <Network v-else-if="kindOf(api.id) === 'architecture'" :size="12" />
          <Ticket v-else-if="kindOf(api.id) === 'issue'" :size="12" />
          <template v-else-if="kindOf(api.id) === 'conversation'">
            <StatusDot :status="tabDot(api.id)" :unseen="false" />
            <HarnessIcon :harness="tabHarness(api.id)" :working="tabDot(api.id) === 'running'" />
          </template>
          <template v-else-if="tabSession(api.id)">
            <StatusDot :status="sessionTabDot(api.id)" :unseen="tabSession(api.id)?.unseen" data-testid="session-tab-status" />
            <HarnessIcon v-if="tabLinked(api.id)" :harness="tabLinked(api.id)?.harness ?? 'claude'" :working="sessionTabDot(api.id) === 'running'" />
          </template>
          <span class="name">{{ tabSession(api.id)?.name ?? external?.anyByKey.value.get(api.id)?.title ?? api.title }}</span>
          <TabMrBadge v-if="kindOf(api.id) === 'session' || kindOf(api.id) === 'conversation'" :panel="api" />
          <span v-if="isFilePanel(api.id) && isDirtyPath(api.id.slice(FILE_PANEL_PREFIX.length))" class="dirty" :title="t('files.editor.dirty')" data-testid="file-tab-dirty" />
          <button class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="closeTab(api)"><X :size="12" /></button>
        </span>
      </template>
      <template #watermark>
        <p class="watermark">{{ t('workbench.pick') }}</p>
      </template>
    </DockviewVue>
  </div>
</template>

<style scoped>
.dock {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.dock :deep(.dockview) {
  height: 100%;
}

:root[data-vr-viewport='phone'] .dock :deep(.dv-tabs-and-actions-container) {
  display: none;
}

.dock :deep(.dockview-theme-light) {
  color-scheme: inherit;
  --dv-group-view-background-color: var(--vr-color-surface);
  --dv-tabs-and-actions-container-background-color: var(--vr-color-surface-muted);
  --dv-tabs-and-actions-container-font-size: var(--vr-font-size-xs);
  --dv-activegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-activegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-inactivegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-inactivegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-activegroup-visiblepanel-tab-color: var(--vr-color-text);
  --dv-activegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-visiblepanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-tab-divider-color: var(--vr-color-border);
  --dv-separator-border: var(--vr-color-border);
  --dv-paneview-header-border-color: var(--vr-color-border);
  --dv-icon-hover-background-color: var(--vr-color-nav-hover);
  --dv-drag-over-background-color: var(--vr-color-nav-active);
  --dv-scrollbar-background-color: var(--vr-color-nav-active);
  --dv-tabs-container-scrollbar-color: var(--vr-color-nav-active);
}

.dock[data-tabs='scroll'] :deep(.dv-scrollable .dv-scrollbar-horizontal) {
  height: 6px;
  background-color: var(--dv-scrollbar-background-color);
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-and-actions-container) {
  height: auto;
  min-height: var(--dv-tabs-and-actions-container-height);
  align-items: flex-start;
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-container) {
  flex-wrap: wrap;
  height: auto;
  overflow: visible;
  align-content: flex-start;
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-container .dv-tab) {
  height: var(--dv-tabs-and-actions-container-height);
}

.dock[data-tabs='wrap'] :deep(.dv-scrollbar),
.dock[data-tabs='wrap'] :deep(.dv-tabs-overflow-dropdown-root) {
  display: none;
}

.tab {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 100%;
  padding: 0 var(--vr-space-1) 0 var(--vr-space-3);
}

.tab[data-dragging='true'] {
  opacity: 0.6;
}

.tab[data-drop-side='before'] {
  box-shadow: inset 2px 0 0 var(--vr-color-accent);
}

.tab[data-drop-side='after'] {
  box-shadow: inset -2px 0 0 var(--vr-color-accent);
}

.name {
  max-width: 200px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.dirty {
  flex: none;
  width: 7px;
  height: 7px;
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  visibility: hidden;
}

.tab:hover .close,
.tab[data-active='true'] .close {
  visibility: visible;
}

.close:hover {
  background: var(--vr-color-nav-hover);
}

.watermark {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--vr-color-text-muted);
}
</style>
