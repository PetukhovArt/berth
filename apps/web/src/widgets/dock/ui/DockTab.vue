<script setup lang="ts">
import { useTemplateRef } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { FileText, GitPullRequest, Network, Ticket, X } from '../../../shared/ui/icons';
import { isDirtyPath, settleDirty } from '../../../entities/document';
import { t } from '../../../shared/lib/i18n/i18n';
import { METRICS_PANEL, archiveAgent, archiveExternal, useBerth, isExternalPanel, stopAgent, useExternalSessions, useSessionStates, StatusDot } from '../../../entities/session';
import { VContextMenu } from '../../../shared/ui/kit';
import { useSettings } from '../../../shared/config/settings/store';
import { FILE_PANEL_PREFIX, isFilePanel } from '../../../shared/lib/markdownFiles';
import { panelTitle } from '../../../shared/model/dock';
import { SessionViewSwitch } from '../../../features/session-view-switch';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const berth = useBerth();
const { values } = useSettings();
const external = useExternalSessions();

function kindOf(id: string): 'metrics' | 'file' | 'review' | 'architecture' | 'issue' | 'conversation' | 'session' {
  if (id === METRICS_PANEL) return 'metrics';
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

function closeAndArchive() {
  const node = session(props.panel.id);
  const conversation = external?.anyByKey.value.get(props.panel.id);
  props.panel.close();
  if (node && 'Agent' in node.ref) void archiveAgent(node.ref.Agent);
  else if (conversation) void archiveExternal(conversation);
}

const archivable = () => {
  const node = session(props.panel.id);
  return node ? 'Agent' in node.ref : !!external?.anyByKey.value.get(props.panel.id);
};

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const menuItems = [
  { key: 'close', label: t('common.close'), command: () => void close() },
  { key: 'archive', label: t('agents.session.closeAndArchive'), command: closeAndArchive },
];

function onClose(event: MouseEvent) {
  if (!archivable()) void close();
  else if (values.value['sessions.archiveOnClose']) closeAndArchive();
  else menu.value?.show(event);
}
</script>

<template>
  <span class="tab status-host" :data-testid="`${kindOf(panel.id)}-tab`" :data-panel-id="panel.id" :data-session-id="session(panel.id)?.id" :data-active="active" @auxclick.middle="onClose">
    <FileText v-if="kindOf(panel.id) === 'file'" :size="12" />
    <GitPullRequest v-else-if="kindOf(panel.id) === 'review'" :size="12" />
    <Network v-else-if="kindOf(panel.id) === 'architecture'" :size="12" />
    <Ticket v-else-if="kindOf(panel.id) === 'issue'" :size="12" />
    <template v-else-if="kindOf(panel.id) === 'conversation'">
      <StatusDot :status="conversationDot(panel.id)" :unseen="false" />
    </template>
    <template v-else-if="session(panel.id)">
      <StatusDot :status="states.get(panel.id)?.status ?? session(panel.id)?.status" :unseen="states.get(panel.id)?.unseen ?? session(panel.id)?.unseen" data-testid="session-tab-status" />
    </template>
    <span class="name">{{ session(panel.id)?.name ?? external?.anyByKey.value.get(panel.id)?.title ?? panelTitle(panel.title, panel.getParameters()) }}</span>
    <SessionViewSwitch v-if="kindOf(panel.id) === 'conversation'" :panel="panel" :active="active" />
    <span v-if="isFilePanel(panel.id) && isDirtyPath(panel.id.slice(FILE_PANEL_PREFIX.length))" class="dirty" :title="t('files.editor.dirty')" data-testid="file-tab-dirty" />
    <UiButton class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="onClose"><X :size="12" /></UiButton>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`session-tab-close-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
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
