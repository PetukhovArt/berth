<script setup lang="ts">
import { computed, onMounted, shallowRef } from 'vue';
import { VButton, VInputText, VMessage } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { describeError } from '../../../shared/api/connectors/api';
import { readChatBridge, sendChatBridgeTest, writeChatBridge, type ChatBridge } from '../model/bridge';

const { t } = useI18n();

const loaded = shallowRef(false);
const saved = shallowRef<ChatBridge>({ telegram_token: '', telegram_chat: '', telegram_owner: '', webhook_url: '' });
const token = shallowRef('');
const chat = shallowRef('');
const owner = shallowRef('');
const url = shallowRef('');
const busy = shallowRef<'save' | 'test' | null>(null);
const error = shallowRef<string | null>(null);
const sent = shallowRef(false);

const draft = computed<ChatBridge>(() => ({ telegram_token: token.value.trim(), telegram_chat: chat.value.trim(), telegram_owner: owner.value.trim(), webhook_url: url.value.trim() }));
const dirty = computed(() => draft.value.telegram_token !== saved.value.telegram_token || draft.value.telegram_chat !== saved.value.telegram_chat || draft.value.telegram_owner !== saved.value.telegram_owner || draft.value.webhook_url !== saved.value.webhook_url);
const ownerInvalid = computed(() => draft.value.telegram_owner !== '' && !/^-?\d+$/.test(draft.value.telegram_owner));
const telegramHalf = computed(() => Boolean(draft.value.telegram_token) !== Boolean(draft.value.telegram_chat));
const configured = computed(() => (draft.value.telegram_token && draft.value.telegram_chat) || draft.value.webhook_url);

function show(bridge: ChatBridge) {
  saved.value = bridge;
  token.value = bridge.telegram_token;
  chat.value = bridge.telegram_chat;
  owner.value = bridge.telegram_owner;
  url.value = bridge.webhook_url;
}

async function load() {
  try {
    show(await readChatBridge());
    loaded.value = true;
  } catch (failure) {
    error.value = describeError(failure);
  }
}

async function persist() {
  const next = draft.value;
  await writeChatBridge(next);
  show(next);
}

async function run(kind: 'save' | 'test', work: () => Promise<void>) {
  if (busy.value) return;
  busy.value = kind;
  error.value = null;
  sent.value = false;
  try {
    await work();
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = null;
  }
}

const save = () => run('save', persist);

const test = () =>
  run('test', async () => {
    if (dirty.value) await persist();
    await sendChatBridgeTest();
    sent.value = true;
  });

onMounted(load);
</script>

<template>
  <div class="bridge" data-testid="chat-bridge-panel">
    <h2 class="title">{{ t('settings.chatBridge.title') }}</h2>
    <p class="hint">{{ t('settings.chatBridge.hint') }}</p>
    <form class="editor" @submit.prevent="save">
      <label class="field">
        <span>{{ t('settings.chatBridge.telegramToken') }}</span>
        <VInputText v-model="token" size="small" type="password" autocomplete="off" :disabled="!loaded" data-testid="chat-bridge-token" />
      </label>
      <label class="field">
        <span>{{ t('settings.chatBridge.telegramChat') }}</span>
        <VInputText v-model="chat" size="small" autocomplete="off" :disabled="!loaded" data-testid="chat-bridge-chat" />
      </label>
      <p class="hint">{{ t('settings.chatBridge.telegramHint') }}</p>
      <label class="field">
        <span>{{ t('settings.chatBridge.telegramOwner') }}</span>
        <VInputText v-model="owner" size="small" inputmode="numeric" autocomplete="off" :invalid="ownerInvalid" :disabled="!loaded" data-testid="chat-bridge-owner" />
      </label>
      <p class="hint">{{ t('settings.chatBridge.ownerHint') }}</p>
      <VMessage v-if="ownerInvalid" severity="warn" size="small" data-testid="chat-bridge-owner-invalid">{{ t('settings.chatBridge.ownerInvalid') }}</VMessage>
      <label class="field">
        <span>{{ t('settings.chatBridge.webhookUrl') }}</span>
        <VInputText v-model="url" size="small" type="url" autocomplete="off" placeholder="https://" :disabled="!loaded" data-testid="chat-bridge-webhook" />
      </label>
      <p class="hint">{{ t('settings.chatBridge.webhookHint') }}</p>
      <VMessage v-if="telegramHalf" severity="warn" size="small" data-testid="chat-bridge-half">{{ t('settings.chatBridge.half') }}</VMessage>
      <VMessage v-if="error" severity="error" size="small" data-testid="chat-bridge-error">{{ error }}</VMessage>
      <VMessage v-if="sent" severity="success" size="small" data-testid="chat-bridge-sent">{{ t('settings.chatBridge.sent') }}</VMessage>
      <div class="row-actions">
        <VButton type="submit" :label="t('settings.chatBridge.save')" size="small" :loading="busy === 'save'" :disabled="!loaded || !dirty || ownerInvalid || busy !== null" data-testid="chat-bridge-save" />
        <VButton :label="t('settings.chatBridge.test')" size="small" severity="secondary" outlined :loading="busy === 'test'" :disabled="!loaded || !configured || busy !== null" data-testid="chat-bridge-test" @click="test" />
      </div>
    </form>
  </div>
</template>

<style scoped>
.bridge {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.editor {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 32rem;
  padding: var(--vr-space-2) 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
}

.row-actions {
  display: flex;
  gap: var(--vr-space-2);
  padding-top: var(--vr-space-1);
}
</style>
