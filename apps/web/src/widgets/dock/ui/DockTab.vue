<script setup lang="ts">
import type { DockviewPanelApi } from 'dockview-vue';
import { FileText, GitPullRequest, Network, Search, Ticket, X } from '../../../shared/ui/icons';
import { isDirtyPath, settleDirty } from '../../../entities/document';
import { t } from '../../../shared/lib/i18n/i18n';
import { METRICS_PANEL, useBerth, isExternalPanel, stopAgent, useExternalSessions, useSessionStates, StatusDot } from '../../../entities/session';
import { FILE_PANEL_PREFIX, isFilePanel } from '../../../shared/lib/markdownFiles';
import { panelTitle } from '../../../shared/model/dock';
import { SessionViewSwitch } from '../../../features/session-view-switch';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const berth = useBerth();
const external = useExternalSessions();

function kindOf({ id, component }: DockviewPanelApi): 'metrics' | 'search' | 'file' | 'review' | 'architecture' | 'issue' | 'conversation' | 'session' {
  if (id === METRICS_PANEL) return 'metrics';
  if (component === 'search') return 'search';
  if (id.startsWith('arch:')) return 'architecture';
  if (isExternalPanel(id)) return 'conversation';
  if (isFilePanel(id)) return 'file';
  if (id.startsWith('mr:')) return 'review';
  if (id.startsWith('jira:')) return 'issue';
  return 'session';
}

const states = useSessionStates();
const conversationDot = (id: string) => states.value.get(id)?.status ?? external?.anyByKey.value.get(id)?.status ?? 'closed';
const session = (id: string) => berth?.sessions.value.get(id);

async function close() {
  if (isFilePanel(props.panel.id) && !(await settleDirty([props.panel.id.slice(FILE_PANEL_PREFIX.length)]))) return;
  const node = session(props.panel.id);
  props.panel.close();
  if (node?.alive && 'Agent' in node.ref) void stopAgent(node.ref.Agent);
}
</script>

<template>
  <span class="tab status-host" :data-testid="`${kindOf(panel)}-tab`" :data-panel-id="panel.id" :data-session-id="session(panel.id)?.id" :data-active="active" @auxclick.middle="close">
    <Search v-if="kindOf(panel) === 'search'" :size="12" />
    <FileText v-else-if="kindOf(panel) === 'file'" :size="12" />
    <GitPullRequest v-else-if="kindOf(panel) === 'review'" :size="12" />
    <Network v-else-if="kindOf(panel) === 'architecture'" :size="12" />
    <Ticket v-else-if="kindOf(panel) === 'issue'" :size="12" />
    <template v-else-if="kindOf(panel) === 'conversation'">
      <StatusDot :status="conversationDot(panel.id)" :unseen="false" />
    </template>
    <template v-else-if="session(panel.id)">
      <StatusDot :status="states.get(panel.id)?.status ?? session(panel.id)?.status" :unseen="states.get(panel.id)?.unseen ?? session(panel.id)?.unseen" data-testid="session-tab-status" />
    </template>
    <span class="name">{{ session(panel.id)?.name ?? external?.anyByKey.value.get(panel.id)?.title ?? panelTitle(panel.title, panel.getParameters()) }}</span>
    <SessionViewSwitch v-if="kindOf(panel) === 'conversation'" :panel="panel" :active="active" />
    <span v-if="isFilePanel(panel.id) && isDirtyPath(panel.id.slice(FILE_PANEL_PREFIX.length))" class="dirty" :title="t('files.editor.dirty')" data-testid="file-tab-dirty" />
    <UiButton class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="close"><X :size="12" /></UiButton>
  </span>
</template>

<style scoped>
.tab {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  min-width: 0;
  height: 100%;
  box-sizing: border-box;
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
  flex: 1 1 auto;
  min-width: 0;
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
  flex: none;
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
</style>
