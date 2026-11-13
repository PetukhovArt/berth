<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { Check, ChevronDown, ChevronUp, Copy, FolderOpen, GitBranch, LoaderCircle, Mic, Play, SquareTerminal, TriangleAlert, Wrench } from '../../../shared/ui/icons';
import { useIsPhone, VButton, VMessage } from '../../../shared/ui/kit';
import { copyText } from '../../../shared/lib/atlassian';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { continueInBerth, externalDot, externalKey, externalOrigin, externalStatusLabel, externalTarget, readTranscript, resumeCommand, useExternalSessions, type Harness, type TranscriptMessage, HarnessIcon, describeError, useBerth, StatusIcon, useAction, useSessionDock, derivePlan, sendInput, useFeedbackRequest, allowRequest, denyRequest, chooseOption, type SessionRef } from '../../../entities/session';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { useHoldToTalk, type HoldToTalkIssue } from '../model/useHoldToTalk';
import { useVoiceLoop } from '../model/useVoiceLoop';

const TAIL = 200;
const PIN_PX = 48;
const COPIED_MS = 1500;
const NOTICE_MS = 3000;

const props = defineProps<{ harness: Harness; sessionId: string; terminal?: SessionRef }>();

const { t, formatRelative, formatDate } = useI18n();
const external = useExternalSessions();
const berth = useBerth();
const dock = useSessionDock();
const action = useAction();
const scroller = useTemplateRef<HTMLElement>('scroller');
const messages = shallowRef<TranscriptMessage[]>([]);
const loading = shallowRef(true);
const error = shallowRef<string | null>(null);
const copied = shallowRef(false);
const request = props.terminal ? useFeedbackRequest(props.terminal) : null;
const draft = shallowRef('');
const sending = shallowRef(false);
const notice = shallowRef('');
const phone = useIsPhone();
const planOpen = shallowRef(true);
const plan = computed(() => derivePlan(messages.value));
const planDone = computed(() => plan.value.filter((item) => item.state === 'done').length);
let generation = 0;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const session = computed(() => external?.anyByKey.value.get(externalKey({ harness: props.harness, id: props.sessionId })));
const target = computed(() => (session.value ? externalTarget(session.value, berth?.entities.value) : null));
const running = computed(() => session.value?.running ?? false);
const suggestion = computed(() => {
  const ref = props.terminal;
  if (!ref || !('Agent' in ref) || draft.value) return null;
  const agent = berth?.entities.value.agents.get(ref.Agent);
  return agent?.status === 'finished' ? (agent.suggested_prompt ?? null) : null;
});
const command = computed(() => resumeCommand({ harness: props.harness, id: props.sessionId }));
const canContinue = computed(() => !!session.value && !!target.value && !running.value && !action.busy.value);

async function load() {
  const current = ++generation;
  const el = scroller.value;
  const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < PIN_PX;
  try {
    const next = await readTranscript({ harness: props.harness, id: props.sessionId }, TAIL);
    if (current !== generation) return;
    messages.value = next;
    error.value = null;
  } catch (e) {
    if (current === generation) error.value = describeError(e);
  } finally {
    if (current === generation) loading.value = false;
  }
  if (!pinned || current !== generation) return;
  await nextTick();
  const box = scroller.value;
  if (box) box.scrollTop = box.scrollHeight;
}

const root = useTemplateRef<HTMLElement>('root');
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
  const element = root.value;
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
  clearTimeout(noticeTimer);
  copiedTimer = setTimeout(() => (copied.value = false), COPIED_MS);
}

async function continueHere() {
  const current = session.value;
  const where = target.value;
  if (!current || !where || current.running) return;
  await action.run(async () => {
    const ref = await continueInBerth(current, where);
    dock.openWhenListed(ref, externalKey(current), true);
  });
}

async function send() {
  await submit(draft.value.trim(), true);
}

async function submit(text: string, clearDraft: boolean): Promise<boolean> {
  if (!props.terminal || !text || sending.value) return false;
  sending.value = true;
  try {
    await sendInput(props.terminal, `\x1b[200~${text}\x1b[201~`);
    await sendInput(props.terminal, '\r');
    if (clearDraft) draft.value = '';
    return true;
  } catch (e) {
    error.value = describeError(e);
    return false;
  } finally {
    sending.value = false;
  }
}

const ISSUES: Record<HoldToTalkIssue, 'chatTalkDenied' | 'chatTalkShort' | 'chatTalkEmpty' | 'chatTalkFailed'> = { denied: 'chatTalkDenied', short: 'chatTalkShort', empty: 'chatTalkEmpty', failed: 'chatTalkFailed' };
const voice = useVoiceLoop({
  messages: () => messages.value,
  lang: () => (document.documentElement.lang === 'ru' ? 'ru-RU' : 'en-US'),
  listen: () => talk.listen(),
  stop: () => talk.stopListening(),
  listening: () => talk.holding.value,
});
const talk = useHoldToTalk({
  lang: () => (document.documentElement.lang === 'ru' ? 'ru-RU' : 'en-US'),
  onSend: async (text) => {
    voice.sent();
    if (!(await submit(text, false))) voice.unsent();
  },
  onSpeech: () => voice.onSpeech(),
  onSettled: () => voice.onSettled(),
  onIssue: (issue) => {
    if (voice.active.value && (issue === 'denied' || issue === 'failed')) voice.end();
    notice.value = t(`agents.sessionView.${ISSUES[issue]}`);
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice.value = ''), NOTICE_MS);
  },
});
const showTalk = computed(() => phone.value && talk.supported);
const showVoice = computed(() => showTalk.value && voice.supported);
const voiceStatus = computed(() => {
  if (voice.phase.value === 'wait') return t('agents.sessionView.chatVoiceWaiting');
  if (voice.phase.value === 'speak') return t('agents.sessionView.chatVoiceSpeaking');
  if (voice.muted.value) return t('agents.sessionView.chatVoiceMuted');
  return talk.transcript.value || t('agents.sessionView.chatVoiceListening');
});

watch(messages, () => voice.transcriptChanged());

function acceptSuggestion() {
  if (suggestion.value) draft.value = suggestion.value;
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Tab' && !event.shiftKey && suggestion.value) {
    event.preventDefault();
    acceptSuggestion();
    return;
  }
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  void send();
}

onBeforeUnmount(() => {
  clearTimeout(copiedTimer);
  observer?.disconnect();
});
</script>

<template>
  <section ref="root" class="conversation" data-testid="conversation-view" :data-session-id="sessionId" :data-harness="harness">
    <header v-if="!terminal" class="head">
      <div class="title-line">
        <HarnessIcon :harness="harness" :size="14" />
        <h2 class="title">{{ session?.title ?? sessionId }}</h2>
        <span v-if="session" class="status" data-testid="conversation-status" :data-running="session.running" :data-status="session.status">
          <StatusIcon :status="externalDot(session)" :unseen="false" />
          {{ externalStatusLabel(session) }}
        </span>
      </div>
      <div class="meta">
        <span class="chip">{{ t(`workbench.external.harness.${harness}`) }}</span>
        <span v-if="session" class="chip mono" :title="session.cwd"><FolderOpen :size="12" />{{ session.cwd }}</span>
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
      <VMessage v-if="action.error.value" severity="error" size="small" data-testid="conversation-error">{{ action.error.value }}</VMessage>
    </header>
    <div v-if="plan.length" class="plan" data-testid="conversation-plan">
      <UiButton type="button" class="plan-toggle" :aria-expanded="planOpen" data-testid="conversation-plan-toggle" @click="planOpen = !planOpen">
        {{ t('workbench.external.plan', { done: planDone, total: plan.length }) }}
        <ChevronUp v-if="planOpen" :size="12" />
        <ChevronDown v-else :size="12" />
      </UiButton>
      <ul v-if="planOpen" class="plan-items">
        <li v-for="(item, index) in plan" :key="index" class="plan-item" :data-state="item.state" data-testid="conversation-plan-item">
          <Check v-if="item.state === 'done'" :size="13" class="plan-icon" />
          <LoaderCircle v-else-if="item.state === 'doing'" :size="13" class="plan-icon" />
          <span v-else class="plan-ring" />
          <span class="plan-text">{{ item.text }}</span>
        </li>
      </ul>
    </div>
    <div ref="scroller" class="scroll" data-testid="conversation-scroll">
      <p v-if="loading" class="state">{{ t('workbench.external.loading') }}</p>
      <p v-else-if="error" class="state error" role="alert">{{ error }}</p>
      <p v-else-if="!messages.length" class="state">{{ t('workbench.external.empty') }}</p>
      <ol v-else class="messages" data-testid="conversation-messages">
        <li v-for="message in messages" :key="message.key" class="message" :data-role="message.role" :data-tool="message.tool !== null" :title="message.at ? formatDate(message.at) : undefined" data-testid="conversation-message">
          <div v-if="message.tool" class="tool">
            <Wrench :size="12" />
            <span class="tool-name">{{ message.tool }}</span>
            <span class="tool-arg mono">{{ message.text }}</span>
          </div>
          <p v-else-if="message.role === 'user'" class="user">{{ message.text }}</p>
          <RichContent v-else class="assistant" :markdown="message.text" />
        </li>
      </ol>
    </div>
    <div v-if="terminal && request" class="feedback" data-testid="chat-feedback" :data-kind="request.kind">
      <span class="feedback-text">{{ request.kind === 'Permission' ? t('feedback.permission', { tool: request.title }) : request.title }}</span>
      <code v-if="request.detail" class="feedback-detail mono">{{ request.detail }}</code>
      <template v-if="request.kind === 'Permission'">
        <UiButton type="button" class="feedback-action" data-testid="feedback-allow" @click="allowRequest(terminal)">{{ t('feedback.allow') }}</UiButton>
        <UiButton type="button" class="feedback-action" data-testid="feedback-deny" @click="denyRequest(terminal)">{{ t('feedback.deny') }}</UiButton>
      </template>
      <UiButton v-for="(option, index) in request.options" v-else :key="index" type="button" class="feedback-action" data-testid="feedback-option" @click="chooseOption(terminal, index)">{{ option }}</UiButton>
    </div>
    <form v-if="terminal" class="composer" data-testid="chat-composer" @submit.prevent="send">
      <div v-if="voice.active.value" class="voice" data-testid="chat-voice" :data-phase="voice.phase.value" role="status">
        <span class="voice-text" data-testid="chat-voice-status">{{ voiceStatus }}</span>
        <UiButton type="button" class="voice-action" data-testid="chat-voice-mute" @click="voice.toggleMute">{{ voice.muted.value ? t('agents.sessionView.chatVoiceUnmute') : t('agents.sessionView.chatVoiceMute') }}</UiButton>
        <UiButton type="button" class="voice-action voice-end" data-testid="chat-voice-end" @click="voice.end">{{ t('agents.sessionView.chatVoiceEnd') }}</UiButton>
      </div>
      <p v-else-if="talk.holding.value || notice" class="talk-note" :data-cancelling="talk.cancelling.value || undefined" data-testid="chat-talk-note" role="status">{{ talk.holding.value ? talk.transcript.value || t('agents.sessionView.chatTalkListening') : notice }}{{ talk.holding.value ? ' ' + t('agents.sessionView.chatTalkHint') : '' }}</p>
      <UiButton v-if="suggestion && !voice.active.value" type="button" class="suggestion" :title="t('agents.sessionView.chatSuggestion')" data-testid="chat-suggestion" @click="acceptSuggestion">{{ suggestion }}</UiButton>
      <UiTextarea v-if="!voice.active.value" v-model="draft" class="input" rows="2" :placeholder="t('agents.sessionView.chatPlaceholder')" :aria-label="t('agents.sessionView.chatPlaceholder')" data-testid="chat-input" @keydown="onKey" />
      <VButton v-if="!voice.active.value" size="small" type="submit" :disabled="!draft.trim()" :loading="sending" data-testid="chat-send">{{ t('agents.sessionView.chatSend') }}</VButton>
      <UiButton v-if="showVoice && !voice.active.value" type="button" class="voice-start" data-testid="chat-voice-start" @click="voice.start">{{ t('agents.sessionView.chatVoiceStart') }}</UiButton>
      <UiButton v-if="showTalk && !voice.active.value" type="button" class="talk" :data-holding="talk.holding.value || undefined" :data-cancelling="talk.cancelling.value || undefined" :aria-label="t('agents.sessionView.chatTalk')" :title="t('agents.sessionView.chatTalk')" data-testid="chat-talk" @pointerdown="talk.start" @pointermove="talk.move" @pointerup="talk.release" @pointercancel="talk.release" @contextmenu.prevent><Mic :size="20" /></UiButton>
    </form>
  </section>
</template>

<style scoped>
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
  line-height: 20px;
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

.plan {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.plan-toggle {
  display: inline-flex;
  align-items: center;
  align-self: flex-end;
  gap: var(--vr-space-1);
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: none;
}

.plan-items {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--vr-space-1) var(--vr-space-4);
  max-height: 160px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.plan-item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
}

.plan-item[data-state='done'] {
  color: var(--vr-color-text-muted);
  text-decoration: line-through;
}

.plan-icon {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.plan-item[data-state='done'] .plan-icon {
  color: var(--vr-status-finished);
}

.plan-item[data-state='doing'] .plan-icon {
  color: var(--vr-status-running);
}

.plan-ring {
  flex: none;
  width: 11px;
  height: 11px;
  margin: 1px;
  border: 1.5px solid var(--vr-color-text-subtle);
  border-radius: var(--vr-radius-full);
}

.plan-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-6);
}

.feedback {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  max-width: 920px;
  margin: 0 auto var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-tint-feedback);
  border-radius: var(--vr-radius-lg);
  box-shadow: inset 3px 0 0 var(--vr-status-feedback);
}

.feedback-detail {
  min-width: 0;
  padding: 1px 6px;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
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

.composer {
  display: flex;
  flex: none;
  align-items: flex-end;
  gap: var(--vr-space-2);
  width: 100%;
  max-width: 920px;
  margin: 0 auto;
  padding: 0 var(--vr-space-4) var(--vr-space-4);
}

.composer {
  flex-wrap: wrap;
}

.suggestion {
  flex: 0 0 100%;
  overflow: hidden;
  padding: var(--vr-space-1) var(--vr-space-3);
  border: 1px dashed var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.talk-note {
  flex: 0 0 100%;
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.voice {
  display: flex;
  flex: 0 0 100%;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.voice-text {
  flex: 1 1 100%;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.voice-action,
.voice-start {
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-4);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: none;
  border-radius: var(--vr-radius-full);
}

.voice-end {
  font-weight: 600;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.talk-note[data-cancelling] {
  color: var(--vr-color-danger);
}

.talk {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--vr-size-row);
  height: var(--vr-size-row);
  color: var(--vr-color-text);
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 50%;
}

.talk[data-holding] {
  background: var(--vr-color-nav-active);
  transform: scale(1.1);
}

.talk[data-cancelling] {
  color: var(--vr-color-danger);
  border-color: var(--vr-color-danger);
}

.input {
  flex: 1;
  min-width: 0;
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  resize: none;
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-lg);
  outline: none;
}

.input:focus-visible {
  border-color: var(--vr-color-focus);
}

.state {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.state.error {
  color: var(--vr-color-danger);
}

.messages {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 920px;
  margin: 0 auto;
  padding: 0;
  list-style: none;
}

.message[data-role='user'] {
  align-self: flex-end;
  max-width: 85%;
}

.user {
  margin: 0;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-lg);
}

.assistant {
  font-size: var(--vr-font-size-sm);
}

.message[data-tool='true'] + .message[data-tool='true'] {
  margin-top: calc(-1 * var(--vr-space-2));
}

.tool {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  padding: 2px var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.tool-name {
  flex: none;
  font-weight: 600;
}

.tool-arg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
