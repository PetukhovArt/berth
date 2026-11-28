<script setup lang="ts">
import { computed, useTemplateRef, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { isChat, latestModel, modelLabel, sessionStateOf, StatusIcon, useAgentMessages, useBerth, useExternalSessions, type AgentKind, type SessionStatus } from '../../../entities/session';
import { knownMr, lookupMr, numberLabel } from '../../../entities/merge-request';
import { SessionViewSwitch } from '../../../features/session-view-switch';
import { GitStepButton } from '../../../features/git-step';
import TaskNoteButton from './TaskNoteButton.vue';
import CheckpointsButton from './CheckpointsButton.vue';
import RouteButton from './RouteButton.vue';
import BackendSwitchButton from './BackendSwitchButton.vue';
import ContinueAsAgentButton from './ContinueAsAgentButton.vue';
import AgentLogButton from './AgentLogButton.vue';
import { OverlapChip } from '../../../features/overlaps';
import { ShareButton } from '../../../features/share-session';
import { VButton, VMenu, VMiddleTruncate, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { ChevronLeft, Ellipsis } from '../../../shared/ui/icons';
import { useDock } from '../../../shared/model/dock';
import type { MenuAction } from '../../../shared/lib/menuAction';
import { isDesktop } from '../../../bridge';
import { inspectorShown } from '../../../shared/model/inspector';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();
const { t, formatTime } = useI18n();

const MODEL_TAIL = 40;
const KIND_LABEL: Record<AgentKind, string> = { claude: 'Claude Code', codex: 'Codex', cursor: 'Cursor', opencode: 'OpenCode', grok: 'Grok', acp: 'ACP' };

const berth = useBerth();

const node = computed(() => berth?.sessions.value.get(props.panel.id));
const entities = computed(() => berth?.entities.value);
const external = useExternalSessions();
const terminalId = computed(() => {
  const ref = node.value?.ref;
  return ref && 'Terminal' in ref ? ref.Terminal : undefined;
});
const lapsed = computed(() => {
  const conversation = terminalId.value ? external?.linked.value.get(props.panel.id) : undefined;
  return conversation && !conversation.running ? conversation : undefined;
});

const agent = computed(() => {
  const ref = node.value?.ref;
  return ref && 'Agent' in ref ? entities.value?.agents.get(ref.Agent) : undefined;
});
const worktree = computed(() => {
  const ref = node.value?.ref;
  const worktreeId = ref && 'Agent' in ref ? agent.value?.worktree_id : ref && 'Terminal' in ref ? entities.value?.terminals.get(ref.Terminal)?.worktree_id : undefined;
  return worktreeId ? entities.value?.worktrees.get(worktreeId) : undefined;
});
const project = computed(() => (worktree.value ? entities.value?.projects.get(worktree.value.project_id)?.name : undefined));
const mr = computed(() => (worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null));

const state = computed(() => sessionStateOf(props.panel.id));
const status = computed<SessionStatus | undefined>(() => state.value?.status ?? node.value?.status);
const tone = computed(() => {
  if (status.value === 'finished' && state.value?.unseen) return 'unseen';
  if (status.value === 'running' || status.value === 'needs_feedback' || status.value === 'terminated' || status.value === 'limit') return { running: 'running', needs_feedback: 'feedback', terminated: 'crashed', limit: 'limit' }[status.value];
  return 'neutral';
});

const statusLabel = computed(() => {
  if (!status.value) return '';
  const failure = state.value?.providerError;
  return failure ? t(`agents.route.errorKind.${failure.kind}`) : t(`agents.status.${status.value}`);
});
const statusText = computed(() => {
  const until = state.value?.limitUntil;
  return until && statusLabel.value ? `${statusLabel.value} · ${t('agents.metrics.resets', { time: formatTime(until) })}` : statusLabel.value;
});

const place = computed(() => [project.value, worktree.value?.branch].filter(Boolean).join(' › '));
const routedFrom = computed(() => {
  const names: string[] = [];
  const seen = new Set<string>();
  let id = agent.value?.routed_from;
  while (id && !seen.has(id)) {
    seen.add(id);
    const previous = entities.value?.agents.get(id);
    if (!previous) break;
    names.push(`${previous.name} (${KIND_LABEL[previous.kind]})`);
    id = previous.routed_from;
  }
  return names.length ? t('agents.route.routedFrom', { name: names.join(' ← ') }) : undefined;
});
const messages = useAgentMessages(() => agent.value, MODEL_TAIL, () => status.value);
const modelName = computed(() => {
  const id = latestModel(messages.value) ?? agent.value?.model;
  return id ? modelLabel(id) : undefined;
});
const meta = computed(() =>
  [agent.value ? KIND_LABEL[agent.value.kind] : undefined, modelName.value, place.value, mr.value ? numberLabel(mr.value) : undefined, routedFrom.value].filter(Boolean).join(' · '),
);

const phone = useIsPhone();
const { detail } = useDock();
const menu = useTemplateRef<InstanceType<typeof VMenu>>('menu');
const continueAs = useTemplateRef<InstanceType<typeof ContinueAsAgentButton>>('continueAs');
const backend = useTemplateRef<InstanceType<typeof BackendSwitchButton>>('backend');
const route = useTemplateRef<InstanceType<typeof RouteButton>>('route');
const checkpoints = useTemplateRef<InstanceType<typeof CheckpointsButton>>('checkpoints');
const share = useTemplateRef<InstanceType<typeof ShareButton>>('share');
const taskNote = useTemplateRef<InstanceType<typeof TaskNoteButton>>('taskNote');
const agentLog = useTemplateRef<InstanceType<typeof AgentLogButton>>('agentLog');
const gitStep = useTemplateRef<InstanceType<typeof GitStepButton>>('gitStep');
let menuAnchor: HTMLElement | null = null;

function defer(entry: MenuAction, anchor: HTMLElement | null) {
  if (anchor) window.setTimeout(() => entry.run(anchor));
}

const menuGroups = computed<MenuAction[][]>(() =>
  [
    [continueAs.value?.item, backend.value?.item],
    [route.value?.item, checkpoints.value?.item, share.value?.item, taskNote.value?.item, agentLog.value?.item],
    [gitStep.value?.item],
  ].map((group) => group.filter((entry): entry is MenuAction => !!entry)).filter((group) => group.length),
);
const menuModel = computed(() =>
  menuGroups.value.flatMap((group, index) => [
    ...(index ? [{ separator: true }] : []),
    ...group.map((entry) => ({ label: entry.label, testid: entry.testid, disabled: entry.disabled, command: () => defer(entry, menuAnchor) })),
  ]),
);

function openMenu(event: Event) {
  if (event.currentTarget instanceof HTMLElement) menuAnchor = event.currentTarget;
  menu.value?.toggle(event);
}

watch(
  () => [worktree.value?.path, worktree.value?.branch] as const,
  ([path, branch]) => {
    if (path && branch) void lookupMr(path, branch);
  },
  { immediate: true },
);
</script>

<template>
  <header v-if="node" class="session-header" data-testid="session-header">
    <div v-if="phone" class="phone-bar" data-testid="session-header-bar">
      <VButton class="bar-button" text severity="secondary" :aria-label="t('mobile.back')" data-testid="mobile-back" @click="detail = false">
        <template #icon><ChevronLeft :size="22" /></template>
      </VButton>
      <VMiddleTruncate class="title" :text="node.name" data-testid="session-header-title" />
      <span v-if="status" class="pill" :data-tone="tone" data-testid="session-header-status">
        <StatusIcon class="icon-status" :status="status" :unseen="state?.unseen ?? node.unseen" />
        {{ statusLabel }}
      </span>
      <VButton v-if="menuModel.length" class="bar-button" text severity="secondary" :aria-label="t('agents.session.actions')" aria-haspopup="menu" data-testid="session-header-menu" @click="openMenu">
        <template #icon><Ellipsis :size="20" /></template>
      </VButton>
      <VMenu ref="menu" :model="menuModel" :label="t('agents.session.actions')" />
    </div>
    <div v-else class="identity">
      <div class="title-row">
        <span v-tooltip="node.name" class="title" data-testid="session-header-title">{{ node.name }}</span>
        <span v-if="status" class="pill" :data-tone="tone" data-testid="session-header-status">
          <StatusIcon class="icon" :status="status" :unseen="state?.unseen ?? node.unseen" />
          {{ statusText }}
        </span>
        <OverlapChip v-if="agent && !inspectorShown" :agent-id="agent.id" />
      </div>
      <div v-if="meta" class="meta" data-testid="session-header-meta">{{ meta }}</div>
    </div>
    <div class="actions">
      <BackendSwitchButton v-if="agent" ref="backend" :agent-id="agent.id" />
      <RouteButton v-if="agent" ref="route" :agent-id="agent.id" :kind="agent.kind" :model="agent.model" :resumable="!!agent.session_id" :error="agent.provider_error ?? null" />
      <CheckpointsButton v-if="agent && !inspectorShown" ref="checkpoints" :agent-id="agent.id" :name="agent.name" :kind="agent.kind" :refresh-key="status" />
      <AgentLogButton v-if="agent && isChat(agent)" ref="agentLog" :agent-id="agent.id" />
      <ShareButton v-if="agent && isDesktop" ref="share" :agent-id="agent.id" :name="node.name" />
      <ContinueAsAgentButton v-if="terminalId && lapsed" ref="continueAs" :terminal="terminalId" :terminal-key="panel.id" :conversation="lapsed" />
      <TaskNoteButton v-if="worktree && !inspectorShown" ref="taskNote" :worktree-id="worktree.id" :branch="worktree.branch" />
    </div>
    <GitStepButton v-if="worktree && worktree.branch" ref="gitStep" :path="worktree.path" :branch="worktree.branch" :has-request="!!mr" :refresh-key="status" />
    <SessionViewSwitch :panel="panel" :active="active" />
  </header>
</template>

<style scoped>
@container session-header (max-width: 1280px) {
  .session-header :deep(.git-step .full),
  .session-header :deep(.git-step > .hint) {
    display: none;
  }

  .session-header :deep(.git-step .short) {
    display: inline;
  }
}

@container session-header (max-width: 960px) {
  .session-header :deep(.git-step .short) {
    display: none;
  }

  .session-header :deep(.git-step .glyph) {
    display: block;
  }

  .session-header :deep(.git-step > .run) {
    padding: 0 8px;
  }
}

.session-header {
  container: session-header / inline-size;
  display: flex;
  flex: none;
  align-items: center;
  gap: 14px;
  min-height: 56px;
  padding: var(--vr-space-2) 14px var(--vr-space-2) 18px;
  border-bottom: 1px solid var(--vr-color-border);
}

:root[data-vr-viewport='phone'] .session-header {
  position: relative;
  container-type: normal;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  min-height: 0;
  padding: 0 var(--vr-space-3) var(--vr-space-2);
}

:root[data-vr-viewport='phone'] .session-header :deep(.git-step) {
  display: contents;
}

:root[data-vr-viewport='phone'] .session-header :deep(.git-step > .run),
:root[data-vr-viewport='phone'] .session-header :deep(.git-step > .hint) {
  display: none;
}

:root[data-vr-viewport='phone'] .session-header :deep(.git-step > .error) {
  position: static;
  flex-basis: 100%;
  max-width: none;
  white-space: normal;
}

:root[data-vr-viewport='phone'] .session-header .actions {
  position: absolute;
  top: 3rem;
  left: var(--vr-space-3);
  display: flex;
  height: 0;
  overflow: hidden;
  visibility: hidden;
}

.phone-bar {
  display: flex;
  flex: 1 1 100%;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  min-height: 3rem;
}

.phone-bar .title {
  flex: 1;
}

.phone-bar .bar-button {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--vr-size-touch);
  height: var(--vr-size-touch);
  padding: 0;
}

.phone-bar .bar-button:first-child {
  margin-left: calc(var(--vr-space-2) * -1);
  color: var(--vr-color-text);
}

.phone-bar .icon-status {
  width: 14px;
  height: 14px;
}

.actions {
  display: contents;
}

.identity {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 12rem;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.title {
  overflow: hidden;
  font-size: var(--vr-font-size-title);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.pill {
  display: flex;
  flex: none;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 8px;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-full);
}

.icon {
  width: 12px;
  height: 12px;
}

.pill[data-tone='running'] {
  color: var(--vr-status-running);
  background: var(--vr-tint-running);
}

.pill[data-tone='feedback'] {
  color: var(--vr-status-feedback);
  background: var(--vr-tint-feedback);
}

.pill[data-tone='limit'] {
  color: var(--vr-status-limit);
  background: var(--vr-tint-limit);
}

.pill[data-tone='unseen'] {
  color: var(--vr-status-unseen);
  background: var(--vr-tint-unseen);
}

.pill[data-tone='crashed'] {
  color: var(--vr-status-crashed);
  background: var(--vr-tint-crashed);
}

.meta {
  overflow: hidden;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
