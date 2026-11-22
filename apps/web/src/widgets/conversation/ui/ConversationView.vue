<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { Check, Copy, FolderOpen, GitBranch, Play, SquareTerminal, TriangleAlert } from '../../../shared/ui/icons';
import { VButton, VMessage } from '../../../shared/ui/kit';
import { copyText } from '../../../shared/lib/atlassian';
import { AGENT_KIND_LABEL, clearHandoff, continueInBerth, handoffOf, externalKey, externalOrigin, externalTarget, checkpointsByPrompt, listCheckpoints, modelLabel, readTranscript, resumeCommand, useExternalSessions, type Checkpoint, type Harness, type TranscriptMessage, HarnessIcon, describeError, useBerth, StatusIcon, useAction, useSessionDock, derivePlan, PlanStrip, deriveSubagents, stripVisible, sendInput, useFeedbackRequest, allowRequest, alwaysAllowRequest, offersAlways, denyRequest, chooseOption, enqueuePrompt, removeQueuedPrompt, sessionKey, stopAgent, submitPrompt, useAcpChat, useQueuedPrompts, useSessionStates, type SessionRef, type SlashSource } from '../../../entities/session';
import { worktreeOf } from '../../../features/session-view-switch';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { readAcpAgentLog } from '../../../shared/api/providers';
import { chatBlocks } from '../model/chatBlocks';
import { forwardSlashes } from '../../../shared/lib/homePath';
import { COLUMN_EDGES, COLUMN_GUTTER, useChatColumn, type ColumnEdge } from '../model/chatColumn';
import { useClock } from '../model/clock';
import ChatFeed from './ChatFeed.vue';
import ChatComposer from './ChatComposer.vue';
import SubagentStrip from './SubagentStrip.vue';
import SubagentView from './SubagentView.vue';

const TAIL = 200;
const PIN_PX = 48;
const COPIED_MS = 1500;

const props = defineProps<{ harness: Harness | 'acp'; sessionId: string; terminal?: SessionRef; panelId?: string }>();

const { t, formatRelative, formatDate } = useI18n();
const external = useExternalSessions();
const berth = useBerth();
const dock = useSessionDock();
const action = useAction();
const scroller = useTemplateRef<HTMLElement>('scroller');
const column = useChatColumn(scroller);

function edgeX(edge: ColumnEdge): number {
  const box = column.box.value;
  if (!box) return 0;
  return edge === 'left' ? box.left : box.left + box.width;
}
const acp = props.harness === 'acp' ? useAcpChat(props.sessionId) : null;
const harness = computed<Harness | null>(() => (props.harness === 'acp' ? null : props.harness));
const transcript = shallowRef<TranscriptMessage[]>([]);
const messages = computed(() => acp?.messages.value ?? transcript.value);
const loadingTranscript = shallowRef(!acp);
const loading = computed(() => acp?.loading.value ?? loadingTranscript.value);
const error = shallowRef<string | null>(null);
const shownError = computed(() => error.value ?? acp?.error.value ?? null);
const queued = useQueuedPrompts(props.sessionId);
const copied = shallowRef(false);
const request = props.terminal ? useFeedbackRequest(props.terminal) : null;
const sending = shallowRef(false);
const plan = computed(() => acp?.plan.value ?? derivePlan(messages.value));
const agentLog = shallowRef<string | null>(null);
const checkpoints = shallowRef<Checkpoint[]>([]);
const spoken = shallowRef<ReadonlySet<string>>(new Set());

async function toggleAgentLog() {
  if (agentLog.value !== null) {
    agentLog.value = null;
    return;
  }
  try {
    agentLog.value = await readAcpAgentLog(props.sessionId);
  } catch (e) {
    agentLog.value = describeError(e);
  }
}
let generation = 0;
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const session = computed(() => (harness.value ? external?.anyByKey.value.get(externalKey({ harness: harness.value, id: props.sessionId })) : undefined));
const target = computed(() => (session.value ? externalTarget(session.value, berth?.entities.value) : null));
const running = computed(() => session.value?.running ?? false);
const agentId = computed(() => (props.terminal && 'Agent' in props.terminal ? props.terminal.Agent : null));
const agent = computed(() => (agentId.value ? berth?.entities.value.agents.get(agentId.value) : undefined));
const states = useSessionStates();
const sessionState = computed(() => (props.terminal ? states.value.get(sessionKey(props.terminal)) : undefined));
const turnRunning = computed(() => sessionState.value?.status === 'running');
const live = computed(() => (sessionState.value ? sessionState.value.alive && (sessionState.value.status === 'running' || sessionState.value.status === 'needs_feedback') : running.value));
const suggestion = computed(() => (agent.value?.status === 'finished' ? (agent.value.suggested_prompt ?? null) : null));
const command = computed(() => (harness.value ? resumeCommand({ harness: harness.value, id: props.sessionId }) : ''));
const handoff = computed(() => (session.value ? handoffOf(session.value) : undefined));
const continueError = computed(() => action.error.value ?? (handoff.value?.state === 'failed' ? handoff.value.message : null));
const canContinue = computed(() => !!session.value && !!target.value && !running.value && !action.busy.value && handoff.value?.state !== 'pending');

const root = computed(() => {
  const own = agent.value ? berth?.entities.value.worktrees.get(agent.value.worktree_id)?.path : undefined;
  return own ?? (props.panelId ? worktreeOf(props.panelId)?.path : undefined) ?? session.value?.cwd ?? null;
});
const agentLabel = computed(() => {
  const own = agent.value;
  if (own) return own.kind === 'acp' && own.acp_provider ? own.acp_provider : AGENT_KIND_LABEL[own.kind];
  return harness.value ? t(`workbench.external.harness.${harness.value}`) : AGENT_KIND_LABEL.acp;
});
const blocks = computed(() => chatBlocks(messages.value));
const promptCount = computed(() => blocks.value.filter((block) => block.type === 'prompt').length);
const roster = computed(() => deriveSubagents(messages.value, live.value));
const subagentOf = computed(() => new Map(roster.value.all.map((agent) => [agent.callId, agent])));
const now = useClock((at) => stripVisible(roster.value.current, at));
const stripAgents = computed(() => (stripVisible(roster.value.current, now.value) ? roster.value.current : []));
const openedCall = shallowRef<string | null>(null);
const opened = computed(() => (openedCall.value ? subagentOf.value.get(openedCall.value) : undefined));
let savedScroll = 0;
const checkpointOf = computed(() => checkpointsByPrompt(messages.value, checkpoints.value));
const slashSource = computed<SlashSource | null>(() => {
  if (acp) return { acpAgent: props.sessionId };
  return harness.value && root.value ? { harness: harness.value, cwd: root.value } : null;
});
const latestModel = computed(() => {
  for (let i = blocks.value.length - 1; i >= 0; i--) {
    const block = blocks.value[i]!;
    if (block.type === 'turn' && block.model) return block.model;
  }
  return agent.value?.model ?? null;
});
const modelChip = computed(() => [agentLabel.value, latestModel.value ? modelLabel(latestModel.value) : null].filter(Boolean).join(' · '));

async function loadCheckpoints() {
  const id = agentId.value;
  if (!id) {
    checkpoints.value = [];
    return;
  }
  try {
    const next = await listCheckpoints(id);
    if (agentId.value === id) checkpoints.value = next;
  } catch {
    if (agentId.value === id) checkpoints.value = [];
  }
}

watch([agentId, promptCount], () => void loadCheckpoints(), { immediate: true });

async function load() {
  const own = harness.value;
  if (!own) return;
  const current = ++generation;
  const el = scroller.value;
  const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < PIN_PX;
  try {
    const next = await readTranscript({ harness: own, id: props.sessionId }, TAIL);
    if (current !== generation) return;
    transcript.value = next;
    error.value = null;
  } catch (e) {
    if (current === generation) error.value = describeError(e);
  } finally {
    if (current === generation) loadingTranscript.value = false;
  }
  if (!pinned || current !== generation) return;
  await nextTick();
  const box = scroller.value;
  if (box) box.scrollTop = box.scrollHeight;
}

const section = useTemplateRef<HTMLElement>('section');
let loaded = false;
let shown = true;
let stale = false;
let observer: IntersectionObserver | undefined;

watch(
  () => [props.harness, props.sessionId, session.value?.updated_at] as const,
  () => {
    if (loaded && !shown) {
      stale = true;
      return;
    }
    loaded = true;
    void load();
  },
  { immediate: true },
);

onMounted(() => {
  const element = section.value;
  if (!element) return;
  observer = new IntersectionObserver((entries) => {
    const latest = entries.at(-1);
    if (!latest || latest.isIntersecting === shown) return;
    shown = latest.isIntersecting;
    if (shown && stale) {
      stale = false;
      void load();
    }
  });
  observer.observe(element);
});

async function copyCommand() {
  await copyText(command.value);
  copied.value = true;
  clearTimeout(copiedTimer);
  copiedTimer = setTimeout(() => (copied.value = false), COPIED_MS);
}

async function continueHere() {
  const current = session.value;
  const where = target.value;
  if (!current || !where || current.running) return;
  clearHandoff(current);
  await action.run(async () => {
    const ref = await continueInBerth(current, where);
    dock.openWhenListed(ref, externalKey(current), true);
  });
}

async function submit(text: string, viaVoice: boolean): Promise<boolean> {
  if (!props.terminal || !text || sending.value) return false;
  sending.value = true;
  try {
    if (acp) await sendToAcp(text);
    else {
      await sendInput(props.terminal, `\x1b[200~${text}\x1b[201~`);
      await sendInput(props.terminal, '\r');
    }
    if (viaVoice) spoken.value = new Set(spoken.value).add(text.trim());
    return true;
  } catch (e) {
    error.value = describeError(e);
    return false;
  } finally {
    sending.value = false;
  }
}

async function stop() {
  if (!props.terminal) return;
  try {
    if (acp) await stopAgent(props.sessionId);
    else await sendInput(props.terminal, '\x1b');
  } catch (e) {
    error.value = describeError(e);
  }
}

async function sendToAcp(text: string) {
  const status = berth?.entities.value.agents.get(props.sessionId)?.status;
  if (status === 'running' || status === 'needs_feedback') enqueuePrompt(props.sessionId, text);
  else await submitPrompt({ Agent: props.sessionId }, text);
}

function openSubagent(callId: string) {
  savedScroll = scroller.value?.scrollTop ?? 0;
  openedCall.value = callId;
}

async function closeSubagent() {
  openedCall.value = null;
  await nextTick();
  if (scroller.value) scroller.value.scrollTop = savedScroll;
}

watch(messages, async () => {
  if (!acp) return;
  const el = scroller.value;
  if (!el || el.scrollHeight - el.scrollTop - el.clientHeight >= PIN_PX) return;
  await nextTick();
  el.scrollTop = el.scrollHeight;
});

onBeforeUnmount(() => {
  clearTimeout(copiedTimer);
  observer?.disconnect();
});
</script>

<template>
  <section ref="section" class="conversation" data-testid="conversation-view" :data-session-id="sessionId" :data-harness="harness">
    <header v-if="!terminal" class="head">
      <div class="title-line">
        <HarnessIcon v-if="harness" :harness="harness" :size="14" />
        <h2 class="title">{{ session?.title ?? sessionId }}</h2>
        <span v-if="session" class="status" data-testid="conversation-status" :data-running="session.running" :data-status="session.status">
          <StatusIcon :status="session.status" :unseen="false" />
          {{ t(`agents.status.${session.status}`) }}
        </span>
      </div>
      <div class="meta">
        <span v-if="harness" class="chip">{{ t(`workbench.external.harness.${harness}`) }}</span>
        <span v-if="session" class="chip mono" :title="session.cwd"><FolderOpen :size="12" />{{ forwardSlashes(session.cwd) }}</span>
        <span v-if="session?.branch" class="chip mono"><GitBranch :size="12" />{{ session.branch }}</span>
        <span v-if="session" class="chip" :title="formatDate(session.updated_at)">{{ formatRelative(session.updated_at) }}</span>
        <span v-if="session" class="chip" data-testid="conversation-origin"><SquareTerminal :size="12" />{{ externalOrigin(session) }}</span>
      </div>
      <div class="actions">
        <VButton size="small" :disabled="!canContinue" :loading="action.busy.value" data-testid="conversation-continue" @click="continueHere">
          <Play :size="13" />
          {{ t('workbench.external.continue') }}
        </VButton>
        <VButton size="small" severity="secondary" outlined :title="command" data-testid="conversation-copy" @click="copyCommand">
          <Check v-if="copied" :size="13" />
          <Copy v-else :size="13" />
          {{ copied ? t('workbench.external.copied') : t('workbench.external.copyResume') }}
        </VButton>
        <code class="command mono">{{ command }}</code>
      </div>
      <p v-if="running && session?.in_berth" class="hint" data-testid="conversation-in-terminal">{{ t('workbench.external.liveInTerminal') }}</p>
      <p v-else-if="running" class="warning" role="alert" data-testid="conversation-running-warning"><TriangleAlert :size="13" />{{ t('workbench.external.stillOpen') }}</p>
      <p v-else-if="session && !target" class="hint" data-testid="conversation-no-project">{{ t('workbench.external.noProject') }}</p>
      <p v-else-if="target" class="hint" data-testid="conversation-target">{{ t('workbench.external.continuesIn', { project: target.projectName, branch: target.branch }) }}</p>
      <VMessage v-if="continueError" severity="error" size="small" data-testid="conversation-error">{{ continueError }}</VMessage>
    </header>
    <PlanStrip :plan="plan" />
    <SubagentView v-if="opened && harness" :harness="harness" :session-id="sessionId" :agent="opened" :agent-label="agentLabel" :root="root" :panel-id="panelId" :now="now" @close="closeSubagent" />
    <div v-show="!opened" class="body" :style="column.style.value" data-testid="chat-body">
      <div ref="scroller" class="scroll" data-testid="conversation-scroll">
        <p v-if="loading" class="column state">{{ t('workbench.external.loading') }}</p>
        <p v-else-if="shownError" class="column state error" role="alert">{{ shownError }}</p>
        <p v-else-if="!messages.length" class="column state">{{ t('workbench.external.empty') }}</p>
        <ChatFeed v-else class="column" :blocks="blocks" :harness="harness" :agent-label="agentLabel" :root="root" :panel-id="panelId" :agent-id="agentId" :checkpoint-of="checkpointOf" :spoken="spoken" :subagent-of="subagentOf" :now="now" @restored="loadCheckpoints" @open-subagent="openSubagent" />
      </div>
      <div v-if="terminal && request" class="column feedback" data-testid="chat-feedback" :data-kind="request.kind">
        <span class="feedback-text">{{ request.kind === 'Permission' ? t('feedback.permission', { tool: request.title }) : request.title }}</span>
        <code v-if="request.detail" class="feedback-detail mono">{{ request.detail }}</code>
        <template v-if="request.kind === 'Permission' && !request.options.length">
          <UiButton type="button" class="feedback-action" data-testid="feedback-allow" @click="allowRequest(terminal)">{{ t('feedback.allow') }}</UiButton>
          <UiButton v-if="offersAlways(request)" type="button" class="feedback-action" :title="t('feedback.alwaysTitle')" data-testid="feedback-always" @click="alwaysAllowRequest(terminal)">{{ t('feedback.always') }}</UiButton>
          <UiButton type="button" class="feedback-action" data-testid="feedback-deny" @click="denyRequest(terminal)">{{ t('feedback.deny') }}</UiButton>
        </template>
        <UiButton v-for="(option, index) in request.options" v-else :key="index" type="button" class="feedback-action" data-testid="feedback-option" @click="chooseOption(terminal, index)">{{ option }}</UiButton>
      </div>
      <div v-if="acp" class="column agent-log" data-testid="chat-agent-log">
        <UiButton type="button" class="agent-log-toggle" :aria-expanded="agentLog !== null" data-testid="chat-agent-log-toggle" @click="toggleAgentLog">{{ t('agents.sessionView.chatAgentLog') }}</UiButton>
        <pre v-if="agentLog !== null" class="agent-log-text mono" data-testid="chat-agent-log-text">{{ agentLog || t('agents.sessionView.chatAgentLogEmpty') }}</pre>
      </div>
      <ul v-if="acp && queued.length" class="column queued" data-testid="chat-queue">
        <li v-for="item in queued" :key="item.id" class="queued-item" data-testid="queue-item">
          <span class="queued-text" data-testid="queue-item-text">{{ item.text }}</span>
          <span class="queued-hint">{{ t('feedback.queue.sendsWhenFinished') }}</span>
          <UiButton type="button" class="queued-remove" data-testid="queue-remove" @click="removeQueuedPrompt(sessionId, item.id)">{{ t('feedback.queue.remove') }}</UiButton>
        </li>
      </ul>
      <SubagentStrip v-if="stripAgents.length" class="column" :agents="stripAgents" :now="now" @open="openSubagent" />
      <ChatComposer v-if="terminal" :messages="messages" :suggestion="suggestion" :model-chip="modelChip" :sending="sending" :slash-source="slashSource" :stop="turnRunning ? stop : null" :send="submit" />
      <template v-if="column.box.value && !column.phone.value">
        <div
          v-for="edge in COLUMN_EDGES"
          :key="edge"
          class="column-handle vr-wide-only"
          role="separator"
          aria-orientation="vertical"
          tabindex="0"
          :data-edge="edge"
          :data-dragging="column.dragging.value === edge || undefined"
          :aria-label="t(`chat.column.${edge}`)"
          :title="t('chat.column.hint')"
          :aria-valuenow="edgeX(edge)"
          aria-valuemin="0"
          :aria-valuemax="column.track.value ?? 0"
          :data-testid="`chat-column-${edge}-handle`"
          :style="{ left: `${COLUMN_GUTTER + edgeX(edge)}px` }"
          @pointerdown="column.startDrag(edge, $event)"
          @dblclick="column.reset"
          @keydown="column.nudge(edge, $event)"
        />
      </template>
    </div>
  </section>
</template>

<style scoped>
.agent-log {
  display: flex;
  flex: none;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) 0 0;
}

.agent-log-toggle {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.agent-log-text {
  align-self: stretch;
  max-height: 200px;
  margin: 0;
  overflow: auto;
  font-size: var(--vr-font-size-2xs);
  white-space: pre-wrap;
  color: var(--vr-color-text-muted);
}

.queued {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: var(--vr-space-2) 0 0;
  margin-top: 0;
  margin-bottom: 0;
  list-style: none;
}

.queued-item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: 0 var(--vr-space-2);
  border: 1px dashed var(--vr-color-border-strong);
  border-radius: var(--vr-radius-md);
}

.queued-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-sm);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.queued-hint {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.queued-remove {
  flex: none;
  padding: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}

.conversation {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
}

.head {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.title-line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.status {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.meta,
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1) var(--vr-space-2);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  max-width: 100%;
  padding: 0 var(--vr-space-2);
  overflow: hidden;
  font-size: var(--vr-font-size-2xs);
  line-height: 1.8;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.mono {
  font-family: var(--vr-font-mono);
}

.command {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  user-select: all;
}

.warning,
.hint {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  margin: 0;
  font-size: var(--vr-font-size-xs);
}

.warning {
  color: var(--vr-color-danger);
}

.hint {
  color: var(--vr-color-text-muted);
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  padding: var(--vr-space-3) 0 var(--vr-space-6);
}

.body {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.column {
  box-sizing: border-box;
  width: var(--chat-column-width, auto);
  margin-left: var(--chat-column-left, var(--vr-space-4));
  margin-right: var(--vr-space-4);
}

.column-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 1;
  width: 6px;
  margin-left: -3px;
  cursor: col-resize;
  touch-action: none;
}

.column-handle:hover,
.column-handle:focus-visible,
.column-handle[data-dragging] {
  outline: none;
  background: var(--vr-color-nav-active);
}

.feedback {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  margin-bottom: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-tint-feedback);
  border-radius: var(--vr-radius-lg);
  box-shadow: inset 3px 0 0 var(--vr-status-feedback);
}

.feedback-detail {
  box-sizing: border-box;
  flex: 1 1 100%;
  min-width: 0;
  max-height: min(30dvh, 10em);
  padding: 1px 6px;
  overflow-x: hidden;
  overflow-y: auto;
  overflow-wrap: anywhere;
  font-size: var(--vr-font-size-xs);
  white-space: pre-wrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.feedback-action {
  height: 26px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: none;
  border-radius: var(--vr-radius-md);
}

.state {
  margin-top: 0;
  margin-bottom: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.state.error {
  color: var(--vr-color-danger);
}

</style>
