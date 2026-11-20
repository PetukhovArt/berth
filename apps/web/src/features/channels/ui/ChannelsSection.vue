<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, MessageSquare, Plus, Users } from '../../../shared/ui/icons';
import { VButton, VInputText } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import { createChannel, describeError, openChannel, useChannels, useHubAdmin } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

const channels = useChannels();
const { state } = useHubAdmin();
const onHub = computed(() => state.value === 'member' || state.value === 'admin');
const isAdmin = computed(() => state.value === 'admin');

const open = shallowRef(true);
const form = shallowRef(false);
const name = shallowRef('');
const shared = shallowRef(false);
const saving = shallowRef(false);
const error = shallowRef<string | null>(null);
const nameInput = useTemplateRef<{ $el: HTMLElement }>('nameInput');

async function startForm() {
  open.value = true;
  form.value = true;
  name.value = '';
  shared.value = isAdmin.value;
  error.value = null;
  await nextTick();
  nameInput.value?.$el.focus();
}

async function create() {
  const key = name.value.trim();
  if (!key || saving.value) return;
  saving.value = true;
  error.value = null;
  try {
    await createChannel(key, shared.value && isAdmin.value);
    form.value = false;
  } catch (e) {
    error.value = describeError(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <section v-if="onHub" class="channels" data-testid="channels-section">
    <div class="head">
      <UiButton class="toggle" :aria-expanded="open" :title="t('attention.channels.toggle')" data-testid="channels-toggle" @click="open = !open">
        <ChevronRight :size="14" class="chevron vr-chevron" />
        <span class="title">{{ t('attention.channels.title') }}</span>
        <span v-if="channels.length" class="count">{{ channels.length }}</span>
      </UiButton>
      <UiButton class="icon" :title="t('attention.channels.add')" :aria-label="t('attention.channels.add')" data-testid="channels-add" @click="startForm"><Plus :size="14" /></UiButton>
    </div>
    <template v-if="open">
      <form v-if="form" class="form" data-testid="channels-form" @submit.prevent="create">
        <VInputText ref="nameInput" v-model="name" size="small" autocomplete="off" :aria-label="t('attention.channels.name')" :placeholder="t('attention.channels.namePlaceholder')" :disabled="saving" data-testid="channels-name" @keydown.esc="form = false" />
        <label v-if="isAdmin" class="shared" :title="t('attention.channels.sharedHint')">
          <UiInput v-model="shared" type="checkbox" class="check" :disabled="saving" data-testid="channels-shared" />
          <span>{{ t('attention.channels.shared') }}</span>
        </label>
        <p v-if="error" class="error" role="alert" data-testid="channels-error">{{ error }}</p>
        <div class="actions">
          <VButton type="submit" :label="t('attention.channels.create')" size="small" :loading="saving" :disabled="!name.trim() || saving" data-testid="channels-create" />
          <VButton :label="t('common.cancel')" size="small" severity="secondary" text :disabled="saving" data-testid="channels-cancel" @click="form = false" />
        </div>
      </form>
      <UiButton v-for="channel in channels" :key="channel.key" class="row" :title="channel.shared ? `${channel.key} · ${t('attention.channels.sharedHint')}` : channel.key" :data-testid="`channel-row-${channel.key}`" @click="openChannel(channel.key)">
        <Users v-if="channel.shared" :size="14" class="row-icon" />
        <MessageSquare v-else :size="14" class="row-icon" />
        <span class="name" :class="{ unread: channel.unread > 0 }"># {{ channel.key }}</span>
        <span v-if="channel.unread > 0" class="badge">{{ channel.unread }}</span>
      </UiButton>
    </template>
  </section>
</template>

<style scoped>
.channels {
  margin-bottom: var(--vr-space-2);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 28px;
}

.toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 7px;
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 6px;
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.toggle:hover {
  background: var(--vr-color-nav-hover);
}

.chevron {
  flex: none;
  width: 10px;
  height: 10px;
  color: var(--vr-color-text-subtle);
  transition: transform var(--vr-motion-fast) var(--vr-motion-ease-standard);
}

.toggle[aria-expanded='true'] .chevron {
  transform: rotate(90deg);
}

.title,
.count {
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2);
  margin: 0 var(--vr-space-1) var(--vr-space-2);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.shared {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-label);
}

.actions {
  display: flex;
  gap: var(--vr-space-1);
}

.error {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
  overflow-wrap: anywhere;
}

.row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 4px var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-label);
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.row-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.name.unread {
  font-weight: 600;
}

.badge {
  flex: none;
  min-width: 1.6em;
  padding: 0 0.4em;
  font-size: var(--vr-font-size-2xs);
  line-height: 1.6;
  text-align: center;
  color: var(--vr-color-text);
  background: var(--vr-color-surface-sunken);
  border-radius: 999px;
}
</style>
