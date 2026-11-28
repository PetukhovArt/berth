<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { Ellipsis, MessageSquare } from '../../../shared/ui/icons';
import { VButton, VContextMenu, VInputText, VMessage, VMiddleTruncate, VRowButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { canContinueAsAgent, continueTerminalInDock, sessionStateOf, type ExternalSession, archiveAgent, closeTerminal, deleteAgent, renameSession, restartAgent, setAgentBackend, otherBackend, useBerth, applyRouteSuggestion, entitySessionRef, useSessionDock, type SessionNode, isStopped, useAction, useHollow, StatusDot } from '../../../entities/session';
import { useProviders } from '../../../shared/api/providers';
import { MoveAgentDialog } from '../../../features/move-agent';
import { routeRuleLabel } from '../../../features/edit-router';
import SessionDeleteConfirm from './SessionDeleteConfirm.vue';
const props = defineProps<{ session: SessionNode; active: boolean; linked?: ExternalSession }>();
const emit = defineEmits<{ open: [split: boolean]; 'open-conversation': [] }>();

const state = computed(() => sessionStateOf(props.session.key));
const status = computed(() => state.value?.status ?? props.session.status);
const unseen = computed(() => state.value?.unseen ?? props.session.unseen);
const alive = computed(() => state.value?.alive ?? props.session.alive);
const stopped = computed(() => isStopped(status.value, unseen.value, alive.value));
const hollow = useHollow();
const label = computed(() => t(hollow.value ? `agents.status.${hollow.value}` : stopped.value ? 'agents.status.stopped' : `agents.status.${status.value}`));

type Mode = 'idle' | 'renaming' | 'confirming-delete';
type MenuAction = { key: string; label: string; command: () => void; testid?: string };

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const row = useTemplateRef<HTMLElement>('row');
const mode = shallowRef<Mode>('idle');
const draft = shallowRef('');
const action = useAction();
const moving = shallowRef(false);
const dock = useSessionDock();
const suggestionText = computed(() => {
  const route = props.session.suggestion?.action;
  if (!route) return '';
  if (route.type === 'switch_model') return t('agents.route.suggestSwitch', { model: route.model ?? t('agents.route.defaultModel') });
  return t('agents.route.suggestContinue', { kind: route.model ? `${route.kind}:${route.model}` : route.kind });
});

function applySuggestion() {
  const session = props.session.ref;
  const route = props.session.suggestion?.action;
  if (!('Agent' in session) || !route) return;
  void action.run(async () => {
    const created = entitySessionRef(await applyRouteSuggestion(session.Agent, route));
    if (created) dock.openWhenListed(created);
  });
}

const movableAgent = computed(() => ('Agent' in props.session.ref ? props.session.ref.Agent : null));
const providers = useProviders();
const otherSide = computed(() => {
  const agent = movableAgent.value ? useBerth()?.entities.value.agents.get(movableAgent.value) : undefined;
  return agent ? otherBackend(agent, providers.value) : null;
});

const menuItems = computed<MenuAction[]>(() => {
  const session = props.session.ref;
  if ('Terminal' in session)
    return [
      ...(props.linked ? [{ key: 'conversation', label: t('workbench.external.open'), command: () => emit('open-conversation') }] : []),
      ...(canContinueAsAgent(props.linked) ? [{ key: 'continue-as-agent', label: t('workbench.external.continueAsAgent'), command: continueAsAgent, testid: 'session-continue-as-agent' }] : []),
      { key: 'rename', label: t('common.rename'), command: startRename },
      { key: 'close', label: t('common.close'), command: () => void action.run(() => closeTerminal(session.Terminal)) },
    ];
  return [
    { key: 'archive', label: t('agents.session.archive'), command: () => void action.run(() => archiveAgent(session.Agent)) },
    { key: 'rename', label: t('common.rename'), command: startRename },
    { key: 'restart', label: t('agents.session.restart'), command: () => void action.run(() => restartAgent(session.Agent)) },
    ...(otherSide.value
      ? [{ key: 'backend', label: t(otherSide.value === 'terminal' ? 'agents.session.openInTerminal' : 'agents.session.backToChat'), command: () => void action.run(() => setAgentBackend(session.Agent, otherSide.value!)), testid: 'session-switch-backend' }]
      : []),
    { key: 'move', label: t('agents.session.move'), command: () => (moving.value = true) },
    { key: 'delete', label: t('agents.session.deleteForever'), command: () => (mode.value = 'confirming-delete') },
  ];
});

function continueAsAgent() {
  const session = props.session.ref;
  const linked = props.linked;
  if (!('Terminal' in session) || !linked) return;
  void action.run(() => continueTerminalInDock(session.Terminal, props.session.key, linked));
}

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
      :aria-label="t('agents.session.newName')"
      data-testid="session-rename-input"
      :readonly="action.busy.value"
      @keydown.enter.prevent="saveRename"
      @keydown.esc.prevent="cancel"
    />
    <template v-else>
      <VRowButton
        v-tooltip="`${session.name} — ${label}`"
        class="row status-host"
        data-testid="session-row"
        :data-session-id="session.id"
        :data-status="hollow ? 'disconnected' : status"
        :data-unseen="!hollow && unseen"
        :data-alive="alive"
        :data-stopped="!hollow && stopped"
        :data-current="active || undefined"
        :data-linked-harness="linked?.harness"
        :data-linked-session-id="linked?.id"
        :data-linked-status="linked?.status"
        @click="emit('open', primaryPressed($event))"
        @contextmenu.prevent="showMenu"
      >
        <template #leading><StatusDot :status="status" :unseen="unseen" :alive="alive" /></template>
        <VMiddleTruncate class="name" :text="session.name" />
        <template #trailing><span class="label" data-testid="session-row-label">{{ label }}</span></template>
      </VRowButton>
      <VButton v-if="linked" v-tooltip="t('workbench.external.open')" class="more conversation" text severity="secondary" size="small" :aria-label="t('workbench.external.open')" data-testid="session-open-conversation" @click="emit('open-conversation')">
        <template #icon><MessageSquare :size="14" /></template>
      </VButton>
      <VButton v-tooltip="t('agents.session.actions')" class="more" text severity="secondary" size="small" :aria-label="t('agents.session.actions')" aria-haspopup="menu" data-testid="session-menu" @click="showMenu">
        <template #icon><Ellipsis :size="14" /></template>
      </VButton>
      <VButton
        v-if="session.suggestion"
        v-tooltip="t('agents.route.suggestTitle', { rule: routeRuleLabel(session.suggestion.rule) })"
        class="suggestion"
        text
        severity="secondary"
        size="small"
        :label="suggestionText"
        :disabled="action.busy.value"
        data-testid="session-route-suggestion"
        @click="applySuggestion"
      />
    </template>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="item.testid ?? `session-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <SessionDeleteConfirm v-if="mode === 'confirming-delete'" :name="session.name" :alive="alive !== false" :busy="action.busy.value" @confirm="confirmDelete" @cancel="cancel" />
    <MoveAgentDialog v-if="moving && movableAgent" :agent="movableAgent" @moved="moving = false" @close="moving = false" />
    <VMessage v-if="action.error.value" class="row-error" severity="error" size="small" data-testid="session-error">{{ action.error.value }}</VMessage>
  </div>
</template>

<style scoped>
.session {
  position: relative;
}

.session[data-busy='true'] .row {
  opacity: 0.6;
}

.row[data-current] {
  color: var(--vr-color-text);
}

.label {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.session:hover .label,
.session:focus-within .label {
  visibility: hidden;
}

.row[data-stopped='true']:not([data-current]) .name {
  color: var(--vr-color-text-subtle);
}

.row[data-status='running'] .label {
  color: var(--vr-status-live);
}

.row[data-status='fresh'] .label {
  color: var(--vr-status-fresh);
}

.row[data-status='finished'] .label {
  color: var(--vr-status-finished);
}

.row[data-stopped='true'] .label {
  color: var(--vr-status-stopped);
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
  width: 22px;
  height: 28px;
  opacity: 0;
}

.more.conversation {
  right: calc(var(--vr-space-1) + 22px);
}

.session:hover .more,
.more:focus-visible {
  opacity: 1;
}

.rename {
  margin-left: var(--vr-space-4);
  width: calc(100% - var(--vr-space-4));
}

.suggestion {
  display: flex;
  margin: 0 0 var(--vr-space-1) calc(var(--vr-space-5) + 19px);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-status-limit);
}

:root[data-vr-viewport='phone'] .session {
  margin-bottom: var(--vr-space-1);
}

:root[data-vr-viewport='phone'] .row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0 var(--vr-space-3);
  align-items: center;
  min-height: 56px;
  line-height: 1.3;
  padding: var(--vr-space-2) var(--vr-space-3);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

:root[data-vr-viewport='phone'] .row > :deep(.leading) {
  grid-row: 1 / span 2;
  align-self: center;
}

:root[data-vr-viewport='phone'] .row > :deep(.main),
:root[data-vr-viewport='phone'] .row > :deep(.trailing) {
  grid-column: 2;
}

:root[data-vr-viewport='phone'] .row .name {
  font-size: var(--vr-font-size-title);
  font-weight: 500;
}

:root[data-vr-viewport='phone'] .row[data-stopped='true'] {
  opacity: 0.72;
}

:root[data-vr-viewport='phone'] .row[data-current] {
  background: var(--vr-color-nav-active);
}

.row-error {
  margin: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
}

</style>
