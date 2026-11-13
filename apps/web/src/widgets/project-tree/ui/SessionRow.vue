<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { Ellipsis, MessageSquare } from '../../../shared/ui/icons';
import { VButton, VContextMenu, VInputText } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { externalStatusLabel, sessionDot, type ExternalSession, archiveAgent, closeTerminal, deleteAgent, renameSession, restartAgent, type SessionNode, type SessionStatus, useAction, StatusIcon } from '../../../entities/session';
import { MoveAgentDialog } from '../../../features/move-agent';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ session: SessionNode; active: boolean; linked?: ExternalSession }>();
const emit = defineEmits<{ open: [split: boolean]; 'open-conversation': [] }>();

function statusLabel(status: SessionStatus): string {
  return t(`agents.status.${status}`);
}

type Mode = 'idle' | 'renaming' | 'confirming-delete';
type MenuAction = { key: string; label: string; command: () => void };

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const row = useTemplateRef<HTMLElement>('row');
const mode = shallowRef<Mode>('idle');
const draft = shallowRef('');
const action = useAction();
const moving = shallowRef(false);
const movableAgent = computed(() => ('Agent' in props.session.ref ? props.session.ref.Agent : null));

const menuItems = computed<MenuAction[]>(() => {
  const session = props.session.ref;
  if ('Terminal' in session)
    return [
      ...(props.linked ? [{ key: 'conversation', label: t('workbench.external.open'), command: () => emit('open-conversation') }] : []),
      { key: 'rename', label: t('common.rename'), command: startRename },
      { key: 'close', label: t('common.close'), command: () => void action.run(() => closeTerminal(session.Terminal)) },
    ];
  return [
    { key: 'archive', label: t('agents.session.archive'), command: () => void action.run(() => archiveAgent(session.Agent)) },
    { key: 'rename', label: t('common.rename'), command: startRename },
    { key: 'restart', label: t('agents.session.restart'), command: () => void action.run(() => restartAgent(session.Agent)) },
    { key: 'move', label: t('agents.session.move'), command: () => (moving.value = true) },
    { key: 'delete', label: t('common.delete'), command: () => (mode.value = 'confirming-delete') },
  ];
});

function showMenu(event: MouseEvent) {
  menu.value?.show(event);
}

async function startRename() {
  draft.value = props.session.name;
  action.clear();
  mode.value = 'renaming';
  await nextTick();
  const element = row.value?.querySelector('input.rename');
  if (element instanceof HTMLInputElement) element.select();
}

async function saveRename() {
  const name = draft.value.trim();
  if (!name || name === props.session.name) {
    mode.value = 'idle';
    return;
  }
  if (await action.run(() => renameSession(props.session.ref, name))) mode.value = 'idle';
}

function cancel() {
  mode.value = 'idle';
  action.clear();
}

async function confirmDelete() {
  const session = props.session.ref;
  if (!('Agent' in session)) return;
  if (await action.run(() => deleteAgent(session.Agent))) mode.value = 'idle';
}
</script>

<template>
  <div ref="row" class="session" :data-busy="action.busy.value">
    <VInputText
      v-if="mode === 'renaming'"
      v-model="draft"
      class="rename"
      size="small"
      :aria-label="t('agents.session.newName')"
      data-testid="session-rename-input"
      :readonly="action.busy.value"
      @keydown.enter.prevent="saveRename"
      @keydown.esc.prevent="cancel"
    />
    <template v-else>
      <UiButton
        class="row status-host"
        data-testid="session-row"
        :data-session-id="session.id"
        :data-status="session.status"
        :data-unseen="session.unseen"
        :data-active="active"
        :data-linked-harness="linked?.harness"
        :data-linked-session-id="linked?.id"
        :data-linked-status="linked?.status"
        :title="linked ? `${session.name}\n${externalStatusLabel(linked)}` : `${session.name} — ${statusLabel(session.status)}`"
        @click="emit('open', primaryPressed($event))"
        @contextmenu.prevent="showMenu"
      >
        <StatusIcon :status="sessionDot(session, linked)" :unseen="session.unseen" />
        <span class="name">{{ session.name }}</span>
        <span class="label">{{ linked ? externalStatusLabel(linked) : statusLabel(session.status) }}</span>
      </UiButton>
      <UiButton v-if="linked" class="more conversation" :title="t('workbench.external.open')" :aria-label="t('workbench.external.open')" data-testid="session-open-conversation" @click="emit('open-conversation')"><MessageSquare :size="14" /></UiButton>
      <UiButton class="more" :title="t('agents.session.actions')" :aria-label="t('agents.session.actions')" aria-haspopup="menu" data-testid="session-menu" @click="showMenu"><Ellipsis :size="14" /></UiButton>
    </template>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`session-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <div v-if="mode === 'confirming-delete'" class="confirm" role="group" :aria-label="t('agents.session.confirmDeleteLabel')" data-testid="session-delete-confirm">
      <span>{{ t('agents.session.confirmDelete', { name: session.name }) }}</span>
      <VButton :label="t('common.delete')" size="small" severity="danger" :loading="action.busy.value" data-testid="session-delete-yes" @click="confirmDelete" />
      <VButton :label="t('common.cancel')" size="small" text data-testid="session-delete-no" @click="cancel" />
    </div>
    <MoveAgentDialog v-if="moving && movableAgent" :agent="movableAgent" @moved="moving = false" @close="moving = false" />
    <p v-if="action.error.value" class="row-error" role="alert" data-testid="session-error">{{ action.error.value }}</p>
  </div>
</template>

<style scoped>
.session {
  position: relative;
}

.session[data-busy='true'] .row {
  opacity: 0.6;
}

.row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  height: 28px;
  padding: 0 4px 0 var(--vr-space-5);
  font: inherit;
  font-size: 13px;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.row[data-active='true'] {
  color: var(--vr-color-text);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.label {
  flex: none;
  font-size: 11.5px;
  color: var(--vr-color-text-muted);
}

.session:hover .label,
.session:focus-within .label {
  visibility: hidden;
}

.row[data-status='needs_feedback'] .label {
  color: var(--vr-status-feedback);
}

.row[data-status='terminated'] .label {
  color: var(--vr-status-crashed);
}

.more {
  position: absolute;
  top: 0;
  right: var(--vr-space-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  opacity: 0;
}

.more.conversation {
  right: calc(var(--vr-space-1) + 22px);
}

.session:hover .more,
.more:focus-visible {
  opacity: 1;
}

.more:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.rename {
  margin-left: var(--vr-space-4);
  width: calc(100% - var(--vr-space-4));
}

.confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1) var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-xs);
}

.row-error {
  margin: 0;
  padding: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
}

</style>
