<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { Bot, UserRound } from '../../../shared/ui/icons';
import { t, useI18n } from '../../../shared/lib/i18n/i18n';
import { agentName, answerInChannel, channelRequestOf, describeError, grantChannelAccess, isAgentMessage, isChannelReply, isNoteMessage, isOwnAgentRequest, onChannelAppended, ownAgentsFor, postNote, postToChannel, promoteChannelMessage, readChannel, readChannelGrants, readPresence, refreshChannels, revokeChannelGrant, settlementOf, type ChannelGrant, type ChannelMessage, type ChannelPresent, type ChannelRequest, type ChannelSettlement } from '../../../entities/session';

const POLL_MS = 4000;

const props = defineProps<{ channel: string }>();

const { formatDate } = useI18n();
const messages = shallowRef<ChannelMessage[]>([]);
const draft = shallowRef('');
const error = shallowRef<string | null>(null);
const present = shallowRef<ChannelPresent[]>([]);
const busy = shallowRef(false);
const sent = shallowRef<ReadonlySet<number>>(new Set());
const rows = computed(() =>
  messages.value
    .filter((message) => !isChannelReply(message))
    .map((message) => {
      const request = channelRequestOf(message);
      return { message, request, settled: request ? settlementOf(messages.value, message) : null, own: request ? isOwnAgentRequest(message) : false };
    }),
);

function requestText(request: ChannelRequest): string {
  return request.kind === 'permission' ? t('feedback.permission', { tool: request.title }) : request.title;
}

function settledText(request: ChannelRequest, settlement: ChannelSettlement): string {
  if (settlement.answer === 'local') return t('attention.channel.request.answeredLocally');
  const answer = settlement.answer === 'allow' ? t('feedback.allow') : settlement.answer === 'deny' ? t('feedback.deny') : (request.options[Number(settlement.answer) - 1] ?? settlement.answer);
  return t('attention.channel.request.answeredBy', { who: settlement.by ?? '', answer });
}

async function answer(seq: number, token: string) {
  if (busy.value) return;
  busy.value = true;
  try {
    await append(await answerInChannel(props.channel, seq, token));
    sent.value = new Set(sent.value).add(seq);
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}
const grants = shallowRef<ChannelGrant[]>([]);
const granting = shallowRef<number | null>(null);
const ownAgents = computed(() => ownAgentsFor(props.channel, messages.value));
const promoted = shallowRef<ReadonlySet<number>>(new Set());
const list = useTemplateRef<HTMLElement>('list');
let timer: ReturnType<typeof setTimeout> | undefined;
let stopped = false;

async function append(incoming: ChannelMessage[]) {
  const known = new Set(messages.value.map((message) => message.seq));
  const fresh = incoming.filter((message) => !known.has(message.seq));
  if (!fresh.length) return;
  const el = list.value;
  const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  messages.value = [...messages.value, ...fresh].sort((a, b) => a.seq - b.seq);
  if (pinned) {
    await nextTick();
    if (list.value) list.value.scrollTop = list.value.scrollHeight;
  }
}

let readCursor = 0;

async function readNew() {
  const incoming = await readChannel(props.channel, readCursor);
  readCursor = Math.max(readCursor, ...incoming.map((message) => message.seq));
  await append(incoming);
  error.value = null;
}

async function poll() {
  try {
    await readNew();
    void refreshChannels();
    present.value = await readPresence(props.channel).catch(() => []);
    grants.value = await readChannelGrants(props.channel).catch(() => []);
  } catch (failure) {
    error.value = describeError(failure);
  }
  if (!stopped) timer = setTimeout(() => void poll(), POLL_MS);
}

const stopPush = onChannelAppended((channel) => {
  if (channel === props.channel) void readNew().catch((failure) => (error.value = describeError(failure)));
});

const activeNote = computed(() => {
  const last = messages.value.filter(isNoteMessage).at(-1);
  return last?.content ? last : null;
});

async function run(action: () => Promise<ChannelMessage[]>, clearDraft: boolean) {
  if (busy.value) return;
  busy.value = true;
  try {
    await append(await action());
    if (clearDraft) draft.value = '';
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

function send() {
  const content = draft.value.trim();
  if (content) void run(() => postToChannel(props.channel, content), true);
}

function sendNote() {
  const content = draft.value.trim();
  if (content) void run(() => postNote(props.channel, content), true);
}

function clearNote() {
  void run(() => postNote(props.channel, ''), false);
}

async function promote(message: ChannelMessage) {
  try {
    await promoteChannelMessage(props.channel, message.seq);
    promoted.value = new Set(promoted.value).add(message.seq);
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  }
}

function grantChoices(message: ChannelMessage) {
  return ownAgents.value.filter((agent) => !grants.value.some((grant) => grant.agent === agent.id && grant.member === message.author));
}

function toggleGrant(message: ChannelMessage) {
  granting.value = granting.value === message.seq ? null : message.seq;
}

async function grantTo(message: ChannelMessage, agent: string) {
  try {
    grants.value = await grantChannelAccess(props.channel, agent, message.author);
    granting.value = null;
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  }
}

async function revoke(grant: ChannelGrant) {
  try {
    grants.value = await revokeChannelGrant(grant);
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  send();
}

onMounted(() => void poll());
onBeforeUnmount(() => {
  stopped = true;
  clearTimeout(timer);
  stopPush();
});
</script>

<template>
  <section class="channel" data-testid="channel-view">
    <header class="head">{{ t('attention.channel.messages', { key: channel }) }}</header>
    <p v-if="present.length" class="presence" data-testid="channel-presence">{{ t('attention.channel.present', { who: present.map((p) => (p.agents.length ? `${p.name} (${p.agents.join(', ')})` : p.name)).join('; ') }) }}</p>
    <p v-if="activeNote" class="note" data-testid="channel-note">
      <span class="note-text">{{ t('attention.channel.noteBy', { name: activeNote.authorName, text: activeNote.content }) }}</span>
      <UiButton type="button" class="note-clear" :disabled="busy" data-testid="channel-note-clear" @click="clearNote">{{ t('attention.channel.noteClear') }}</UiButton>
    </p>
    <p v-if="grants.length" class="grants" data-testid="channel-grants">
      <span>{{ t('attention.channel.grants') }}</span>
      <span v-for="grant in grants" :key="`${grant.agent}:${grant.member}`" class="grant" data-testid="channel-grant">
        {{ t('attention.channel.grant', { member: grant.member_name, agent: agentName(grant.agent) }) }}
        <UiButton type="button" class="link" :aria-label="t('attention.channel.revokeLabel', { member: grant.member_name, agent: agentName(grant.agent) })" data-testid="channel-grant-revoke" @click="revoke(grant)">{{ t('attention.channel.revoke') }}</UiButton>
      </span>
    </p>
    <ol ref="list" class="messages" data-testid="channel-messages">
      <li v-if="!rows.length" class="empty" data-testid="channel-empty">{{ t('attention.channel.empty') }}</li>
      <li v-for="row in rows" :key="row.message.seq" class="message" :data-author-kind="row.message.authorKind" data-testid="channel-message">
        <span class="marker" :title="isAgentMessage(row.message) ? t('attention.channel.agent') : t('attention.channel.person')" :aria-label="isAgentMessage(row.message) ? t('attention.channel.agent') : t('attention.channel.person')">
          <Bot v-if="isAgentMessage(row.message)" :size="14" />
          <UserRound v-else :size="14" />
        </span>
        <div class="body">
          <div class="meta">
            <span class="author" data-testid="channel-author">{{ row.message.authorName }}</span>
            <span class="time">{{ formatDate(row.message.createdAt, { timeStyle: 'short' }) }}</span>
            <span v-if="isNoteMessage(row.message)" class="tag" data-testid="channel-note-tag">{{ t('attention.channel.noteTag') }}</span>
            <span v-if="promoted.has(row.message.seq)" class="time" data-testid="channel-promoted">{{ t('attention.channel.promoted') }}</span>
            <UiButton v-else-if="row.message.promotable" type="button" class="link" :title="t('attention.channel.promoteHint')" data-testid="channel-promote" @click="promote(row.message)">{{ t('attention.channel.promote') }}</UiButton>
            <UiButton v-if="row.message.grantable && grantChoices(row.message).length" type="button" class="link" :title="t('attention.channel.grantOfferHint', { name: row.message.authorName })" :aria-expanded="granting === row.message.seq" data-testid="channel-grant-open" @click="toggleGrant(row.message)">{{ t('attention.channel.grantOffer', { name: row.message.authorName }) }}</UiButton>
          </div>
          <div v-if="row.request" class="request" data-testid="channel-request" :data-settled="row.settled ? 'true' : 'false'">
            <p class="content">{{ requestText(row.request) }}</p>
            <code v-if="row.request.detail" class="detail" data-testid="channel-request-detail">{{ row.request.detail }}</code>
            <p v-if="row.settled" class="request-state" data-testid="channel-request-settled">{{ settledText(row.request, row.settled) }}</p>
            <p v-else-if="!row.own" class="request-state" data-testid="channel-request-owner-only">{{ t('attention.channel.request.ownerOnly') }}</p>
            <p v-else-if="sent.has(row.message.seq)" class="request-state" data-testid="channel-request-sent">{{ t('attention.channel.request.sent') }}</p>
            <div v-else class="actions">
              <template v-if="row.request.kind === 'permission' && !row.request.options.length">
                <UiButton type="button" class="action primary" :disabled="busy" data-testid="channel-request-allow" @click="answer(row.message.seq, 'allow')">{{ t('feedback.allow') }}</UiButton>
                <UiButton type="button" class="action" :disabled="busy" data-testid="channel-request-deny" @click="answer(row.message.seq, 'deny')">{{ t('feedback.deny') }}</UiButton>
              </template>
              <UiButton v-for="(option, index) in row.request.options" v-else :key="index" type="button" class="action" :class="{ primary: index === 0 }" :disabled="busy" data-testid="channel-request-option" @click="answer(row.message.seq, String(index + 1))">{{ option }}</UiButton>
            </div>
          </div>
          <p v-else class="content">{{ isNoteMessage(row.message) && !row.message.content ? t('attention.channel.noteCleared') : row.message.content }}</p>
          <div v-if="granting === row.message.seq" class="actions" data-testid="channel-grant-choice">
            <UiButton v-for="agent in grantChoices(row.message)" :key="agent.id" type="button" class="action" :aria-label="t('attention.channel.grantAgentLabel', { member: row.message.authorName, agent: agent.name })" data-testid="channel-grant-agent" @click="grantTo(row.message, agent.id)">{{ agent.name }}</UiButton>
          </div>
        </div>
      </li>
    </ol>
    <span v-if="error" class="error" role="alert" data-testid="channel-error">{{ error }}</span>
    <form class="post" @submit.prevent="send">
      <UiTextarea v-model="draft" class="input" rows="2" :placeholder="t('attention.channel.placeholder')" :aria-label="t('attention.channel.placeholder')" data-testid="channel-input" @keydown="onKeydown" />
      <UiButton type="button" class="send" :disabled="busy || !draft.trim()" :title="t('attention.channel.noteHint')" data-testid="channel-note-send" @click="sendNote">{{ t('attention.channel.note') }}</UiButton>
      <UiButton type="submit" class="send" :disabled="busy || !draft.trim()" data-testid="channel-send">{{ t('attention.channel.send') }}</UiButton>
    </form>
  </section>
</template>

<style scoped>
.channel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
}

.head {
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  border-bottom: 1px solid var(--vr-color-border);
}

.note {
  display: flex;
  gap: var(--vr-space-2);
  align-items: baseline;
  justify-content: space-between;
  padding: var(--vr-space-1) var(--vr-space-3);
  margin: 0;
  font-size: var(--vr-font-size-xs);
  border-bottom: 1px solid var(--vr-color-border);
}

.note-text {
  min-width: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.note-clear {
  flex: none;
  padding: 0;
  font: inherit;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
}

.tag {
  color: var(--vr-color-text-muted);
}

.presence {
  padding: var(--vr-space-1) var(--vr-space-3);
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  border-bottom: 1px solid var(--vr-color-border);
}

.grants {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-3);
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  border-bottom: 1px solid var(--vr-color-border);
}

.grant {
  color: var(--vr-color-text);
}

.link {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-accent);
  cursor: pointer;
  background: none;
  border: 0;
}

.messages {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-3);
  min-height: 0;
  padding: var(--vr-space-3);
  margin: 0;
  overflow-y: auto;
  list-style: none;
}

.empty {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.message {
  display: flex;
  gap: var(--vr-space-2);
}

.marker {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.message[data-author-kind='agent'] .marker {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.body {
  min-width: 0;
}

.meta {
  display: flex;
  gap: var(--vr-space-2);
  align-items: baseline;
  font-size: var(--vr-font-size-xs);
}

.author {
  font-weight: 600;
}

.time {
  color: var(--vr-color-text-muted);
}

.content {
  margin: 2px 0 0;
  font-size: var(--vr-font-size-sm);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.request {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  align-items: flex-start;
  padding: var(--vr-space-2);
  margin-top: 2px;
  background: var(--vr-tint-feedback);
  border-radius: var(--vr-radius-md);
}

.request[data-settled='true'] {
  background: var(--vr-color-surface-muted);
}

.detail {
  max-width: 100%;
  padding: 1px 6px;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.request-state {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
}

.action {
  height: 28px;
  padding: 0 12px;
  font-size: var(--vr-font-size-sm);
  font-weight: 500;
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.action.primary {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.error {
  padding: 0 var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-status-crashed);
}

.post {
  display: flex;
  gap: var(--vr-space-2);
  align-items: flex-end;
  padding: var(--vr-space-3);
  border-top: 1px solid var(--vr-color-border);
}

.input {
  flex: 1;
  padding: var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  resize: none;
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.send {
  min-height: 32px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.send:disabled {
  opacity: 0.5;
}
</style>
