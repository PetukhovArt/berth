<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, Ellipsis, MessageSquare, Plus, Users } from '../../../shared/ui/icons';
import { VBadge, VButton, VCheckbox, VContextMenu, VInputText, VMessage, VRowButton, vTooltip } from '../../../shared/ui/kit';
import ConfirmPopover from '../../../shared/ui/ConfirmPopover.vue';
import { createChannel, deleteChannel, describeError, leaveChannel, openChannel, useChannels, useHubAdmin, useMay, type ChannelSummary } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

const channels = useChannels();
const { state } = useHubAdmin();
const onHub = computed(() => state.value === 'member' || state.value === 'admin');
const isAdmin = computed(() => state.value === 'admin');
const mayDelete = useMay('channels.delete');

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const confirm = useTemplateRef<InstanceType<typeof ConfirmPopover>>('confirm');
const target = shallowRef<ChannelSummary | null>(null);
const anchor = shallowRef<HTMLElement | null>(null);
const removing = shallowRef(false);
const rowError = shallowRef<string | null>(null);

const canDelete = (channel: ChannelSummary) => channel.mine || mayDelete.value.ok;
const hasActions = (channel: ChannelSummary) => !channel.shared || canDelete(channel);

const menuItems = computed(() => {
  const channel = target.value;
  if (!channel) return [];
  return [
    ...(channel.shared ? [] : [{ key: 'leave', label: t('attention.channels.leave'), command: () => void leave(channel) }]),
    ...(canDelete(channel) ? [{ key: 'delete', label: t('attention.channels.delete'), command: () => setTimeout(() => confirm.value?.show(anchor.value), 0) }] : []),
  ];
});

function showMenu(event: MouseEvent, channel: ChannelSummary) {
  target.value = channel;
  anchor.value = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  rowError.value = null;
  menu.value?.show(event);
}

async function leave(channel: ChannelSummary) {
  try {
    await leaveChannel(channel.key);
  } catch (e) {
    rowError.value = describeError(e);
  }
}

async function remove() {
  const channel = target.value;
  if (!channel || removing.value) return;
  removing.value = true;
  try {
    await deleteChannel(channel.key);
  } catch (e) {
    rowError.value = describeError(e);
  } finally {
    removing.value = false;
    confirm.value?.hide();
  }
}

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
      <VButton v-tooltip="t('attention.channels.toggle')" class="toggle" size="small" text severity="secondary" :aria-expanded="open" :aria-label="t('attention.channels.toggle')" data-testid="channels-toggle" @click="open = !open">
        <ChevronRight :size="14" class="chevron vr-chevron" />
        <span class="title">{{ t('attention.channels.title') }}</span>
        <VBadge v-if="channels.length" :value="channels.length" severity="secondary" size="small" />
      </VButton>
      <VButton v-tooltip="t('attention.channels.add')" size="small" text severity="secondary" :aria-label="t('attention.channels.add')" data-testid="channels-add" @click="startForm">
        <template #icon><Plus :size="14" /></template>
      </VButton>
    </div>
    <template v-if="open">
      <form v-if="form" class="form" data-testid="channels-form" @submit.prevent="create">
        <VInputText ref="nameInput" v-model="name" autocomplete="off" :aria-label="t('attention.channels.name')" :placeholder="t('attention.channels.namePlaceholder')" :disabled="saving" data-testid="channels-name" @keydown.esc="form = false" />
        <VCheckbox v-if="isAdmin" v-model="shared" v-tooltip="t('attention.channels.sharedHint')" :label="t('attention.channels.shared')" :disabled="saving" data-testid="channels-shared" />
        <VMessage v-if="error" severity="error" size="small" data-testid="channels-error">{{ error }}</VMessage>
        <div class="actions">
          <VButton type="submit" :label="t('attention.channels.create')" size="small" :loading="saving" :disabled="!name.trim() || saving" data-testid="channels-create" />
          <VButton :label="t('common.cancel')" size="small" severity="secondary" text :disabled="saving" data-testid="channels-cancel" @click="form = false" />
        </div>
      </form>
      <VMessage v-if="rowError" class="row-error" severity="error" size="small" data-testid="channels-row-error">{{ rowError }}</VMessage>
      <div v-for="channel in channels" :key="channel.key" class="item">
        <VRowButton v-tooltip="channel.shared ? `${channel.key} · ${t('attention.channels.sharedHint')}` : channel.key" :data-testid="`channel-row-${channel.key}`" @click="openChannel(channel.key)">
          <template #leading>
            <Users v-if="channel.shared" :size="14" class="row-icon" />
            <MessageSquare v-else :size="14" class="row-icon" />
          </template>
          <span :class="{ unread: channel.unread > 0 }"># {{ channel.key }}</span>
          <template v-if="channel.unread > 0" #trailing><VBadge class="badge" :value="channel.unread" severity="secondary" size="small" /></template>
        </VRowButton>
        <VButton v-if="hasActions(channel)" v-tooltip="t('attention.channels.actions')" class="more" size="small" text severity="secondary" :aria-label="t('attention.channels.actions')" aria-haspopup="menu" :data-testid="`channel-menu-${channel.key}`" @click="showMenu($event, channel)">
          <template #icon><Ellipsis :size="14" /></template>
        </VButton>
      </div>
    </template>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`channel-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <ConfirmPopover ref="confirm" :question="t('attention.channels.deleteQuestion', { key: target?.key ?? '' })" :busy="removing" testid="channel-delete-confirm" @confirm="remove" />
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
  flex: 1;
  gap: 7px;
  min-width: 0;
  justify-content: flex-start;
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

.title {
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
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

.actions {
  display: flex;
  gap: var(--vr-space-1);
}

.item {
  position: relative;
}

.more {
  position: absolute;
  top: 3px;
  right: var(--vr-space-1);
  opacity: 0;
}

.item:hover .more,
.more:focus-visible {
  opacity: 1;
}

.item:hover .badge {
  visibility: hidden;
}

.row-error {
  padding: 0 var(--vr-space-2) var(--vr-space-1);
}

.row-icon {
  color: var(--vr-color-text-muted);
}

.unread {
  font-weight: 600;
}
</style>
