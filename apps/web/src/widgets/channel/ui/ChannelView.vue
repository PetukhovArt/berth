<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { Bot, UserRound } from '../../../shared/ui/icons';
import { t, useI18n } from '../../../shared/lib/i18n/i18n';
import { describeError, isAgentMessage, postToChannel, readChannel, readPresence, refreshChannels, type ChannelMessage, type ChannelPresent } from '../../../entities/session';

const POLL_MS = 4000;

const props = defineProps<{ channel: string }>();

const { formatDate } = useI18n();
const messages = shallowRef<ChannelMessage[]>([]);
const draft = shallowRef('');
const error = shallowRef<string | null>(null);
const present = shallowRef<ChannelPresent[]>([]);
const busy = shallowRef(false);
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

async function poll() {
  try {
    await append(await readChannel(props.channel, messages.value.at(-1)?.seq ?? 0));
    error.value = null;
    void refreshChannels();
    present.value = await readPresence(props.channel).catch(() => []);
  } catch (failure) {
    error.value = describeError(failure);
  }
  if (!stopped) timer = setTimeout(() => void poll(), POLL_MS);
}

async function send() {
  const content = draft.value.trim();
  if (!content || busy.value) return;
  busy.value = true;
  try {
    await append(await postToChannel(props.channel, content));
    draft.value = '';
    error.value = null;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  void send();
}

onMounted(() => void poll());
onBeforeUnmount(() => {
  stopped = true;
  clearTimeout(timer);
});
</script>

<template>
  <section class="channel" data-testid="channel-view">
    <header class="head">{{ t('attention.channel.messages', { key: channel }) }}</header>
    <p v-if="present.length" class="presence" data-testid="channel-presence">{{ t('attention.channel.present', { who: present.map((p) => `${p.name} (${p.agents.join(', ')})`).join('; ') }) }}</p>
    <ol ref="list" class="messages" data-testid="channel-messages">
      <li v-if="!messages.length" class="empty" data-testid="channel-empty">{{ t('attention.channel.empty') }}</li>
      <li v-for="message in messages" :key="message.seq" class="message" :data-author-kind="message.authorKind" data-testid="channel-message">
        <span class="marker" :title="isAgentMessage(message) ? t('attention.channel.agent') : t('attention.channel.person')" :aria-label="isAgentMessage(message) ? t('attention.channel.agent') : t('attention.channel.person')">
          <Bot v-if="isAgentMessage(message)" :size="14" />
          <UserRound v-else :size="14" />
        </span>
        <div class="body">
          <div class="meta">
            <span class="author" data-testid="channel-author">{{ message.authorName }}</span>
            <span class="time">{{ formatDate(message.createdAt, { timeStyle: 'short' }) }}</span>
          </div>
          <p class="content">{{ message.content }}</p>
        </div>
      </li>
    </ol>
    <span v-if="error" class="error" role="alert" data-testid="channel-error">{{ error }}</span>
    <form class="post" @submit.prevent="send">
      <UiTextarea v-model="draft" class="input" rows="2" :placeholder="t('attention.channel.placeholder')" :aria-label="t('attention.channel.placeholder')" data-testid="channel-input" @keydown="onKeydown" />
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

.presence {
  padding: var(--vr-space-1) var(--vr-space-3);
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  border-bottom: 1px solid var(--vr-color-border);
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
