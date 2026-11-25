<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, shallowRef, useId, useTemplateRef, watch } from 'vue';
import { isDesktop } from '../../../bridge';
import { ArrowUp, Mic, Paperclip, Square, X } from '../../../shared/ui/icons';
import { useIsPhone } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import type { PlainKey } from '../../../shared/lib/i18n/translate';
import { speechLang, useHoldToTalk, type HoldToTalkIssue } from '../../../shared/lib/dictation';
import { listSlashCommands, matchSlashCommands, uploadName, uploadSrc, type AcpAttachmentUpload, type AcpChatCaps, type AcpConfigOption, type AcpMcpServer, type PromptQuote, type SlashCommand, type SlashSource, type TranscriptMessage } from '../../../entities/session';
import { useDock } from '../../../shared/model/dock';
import { isAvailable, showViewInActivePanel } from '../../../features/session-view-switch';
import { useAttachments } from '../model/useAttachments';
import { useMention } from '../model/useMention';
import { useVoiceLoop } from '../model/useVoiceLoop';
import AttachmentChip from './AttachmentChip.vue';
import OptionPicker from './OptionPicker.vue';
import McpPicker from './McpPicker.vue';

const NOTICE_MS = 3000;
const BLOCKED_NOTICE_MS = 8000;
const WAVE_BARS = [10, 22, 34, 18, 40, 26, 14, 30, 20, 8];

const SLASH_QUERY = /^\/(\S*)$/;

type PopupItem = { key: string; label: string; description: string };

const props = defineProps<{ messages: TranscriptMessage[]; suggestion: string | null; modelChip: string; sending: boolean; slashSource: SlashSource | null; stop: (() => Promise<void>) | null; send: (text: string, spoken: boolean, uploads?: AcpAttachmentUpload[]) => Promise<boolean>; quotes: readonly PromptQuote[]; caps: AcpChatCaps | null; options: readonly AcpConfigOption[]; mcp: readonly AcpMcpServer[]; refreshMcp: () => Promise<void>; worktreeId: string | null }>();
const emit = defineEmits<{ unquote: [label: string]; option: [id: string, value: string] }>();

const phone = useIsPhone();
const draft = shallowRef('');
const input = useTemplateRef<InstanceType<typeof UiTextarea>>('input');
const slashList = useTemplateRef<HTMLElement>('slashList');
const listId = useId();
const commands = shallowRef<SlashCommand[]>([]);
const dismissed = shallowRef(false);
const active = shallowRef(0);
let commandsRun = 0;
const notice = shallowRef('');
const typing = shallowRef(false);
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

function say(text: string, ms: number) {
  notice.value = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (notice.value = ''), ms);
}

const attachments = useAttachments(
  () => props.caps?.images ?? false,
  (text) => say(text, NOTICE_MS),
);
const mention = useMention(draft, () => (props.caps ? props.worktreeId : null));
const modelOption = computed(() => props.options.find((option) => option.category === 'Model'));
const effortOption = computed(() => props.options.find((option) => option.category === 'ThoughtLevel'));

const ISSUES: Record<HoldToTalkIssue, PlainKey> = {
  denied: 'agents.sessionView.chatTalkDenied',
  short: 'agents.sessionView.chatTalkShort',
  empty: 'agents.sessionView.chatTalkEmpty',
  failed: 'agents.sessionView.chatTalkFailed',
  offline: 'voice.issue.offline',
  loading: 'voice.issue.loading',
  phrase: 'voice.issue.phrase',
};

const voice = useVoiceLoop({
  messages: () => props.messages,
  lang: speechLang,
  listen: () => talk.listen(),
  stop: () => talk.stopListening(),
  listening: () => talk.holding.value,
});
const talk = useHoldToTalk({
  onSend: async (text) => {
    voice.sent();
    if (!(await props.send(text, true))) voice.unsent();
  },
  onSpeech: () => voice.onSpeech(),
  onSettled: () => voice.onSettled(),
  onIssue: (issue) => {
    if (voice.active.value && issue !== 'short' && issue !== 'empty') voice.end();
    notice.value = t(ISSUES[issue]);
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice.value = ''), NOTICE_MS);
  },
});

const secure = isDesktop || window.isSecureContext;
const showTalk = talk.supported && secure;
const talkBlocked = !isDesktop && !showTalk;
const showVoice = computed(() => phone.value && showTalk && voice.supported);
const talkPanel = computed(() => phone.value && showTalk && !typing.value);
const { activeKey } = useDock();
const diffAvailable = computed(() => !!activeKey.value && isAvailable(activeKey.value, 'diff'));
const quickSending = shallowRef(false);

async function sendQuick(text: string) {
  if (quickSending.value) return;
  quickSending.value = true;
  try {
    await props.send(text, false);
  } finally {
    quickSending.value = false;
  }
}
const shownSuggestion = computed(() => (draft.value ? null : props.suggestion));
const listening = computed(() => voice.active.value && voice.phase.value === 'listen' && !voice.muted.value);
const voiceStatus = computed(() => {
  if (voice.phase.value === 'wait') return t('agents.sessionView.chatVoiceWaiting');
  if (voice.phase.value === 'speak') return t('agents.sessionView.chatVoiceSpeaking');
  if (voice.muted.value) return t('agents.sessionView.chatVoiceMuted');
  return talk.transcript.value ? t('chat.heard', { text: talk.transcript.value }) : t('agents.sessionView.chatVoiceListening');
});
const talkNote = computed(() => {
  if (talk.holding.value) return `${talk.transcript.value || t('agents.sessionView.chatTalkListening')} ${t('agents.sessionView.chatTalkHint')}`;
  if (talk.recognising.value) return t('voice.recognising');
  return notice.value;
});

watch(
  () => props.messages,
  () => voice.transcriptChanged(),
);

watch(
  () => props.quotes.length,
  (size, before) => {
    if (size > before) void nextTick(() => input.value?.focus());
  },
);

onBeforeUnmount(() => clearTimeout(noticeTimer));

async function sendDraft() {
  const text = draft.value.trim();
  const uploads = attachments.uploads.value;
  if ((text || uploads.length) && (await props.send(text, false, uploads))) {
    draft.value = '';
    attachments.clear();
  }
}

function fill(text: string) {
  draft.value = text;
  void nextTick(() => input.value?.focus());
}

defineExpose({ fill, notify: (text: string) => say(text, BLOCKED_NOTICE_MS) });

const canSend = computed(() => !!draft.value.trim() || attachments.pending.value.length > 0);

function attach(files: FileList | null | undefined) {
  if (files?.length) void attachments.add(Array.from(files));
}

function pickFiles() {
  const picker = document.createElement('input');
  picker.type = 'file';
  picker.multiple = true;
  picker.onchange = () => attach(picker.files);
  picker.click();
}

function onPaste(event: ClipboardEvent) {
  if (!props.caps || !event.clipboardData?.files.length) return;
  event.preventDefault();
  attach(event.clipboardData.files);
}

function onDrop(event: DragEvent) {
  if (props.caps) attach(event.dataTransfer?.files);
}

function acceptSuggestion() {
  if (shownSuggestion.value) draft.value = shownSuggestion.value;
}

const slashQuery = computed(() => SLASH_QUERY.exec(draft.value)?.[1] ?? null);
const slashItems = computed<PopupItem[]>(() => (slashQuery.value === null ? [] : matchSlashCommands(commands.value, slashQuery.value).map((command) => ({ key: command.name, label: `/${command.name}`, description: command.description }))));
const mentionItems = computed<PopupItem[]>(() => mention.files.value.map((path) => ({ key: path, label: path, description: '' })));
const popupKind = computed(() => (dismissed.value ? null : slashItems.value.length ? 'slash' : mentionItems.value.length ? 'mention' : null));
const popupItems = computed(() => (popupKind.value === 'slash' ? slashItems.value : popupKind.value === 'mention' ? mentionItems.value : []));
const slashOpen = computed(() => popupItems.value.length > 0);
const activeId = computed(() => (slashOpen.value ? `${listId}-${active.value}` : undefined));

async function loadCommands() {
  const source = props.slashSource;
  if (!source) return;
  const run = ++commandsRun;
  const next = await listSlashCommands(source).catch(() => []);
  if (run === commandsRun) commands.value = next;
}

watch(slashQuery, (query, previous) => {
  if (query !== null && (previous === null || previous === undefined)) void loadCommands();
});

watch([slashQuery, mention.query], ([slash, mentioned]) => {
  if (slash === null && mentioned === null) dismissed.value = false;
});

watch(popupItems, () => (active.value = 0));

function moveActive(step: number) {
  const size = popupItems.value.length;
  active.value = (active.value + step + size) % size;
  void nextTick(() => slashList.value?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }));
}

function pickItem(item: PopupItem) {
  if (popupKind.value === 'mention') {
    draft.value = mention.strip();
    attachments.addPath(item.key);
  } else draft.value = `/${item.key} `;
  input.value?.focus();
}

function onSlashKey(event: KeyboardEvent): boolean {
  if (!slashOpen.value || event.isComposing) return false;
  const current = popupItems.value[active.value];
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') moveActive(event.key === 'ArrowDown' ? 1 : -1);
  else if (event.key === 'Escape') {
    event.stopPropagation();
    dismissed.value = true;
  } else if (event.key === 'Tab' && !event.shiftKey && current) pickItem(current);
  else if (event.key === 'Enter' && !event.shiftKey && current && (popupKind.value === 'mention' || current.key.toLowerCase() !== slashQuery.value?.toLowerCase())) pickItem(current);
  else return false;
  event.preventDefault();
  return true;
}

function onKey(event: KeyboardEvent) {
  if (onSlashKey(event)) return;
  if (event.key === 'Tab' && !event.shiftKey && shownSuggestion.value) {
    event.preventDefault();
    acceptSuggestion();
    return;
  }
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  void sendDraft();
}

function explainBlocked() {
  notice.value = t(secure ? 'chat.talkUnavailable' : 'chat.talkNeedsHttps');
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (notice.value = ''), BLOCKED_NOTICE_MS);
}

function holdMode() {
  if (voice.active.value) voice.end();
  typing.value = false;
}

function handsFree() {
  typing.value = false;
  voice.start();
}
</script>

<template>
  <form class="composer" data-testid="chat-composer" :data-phone="phone || undefined" @submit.prevent="sendDraft">
    <div v-if="talkPanel && !voice.active.value" class="quick" data-testid="chat-quick">
      <UiButton type="button" class="chip" :disabled="quickSending" data-testid="chat-quick-tests" @click="sendQuick(t('chat.quick.tests'))">{{ t('chat.quick.tests') }}</UiButton>
      <UiButton v-if="diffAvailable" type="button" class="chip" data-testid="chat-quick-diff" @click="showViewInActivePanel('diff')">{{ t('chat.quick.diff') }}</UiButton>
      <UiButton v-if="stop" type="button" class="chip" data-testid="chat-quick-stop" @click="stop">{{ t('chat.quick.stop') }}</UiButton>
    </div>
    <div v-if="showVoice && (voice.active.value || talkPanel)" class="modes" role="group" :aria-label="t('chat.voiceMode')" data-testid="chat-voice-modes">
      <UiButton type="button" class="mode" :aria-pressed="!voice.active.value" data-testid="chat-voice-hold" @click="holdMode">{{ t('chat.holdToTalk') }}</UiButton>
      <UiButton type="button" class="mode" :aria-pressed="voice.active.value" data-testid="chat-voice-start" @click="handsFree">{{ t('chat.handsFree') }}</UiButton>
    </div>
    <div v-if="voice.active.value" class="voice" data-testid="chat-voice" :data-phase="voice.phase.value" role="status">
      <div class="wave" :data-live="listening || undefined" aria-hidden="true">
        <span v-for="(height, bar) in WAVE_BARS" :key="bar" class="bar" :style="{ '--bar': bar, '--h': `${height}px` }" />
      </div>
      <p class="voice-text" data-testid="chat-voice-status">{{ voiceStatus }}</p>
      <div class="voice-actions">
        <UiButton type="button" class="pill" data-testid="chat-voice-mute" @click="voice.toggleMute">{{ voice.muted.value ? t('agents.sessionView.chatVoiceUnmute') : t('agents.sessionView.chatVoiceMute') }}</UiButton>
        <UiButton type="button" class="pill strong" data-testid="chat-voice-end" @click="voice.end">{{ t('agents.sessionView.chatVoiceEnd') }}</UiButton>
      </div>
    </div>
    <template v-else>
      <p v-if="talkNote" class="talk-note" :data-cancelling="talk.cancelling.value || undefined" data-testid="chat-talk-note" role="status">{{ talkNote }}</p>
      <UiButton v-if="shownSuggestion && talkPanel" type="button" class="suggestion" :title="t('agents.sessionView.chatSuggestion')" data-testid="chat-suggestion" @click="acceptSuggestion">{{ shownSuggestion }}</UiButton>
      <div v-if="talkPanel" class="talk-panel">
        <div class="talk-row">
          <UiButton type="button" class="aa" :title="t('chat.typeMode')" :aria-label="t('chat.typeMode')" data-testid="chat-type-mode" @click="typing = true">{{ t('chat.typeGlyph') }}</UiButton>
          <UiButton type="button" class="talk big" :data-holding="talk.holding.value || undefined" :data-cancelling="talk.cancelling.value || undefined" :aria-label="t('agents.sessionView.chatTalk')" :title="t('agents.sessionView.chatTalk')" data-testid="chat-talk" @pointerdown="talk.start" @pointermove="talk.move" @pointerup="talk.release" @pointercancel="talk.release" @contextmenu.prevent><Mic :size="26" /></UiButton>
          <UiButton v-if="stop" type="button" class="aa stop" :title="t('agents.sessionView.chatStop')" :aria-label="t('agents.sessionView.chatStop')" data-testid="chat-stop" @click="stop"><Square :size="14" fill="currentColor" /></UiButton>
          <span v-else class="aa-balance" aria-hidden="true" />
        </div>
        <p class="hint">{{ t(stop ? 'chat.talkHintRunning' : 'chat.talkHint') }}</p>
      </div>
      <div v-else class="box" @dragover.prevent @drop.prevent="onDrop">
        <ul v-if="slashOpen" :id="listId" ref="slashList" class="slash" role="listbox" :aria-label="t(popupKind === 'mention' ? 'chat.mention.list' : 'chat.slash.list')" :data-testid="popupKind === 'mention' ? 'chat-mention-list' : 'chat-slash-list'">
          <li v-for="(item, index) in popupItems" :id="`${listId}-${index}`" :key="item.key" class="slash-item" role="option" :aria-selected="index === active" :data-name="item.key" :data-testid="popupKind === 'mention' ? 'chat-mention-item' : 'chat-slash-item'" @pointerdown.prevent @click="pickItem(item)">
            <span class="slash-name">{{ item.label }}</span>
            <span v-if="item.description" class="slash-description">{{ item.description }}</span>
          </li>
        </ul>
        <div v-if="attachments.pending.value.length" class="quotes" data-testid="chat-pending-attachments">
          <AttachmentChip v-for="entry in attachments.pending.value" :key="entry.id" :label="uploadName(entry.upload)" :src="uploadSrc(entry.upload)" removable @remove="attachments.remove(entry.id)" />
        </div>
        <div v-if="quotes.length" class="quotes" data-testid="chat-quotes">
          <span v-for="quote in quotes" :key="quote.label" class="quote" :title="quote.text" data-testid="chat-quote">
            {{ quote.label }}
            <UiButton type="button" class="unquote" :aria-label="t('agents.sessionView.chatQuoteRemove', { label: quote.label })" data-testid="chat-quote-remove" @click="emit('unquote', quote.label)"><X :size="12" /></UiButton>
          </span>
        </div>
        <div class="field">
          <UiTextarea ref="input" v-model="draft" class="input" rows="2" :placeholder="shownSuggestion ? '' : t(quotes.length ? 'agents.sessionView.chatPlaceholderQuoted' : stop ? 'agents.sessionView.chatPlaceholderRunning' : 'agents.sessionView.chatPlaceholder')" :aria-label="t('agents.sessionView.chatPlaceholder')" role="combobox" aria-autocomplete="list" :aria-expanded="slashOpen" :aria-controls="slashOpen ? listId : undefined" :aria-activedescendant="activeId" data-testid="chat-input" @keydown="onKey" @paste="onPaste" />
          <div v-if="shownSuggestion" class="ghost" :title="t('agents.sessionView.chatSuggestion')" data-testid="chat-suggestion">
            <span class="ghost-text">{{ shownSuggestion }}</span>
            <UiButton type="button" class="keycap" tabindex="-1" @click="acceptSuggestion">{{ t('chat.tabKey') }}</UiButton>
          </div>
        </div>
        <div class="foot">
          <template v-if="caps">
            <UiButton type="button" class="attach" :aria-label="t('chat.attach.button')" :title="t('chat.attach.button')" data-testid="chat-attach" @click="pickFiles"><Paperclip :size="14" /></UiButton>
          </template>
          <OptionPicker v-if="modelOption" :option="modelOption" kind="model" @pick="(id, value) => emit('option', id, value)" />
          <span v-else-if="modelChip" class="model" :title="modelChip" data-testid="chat-model">{{ modelChip }}</span>
          <OptionPicker v-if="effortOption" :option="effortOption" kind="effort" @pick="(id, value) => emit('option', id, value)" />
          <McpPicker v-if="mcp.length" :servers="mcp" :refresh="refreshMcp" />
          <span class="grow" />
          <span v-if="!phone" class="keys" data-testid="chat-keys">{{ t(stop && caps ? 'chat.keysQueue' : 'chat.keysHint') }}</span>
          <UiButton v-if="talkBlocked" type="button" class="talk blocked" aria-disabled="true" :aria-label="t('chat.talkMode')" :title="t(secure ? 'chat.talkUnavailable' : 'chat.talkNeedsHttps')" data-testid="chat-talk-blocked" @click="explainBlocked"><Mic :size="14" /></UiButton>
          <UiButton v-else-if="showTalk && phone" type="button" class="talk" :aria-label="t('chat.talkMode')" :title="t('chat.talkMode')" data-testid="chat-talk-mode" @click="typing = false"><Mic :size="14" /></UiButton>
          <UiButton v-else-if="showTalk" type="button" class="talk" :data-holding="talk.holding.value || undefined" :data-cancelling="talk.cancelling.value || undefined" :aria-label="t('agents.sessionView.chatTalk')" :title="t('agents.sessionView.chatTalk')" data-testid="chat-talk" @pointerdown="talk.start" @pointermove="talk.move" @pointerup="talk.release" @pointercancel="talk.release" @contextmenu.prevent><Mic :size="14" /></UiButton>
          <UiButton v-if="stop && !canSend" type="button" class="send stop" :aria-label="t('agents.sessionView.chatStop')" :title="t('agents.sessionView.chatStop')" data-testid="chat-stop" @click="stop"><Square :size="12" fill="currentColor" /></UiButton>
          <UiButton v-else type="submit" class="send" :disabled="!canSend || sending" :aria-busy="sending" :aria-label="t('agents.sessionView.chatSend')" :title="t('agents.sessionView.chatSend')" data-testid="chat-send"><ArrowUp :size="14" /></UiButton>
        </div>
      </div>
    </template>
  </form>
</template>

<style scoped>
.composer {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 10px;
  box-sizing: border-box;
  width: var(--chat-column-width, auto);
  margin-left: var(--chat-column-left, var(--vr-space-4));
  margin-right: var(--vr-space-4);
  padding: 0 0 var(--vr-space-4);
}

.composer[data-phone] {
  width: auto;
  margin-right: 0;
  margin-left: 0;
  padding: 10px 14px var(--vr-space-3);
  border-top: 1px solid var(--vr-color-border);
}

.box {
  position: relative;
  display: flex;
  flex-direction: column;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-2xl);
}

.box:focus-within {
  border-color: var(--vr-color-focus);
}

.slash {
  position: absolute;
  right: 0;
  bottom: calc(100% + var(--vr-space-1));
  left: 0;
  z-index: 2;
  max-height: min(320px, 45vh);
  margin: 0;
  padding: var(--vr-space-1);
  overflow-y: auto;
  list-style: none;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.slash-item {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-3);
  min-width: 0;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-2);
  font-size: var(--vr-font-size-control);
  cursor: pointer;
  border-radius: var(--vr-radius-md);
}

.slash-item[aria-selected='true'] {
  background: var(--vr-color-nav-active);
}

.slash-item:hover {
  background: var(--vr-color-nav-hover);
}

.slash-name {
  flex: none;
  max-width: 50%;
  overflow: hidden;
  color: var(--vr-color-text);
  font-family: var(--vr-font-mono);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.slash-description {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

:root[data-vr-viewport='phone'] .slash-item {
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: 0;
}

:root[data-vr-viewport='phone'] .slash-name {
  max-width: none;
}

.quotes {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  padding: 10px var(--vr-space-3) 0;
}

.quote {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  height: 24px;
  padding: 0 var(--vr-space-2);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.unquote {
  display: inline-flex;
  align-items: center;
  padding: 0;
  color: var(--vr-color-text-subtle);
  background: none;
  border: 0;
  cursor: pointer;
}

.unquote:hover {
  color: var(--vr-color-text);
}

.field {
  position: relative;
}

.ghost {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) 14px var(--vr-space-1);
  overflow: hidden;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  color: var(--vr-color-text-subtle);
  pointer-events: none;
}

.ghost-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.keycap {
  flex: none;
  padding: 0 5px;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-key);
  line-height: 1.6;
  color: var(--vr-color-text-subtle);
  pointer-events: auto;
  cursor: pointer;
  background: none;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-chip);
  margin-top: 3px;
}

.input {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: 52px;
  padding: var(--vr-space-3) 14px var(--vr-space-1);
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  resize: none;
  background: none;
  border: 0;
  outline: none;
}

.foot {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-2) 10px;
}

.model {
  display: inline-flex;
  flex: 0 1 auto;
  align-items: center;
  min-width: 0;
  max-width: 60%;
  height: 26px;
  padding: 0 var(--vr-space-2);
  overflow: hidden;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-control);
}

.grow {
  flex: 1;
}

.keys {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
}

.attach {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 26px;
  padding: 0;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-control);
}

.attach:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.talk,
.send {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  padding: 0;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: 50%;
}

:root[data-vr-viewport='phone'] .send {
  width: 44px;
  height: 44px;
}

:root:not([data-vr-viewport='phone']) .talk {
  background: transparent;
}

.talk {
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
}

.talk:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.talk.blocked {
  opacity: 0.5;
}

.send {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.send:disabled {
  cursor: default;
  background: var(--vr-color-text-subtle);
}

.talk[data-holding] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
  transform: scale(1.1);
}

.talk[data-cancelling] {
  color: var(--vr-color-danger);
}

.talk-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-2);
}

.talk-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-4);
}

.aa,
.aa-balance {
  width: 44px;
  height: 44px;
}

.aa {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: 50%;
}

.stop {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--vr-btn-danger-text);
  background: var(--vr-btn-danger);
}

.foot .stop {
  margin-left: var(--vr-space-1);
}

.talk.big {
  width: 64px;
  height: 64px;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.talk.big[data-holding] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  box-shadow: 0 0 0 6px var(--vr-color-nav-active);
}

.talk.big[data-cancelling] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}

.hint {
  margin: 0;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-subtle);
}

.quick {
  display: flex;
  gap: 6px;
  align-self: stretch;
  overflow-x: auto;
  scrollbar-width: none;
}

.quick .chip {
  flex: none;
  height: 32px;
  padding: 0 12px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-full);
}

.modes {
  display: flex;
  gap: 2px;
  align-self: center;
  padding: 2px;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.mode {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.mode[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: 0 0 0 1px var(--vr-color-border);
}

.voice {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-3);
}

.wave {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 44px;
}

.bar {
  width: 4px;
  height: var(--h);
  background: var(--vr-color-text);
  border-radius: 2px;
  opacity: 0.5;
}

.wave[data-live] .bar {
  opacity: 1;
  animation: wave calc(var(--vr-motion-slow) * 3) ease-in-out infinite alternate;
  animation-delay: calc(var(--bar) * var(--vr-motion-fast) * -0.7);
}

@keyframes wave {
  from {
    height: 6px;
  }

  to {
    height: var(--h);
  }
}

.voice-text {
  max-width: 300px;
  margin: 0;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-muted);
  text-align: center;
}

.voice-actions {
  display: flex;
  gap: var(--vr-space-3);
}

.pill {
  height: 44px;
  padding: 0 var(--vr-space-4);
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: none;
  border-radius: var(--vr-radius-full);
}

.pill.strong {
  padding: 0 18px;
  font-weight: 600;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.suggestion {
  overflow: hidden;
  padding: var(--vr-space-1) var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
  text-align: left;
  white-space: nowrap;
  text-overflow: ellipsis;
  cursor: pointer;
  background: none;
  border: 1px dashed var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.talk-note {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.talk-note[data-cancelling] {
  color: var(--vr-color-danger);
}
</style>
