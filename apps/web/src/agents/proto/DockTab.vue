<script setup lang="ts">
import type { DockviewPanelApi } from 'dockview-vue';
import { FileText, GitPullRequest, Network, Ticket, X } from 'lucide-vue-next';
import { isDirtyPath, settleDirty } from '../../files/documents';
import { t } from '../../shared/lib/i18n/i18n';
import { METRICS_PANEL } from '../../shared/model/dock';
import { useBerth } from '../berth';
import { externalDot, isExternalPanel, sessionDot, useExternalSessions, type Harness } from '../external';
import HarnessIcon from '../HarnessIcon.vue';
import StatusDot from '../StatusDot.vue';
import { FILE_PANEL_PREFIX, isFilePanel } from '../../shared/lib/markdownFiles';
import SessionViewSwitch from './SessionViewSwitch.vue';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const berth = useBerth();
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

const harness = (id: string): Harness => (id.startsWith('ext:codex:') ? 'codex' : 'claude');
const conversationDot = (id: string) => {
  const session = external?.anyByKey.value.get(id);
  return session ? externalDot(session) : 'closed';
};
const session = (id: string) => berth?.sessions.value.get(id);
const linked = (id: string) => external?.linked.value.get(id);
const sessionTabDot = (id: string) => {
  const node = session(id);
  return node ? sessionDot(node, linked(id)) : undefined;
};

async function close() {
  if (isFilePanel(props.panel.id) && !(await settleDirty([props.panel.id.slice(FILE_PANEL_PREFIX.length)]))) return;
  props.panel.close();
}
</script>

<template>
  <span class="tab status-host" :data-testid="`${kindOf(panel.id)}-tab`" :data-panel-id="panel.id" :data-session-id="session(panel.id)?.id" :data-active="active" @auxclick.middle="close">
    <FileText v-if="kindOf(panel.id) === 'file'" :size="12" />
    <GitPullRequest v-else-if="kindOf(panel.id) === 'review'" :size="12" />
    <Network v-else-if="kindOf(panel.id) === 'architecture'" :size="12" />
    <Ticket v-else-if="kindOf(panel.id) === 'issue'" :size="12" />
    <template v-else-if="kindOf(panel.id) === 'conversation'">
      <StatusDot :status="conversationDot(panel.id)" :unseen="false" />
      <HarnessIcon :harness="harness(panel.id)" :working="conversationDot(panel.id) === 'running'" />
    </template>
    <template v-else-if="session(panel.id)">
      <StatusDot :status="sessionTabDot(panel.id)" :unseen="session(panel.id)?.unseen" data-testid="session-tab-status" />
      <HarnessIcon v-if="linked(panel.id)" :harness="linked(panel.id)?.harness ?? 'claude'" :working="sessionTabDot(panel.id) === 'running'" />
    </template>
    <span class="name">{{ session(panel.id)?.name ?? external?.anyByKey.value.get(panel.id)?.title ?? panel.title }}</span>
    <SessionViewSwitch v-if="kindOf(panel.id) === 'session' || kindOf(panel.id) === 'conversation'" :panel="panel" :active="active" />
    <span v-if="isFilePanel(panel.id) && isDirtyPath(panel.id.slice(FILE_PANEL_PREFIX.length))" class="dirty" :title="t('files.editor.dirty')" data-testid="file-tab-dirty" />
    <button class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="close"><X :size="12" /></button>
  </span>
</template>

<style scoped>
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
</style>
