<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { onBeforeUnmount, shallowRef } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewPanelApi, type DockviewReadyEvent } from 'dockview-vue';
import JiraIssueView from '../../jira/ui/JiraIssueView.vue';
import ArchitectureView from '../../../features/architecture/ui/ArchitectureView.vue';
import ChangeRequestView from '../../change-request/ui/ChangeRequestView.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import MarkdownFile from '../../markdown-file/ui/MarkdownFile.vue';
import ConversationView from '../../conversation/ui/ConversationView.vue';
import { useDock } from '../../../shared/model/dock';
import type { Harness } from '../../../entities/session/model/external';
import { isSessionRef } from '../../../entities/session/model/berth';
import SessionPanel from '../../terminal/ui/SessionPanel.vue';
import MetricsPanel from '../../metrics/ui/MetricsPanel.vue';
import { isDesktop } from '../../../bridge';
import { useSettings } from '../../../shared/config/settings/store';
import { startTabDrag, trackTabOrder } from '../lib/tabOrder';
import DockTab from './DockTab.vue';
import ProjectTabRows from './ProjectTabRows.vue';
import SessionViews from '../../../features/session-view-switch/ui/SessionViews.vue';

defineProps<{ activeKey: string | null }>();
const emit = defineEmits<{ ready: [event: DockviewReadyEvent]; 'open-file': [path: string, hash: string, from: string] }>();

const dock = useDock();
const isHarness = (value: unknown): value is Harness => value === 'claude' || value === 'codex';
const { values } = useSettings();
const subscriptions: { dispose(): void }[] = [];
let dockApi: DockviewApi | null = null;
const readyApi = shallowRef<DockviewApi | null>(null);

function onReady(event: DockviewReadyEvent) {
  const api = event.api;
  dockApi = api;
  readyApi.value = api;
  subscriptions.push(...trackTabOrder(api));
  emit('ready', event);
}

const str = (value: unknown) => (typeof value === 'string' ? value : '');

function onTabPointerDown(event: PointerEvent, panel: DockviewPanelApi) {
  if (isDesktop) startTabDrag(event, panel, dockApi);
}

onBeforeUnmount(() => {
  for (const subscription of subscriptions) subscription.dispose();
  dockApi = null;
  readyApi.value = null;
});
</script>

<template>
  <div class="dock" :data-tabs="values['workbench.tabs']">
    <ProjectTabRows v-if="values['workbench.tabs'] === 'projects'" :api="readyApi" />
    <DockviewVue class="dockview" :theme="themeLight" :disable-dnd="isDesktop" @ready="onReady">
      <template #panel-session="{ params, api }">
        <SessionViews :panel-id="api.id"><SessionPanel v-if="isSessionRef(params.session)" class="vr-fade-in" :session="params.session" :panel="api" /></SessionViews>
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
        <SessionViews :panel-id="api.id"><ConversationView v-if="isHarness(params.harness) && typeof params.id === 'string'" class="vr-fade-in" :harness="params.harness" :session-id="params.id" /></SessionViews>
      </template>
      <template #panel-metrics>
        <MetricsPanel class="vr-fade-in" />
      </template>
      <template #panel-markdown="{ params, api }">
        <MarkdownFile v-if="typeof params.path === 'string'" class="vr-fade-in" :path="params.path" :hash="typeof params.hash === 'string' ? params.hash : ''" @open-file="(path, hash) => emit('open-file', path, hash, api.id)" />
      </template>
      <template #defaultTab="{ api }">
        <DockTab :panel="api" :active="api.id === activeKey" @dblclick="dock.toggleMaximize(api.id)" @pointerdown="onTabPointerDown($event, api)" />
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

.dock[data-tabs='projects'] {
  display: flex;
  flex-direction: column;
}

.dock[data-tabs='projects'] :deep(.dockview) {
  flex: 1;
  height: auto;
  min-height: 0;
}

.dock[data-tabs='projects'] :deep(.dv-tabs-and-actions-container) {
  display: none;
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

.watermark {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--vr-color-text-muted);
}
</style>
