<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { FileText, GitPullRequest, Network, Search, Ticket, X } from '../../../shared/ui/icons';
import { isDirtyPath, settleDirty } from '../../../entities/document';
import { t } from '../../../shared/lib/i18n/i18n';
import { METRICS_PANEL, archiveAgent, archiveExternal, deleteAgent, endAgent, useAction, useBerth, isExternalPanel, stopAgent, useExternalSessions, useSessionStates, StatusDot } from '../../../entities/session';
import { VButton, VContextMenu, VDialog, VMessage, VMiddleTruncate, vTooltip } from '../../../shared/ui/kit';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { useSettings } from '../../../shared/config/settings/store';
import { FILE_PANEL_PREFIX, isFilePanel } from '../../../shared/lib/markdownFiles';
import { panelTitle } from '../../../shared/model/dock';
import { SessionViewSwitch } from '../../../features/session-view-switch';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const berth = useBerth();
const { values } = useSettings();
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
const tabName = () => session(props.panel.id)?.name ?? external?.anyByKey.value.get(props.panel.id)?.title ?? panelTitle(props.panel.title, props.panel.getParameters());

async function close() {
  if (isFilePanel(props.panel.id) && !(await settleDirty([props.panel.id.slice(FILE_PANEL_PREFIX.length)]))) return;
  const node = session(props.panel.id);
  props.panel.close();
  if (node?.alive && 'Agent' in node.ref) void stopAgent(node.ref.Agent);
}

function closeAndEnd() {
  const node = session(props.panel.id);
  props.panel.close();
  if (node && 'Agent' in node.ref) void endAgent(node.ref.Agent);
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

const isAgent = () => {
  const node = session(props.panel.id);
  return !!node && 'Agent' in node.ref;
};

const confirmingDelete = shallowRef(false);
const deleting = useAction();

async function confirmDelete() {
  const node = session(props.panel.id);
  if (!node || !('Agent' in node.ref)) return;
  const agent = node.ref.Agent;
  if (await deleting.run(() => deleteAgent(agent))) confirmingDelete.value = false;
}

const tab = useTemplateRef<HTMLElement>('tab');

function cancelDelete() {
  confirmingDelete.value = false;
  deleting.clear();
  void nextTick(() => tab.value?.focus());
}

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const menuItems = computed(() => [
  isAgent() ? { key: 'close', label: t('agents.session.end'), command: closeAndEnd } : { key: 'close', label: t('common.close'), command: () => void close() },
  { key: 'archive', label: t('agents.session.archive'), command: closeAndArchive },
  ...(isAgent() ? [{ separator: true }, { key: 'delete', label: t('agents.session.deleteForever'), command: () => (confirmingDelete.value = true) }] : []),
]);

const menuOpen = shallowRef(false);

function showMenuUnderCloseButton() {
  const anchor = tab.value?.querySelector('.close')?.getBoundingClientRect();
  if (!anchor) return;
  menu.value?.show(new MouseEvent('click', { clientX: anchor.left, clientY: anchor.bottom + 2 }));
}

function onMenuHide() {
  if (!confirmingDelete.value) tab.value?.querySelector<HTMLElement>('.close')?.focus();
  menuOpen.value = false;
}

function onClose() {
  if (!archivable()) void close();
  else if (values.value['sessions.archiveOnClose']) (isAgent() ? closeAndEnd : closeAndArchive)();
  else showMenuUnderCloseButton();
}
</script>

<template>
  <span ref="tab" v-tooltip="tabName()" class="tab status-host" tabindex="-1" :data-testid="`${kindOf(panel)}-tab`" :data-panel-id="panel.id" :data-session-id="session(panel.id)?.id" :data-active="active" :data-menu-open="menuOpen" @auxclick.middle="onClose">
    <Search v-if="kindOf(panel) === 'search'" :size="12" />
    <FileText v-else-if="kindOf(panel) === 'file'" :size="12" />
    <GitPullRequest v-else-if="kindOf(panel) === 'review'" :size="12" />
    <Network v-else-if="kindOf(panel) === 'architecture'" :size="12" />
    <Ticket v-else-if="kindOf(panel) === 'issue'" :size="12" />
    <template v-else-if="kindOf(panel) === 'conversation'">
      <StatusDot :status="conversationDot(panel.id)" :unseen="false" />
    </template>
    <template v-else-if="session(panel.id)">
      <StatusDot :status="states.get(panel.id)?.status ?? session(panel.id)?.status" :unseen="states.get(panel.id)?.unseen ?? session(panel.id)?.unseen" :alive="states.get(panel.id)?.alive ?? session(panel.id)?.alive" data-testid="session-tab-status" />
    </template>
    <VMiddleTruncate class="name" :text="tabName()" />
    <SessionViewSwitch v-if="kindOf(panel) === 'conversation'" :panel="panel" :active="active" />
    <span v-if="isFilePanel(panel.id) && isDirtyPath(panel.id.slice(FILE_PANEL_PREFIX.length))" v-tooltip="t('files.editor.dirty')" class="dirty" data-testid="file-tab-dirty" />
    <VButton v-tooltip="t('common.close')" class="close" text severity="secondary" size="small" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="onClose">
      <template #icon><X :size="12" /></template>
    </VButton>
    <VContextMenu ref="menu" :model="menuItems" @show="menuOpen = true" @hide="onMenuHide">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :class="{ danger: item.key === 'delete' }" :data-testid="`session-tab-close-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <VDialog v-if="confirmingDelete" :visible="true" modal dismissable-mask :header="t('agents.session.confirmDeleteLabel')" :style="{ width: '26rem', maxWidth: '95vw' }" data-testid="session-delete-confirm" @update:visible="cancelDelete">
      <p class="confirm">{{ t((states.get(panel.id)?.alive ?? session(panel.id)?.alive) === false ? 'agents.session.confirmDeleteStopped' : 'agents.session.confirmDelete', { name: session(panel.id)?.name ?? panel.title ?? panel.id }) }}</p>
      <VMessage v-if="deleting.error.value" severity="error" size="small" data-testid="session-error">{{ deleting.error.value }}</VMessage>
      <DialogActions class="actions" :confirm-label="t('agents.session.delete')" danger focus="cancel" :busy="deleting.busy.value" cancel-testid="session-delete-no" confirm-testid="session-delete-yes" @cancel="cancelDelete" @confirm="confirmDelete" />
    </VDialog>
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

.tab:focus {
  outline: none;
}

.tab:focus-visible {
  outline: 2px solid var(--vr-color-focus);
  outline-offset: -2px;
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='running'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-running);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='needs_feedback'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-feedback);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='limit'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-limit);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='terminated'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-crashed);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='finished'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-finished);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='finished'][data-unseen='true'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-unseen);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='fresh'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-fresh);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='stopped'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-stopped);
}

.tab[data-active='true']:has(:deep(.status-dot[data-status='disconnected'])),
.tab[data-active='true']:has(:deep(.status-dot[data-status='shell'])),
.tab[data-active='true']:has(:deep(.status-dot[data-status='closed'])) {
  box-shadow: inset 0 -2px 0 var(--vr-status-idle);
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
  flex: none;
  width: 18px;
  height: 18px;
  visibility: hidden;
}

.tab:hover .close,
.tab[data-active='true'] .close,
.tab[data-menu-open='true'] .close {
  visibility: visible;
}

.confirm {
  margin: 0 0 var(--vr-space-3);
  overflow-wrap: anywhere;
}

.danger {
  color: var(--vr-color-danger);
}

.actions {
  margin-top: var(--vr-space-3);
}
</style>
