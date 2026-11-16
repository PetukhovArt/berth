<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { Ellipsis, MessageSquare } from '../../../shared/ui/icons';
import { VContextMenu } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { allowRequest, alwaysAllowRequest, chooseOption, denyRequest, isYesNoPermission, offersAlways, openChannel, restartAgent, StatusIcon, useAction, useBerth, useChannelUnread, useSessionDock } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { openNextWaiting, settleInbox, snoozeInbox, useInbox, type InboxEntry } from '../model/inbox';

const { t, formatTime } = useI18n();
const nextShortcut = computed(() => shortcutLabel('inbox.nextWaiting'));
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const { items, now } = useInbox();
const { unread: channels } = useChannelUnread();
const dock = useSessionDock();
const berth = useBerth();
const requestOf = (entry: InboxEntry) => berth?.feedback.value.get(entry.node.id);
const yesNo = (entry: InboxEntry) => {
  const request = requestOf(entry);
  return request ? isYesNoPermission(request) : false;
};
const always = (entry: InboxEntry) => {
  const request = requestOf(entry);
  return request ? offersAlways(request) : false;
};
const resuming = useAction();
const resumeFailed = shallowRef<string | null>(null);

async function resume(entry: InboxEntry) {
  const ref = entry.node.ref;
  if (!('Agent' in ref)) return;
  resumeFailed.value = entry.key;
  if (await resuming.run(() => restartAgent(ref.Agent))) resumeFailed.value = null;
}
const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const target = shallowRef<InboxEntry | null>(null);

const menuItems = computed(() => [
  { key: 'snooze-hour', label: t('attention.inbox.snoozeHour'), command: () => target.value && snoozeInbox(target.value, HOUR) },
  { key: 'snooze-day', label: t('attention.inbox.snoozeDay'), command: () => target.value && snoozeInbox(target.value, untilTomorrow()) },
  { key: 'settle', label: t('attention.inbox.settle'), command: () => target.value && settleInbox(target.value) },
]);

function untilTomorrow(): number {
  const morning = new Date();
  morning.setDate(morning.getDate() + 1);
  morning.setHours(9, 0, 0, 0);
  return morning.getTime() - Date.now();
}

function age(since: number): string {
  const diff = Math.max(0, now.value - since);
  if (diff < MINUTE) return t('attention.inbox.now');
  if (diff < HOUR) return t('attention.inbox.minutes', { count: Math.floor(diff / MINUTE) });
  if (diff < DAY) return t('attention.inbox.hours', { count: Math.floor(diff / HOUR) });
  return t('attention.inbox.days', { count: Math.floor(diff / DAY) });
}

function resetTime(until: number | undefined): string {
  return until ? formatTime(until) : '';
}

function showMenu(event: MouseEvent, entry: InboxEntry) {
  target.value = entry;
  menu.value?.show(event);
}
</script>

<template>
  <section v-if="items.length || channels.length" class="inbox" data-testid="inbox">
    <div class="head">
      <span class="title">{{ t('attention.inbox.title') }}</span>
      <span class="count" data-testid="inbox-count">{{ items.length + channels.length }}</span>
      <span class="spacer" />
      <UiButton v-if="nextShortcut" class="next" :title="t('attention.inbox.next')" :aria-label="t('attention.inbox.next')" data-testid="inbox-next-waiting" @click="openNextWaiting">
        <kbd>{{ nextShortcut }}</kbd>
      </UiButton>
    </div>
    <div v-for="entry in items" :key="entry.key" class="item" data-testid="inbox-item" :data-kind="entry.kind" :data-session-id="entry.node.id">
      <UiButton class="row" :title="entry.node.name" @click="dock.openSession(entry.node)" @contextmenu.prevent="showMenu($event, entry)">
        <StatusIcon :status="entry.node.status" :unseen="entry.node.unseen" />
        <span class="body">
          <span class="line">
            <span class="name">{{ entry.node.name }}</span>
            <span class="age">{{ age(entry.since) }}</span>
          </span>
          <span class="reason"><span class="kind">{{ t(`attention.inbox.${entry.kind}`, { time: resetTime(entry.until) }) }}</span><template v-if="entry.place"> · {{ entry.place }}</template></span>
        </span>
      </UiButton>
      <div v-if="requestOf(entry)" class="card vr-phone-only" data-testid="inbox-request">
        <code v-if="requestOf(entry)?.detail" class="detail">{{ requestOf(entry)?.detail }}</code>
        <div class="actions">
          <template v-if="yesNo(entry)">
            <UiButton type="button" class="action primary" data-testid="inbox-allow" @click="allowRequest(entry.node.ref)">{{ t('feedback.allow') }}</UiButton>
            <UiButton v-if="always(entry)" type="button" class="action" :title="t('feedback.alwaysTitle')" data-testid="inbox-always" @click="alwaysAllowRequest(entry.node.ref)">{{ t('feedback.always') }}</UiButton>
            <UiButton type="button" class="action" data-testid="inbox-deny" @click="denyRequest(entry.node.ref)">{{ t('feedback.deny') }}</UiButton>
          </template>
          <UiButton v-for="(option, index) in requestOf(entry)?.options" v-else :key="index" type="button" class="action" :class="{ primary: index === 0 }" data-testid="inbox-option" @click="chooseOption(entry.node.ref, index)">{{ option }}</UiButton>
        </div>
      </div>
      <div v-else-if="entry.kind === 'crashed' && 'Agent' in entry.node.ref" class="card vr-phone-only" data-testid="inbox-crashed">
        <div class="actions">
          <UiButton type="button" class="action primary" :title="t('attention.inbox.resumeTitle')" :disabled="resuming.busy.value" data-testid="inbox-resume" @click="resume(entry)">{{ t('attention.inbox.resume') }}</UiButton>
          <UiButton type="button" class="action" data-testid="inbox-open" @click="dock.openSession(entry.node)">{{ t('attention.inbox.open') }}</UiButton>
        </div>
        <p v-if="resumeFailed === entry.key && resuming.error.value" class="error" role="alert" data-testid="inbox-resume-error">{{ resuming.error.value }}</p>
      </div>
      <UiButton class="more" :title="t('attention.inbox.actions')" :aria-label="t('attention.inbox.actions')" aria-haspopup="menu" data-testid="inbox-menu" @click="showMenu($event, entry)"><Ellipsis :size="14" /></UiButton>
    </div>
    <div v-for="channel in channels" :key="channel.key" class="item" data-testid="inbox-channel" data-kind="channel">
      <UiButton class="row" :title="channel.key" @click="openChannel(channel.key)">
        <MessageSquare :size="14" class="channel-icon" />
        <span class="body">
          <span class="line">
            <span class="name"># {{ channel.key }}</span>
          </span>
          <span class="reason">{{ t('attention.inbox.channelUnread', { count: channel.unread }) }}</span>
        </span>
      </UiButton>
    </div>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`inbox-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
  </section>
</template>

<style scoped>
.inbox {
  margin-bottom: var(--vr-space-2);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 28px;
  padding-left: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.spacer {
  flex: 1;
}

.next {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 var(--vr-space-1);
  margin-right: var(--vr-space-1);
  color: var(--vr-color-text-muted);
  background: none;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-sm);
}

.next:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.next kbd {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.item {
  position: relative;
}

.row {
  display: flex;
  align-items: flex-start;
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

.channel-icon {
  flex: none;
  margin-top: 3px;
  color: var(--vr-color-text-muted);
}

.row :deep(.status-icon) {
  margin-top: 2px;
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.line {
  display: flex;
  gap: var(--vr-space-2);
  justify-content: space-between;
}

.name,
.reason {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.age {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.reason {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.item[data-kind='waiting'] .kind {
  color: var(--vr-status-feedback);
}

.item[data-kind='limited'] .kind {
  color: var(--vr-status-limit);
}

.item[data-kind='crashed'] .kind {
  color: var(--vr-status-crashed);
}

.more {
  position: absolute;
  top: 4px;
  right: var(--vr-space-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 20px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-nav-hover);
  border: 0;
  border-radius: var(--vr-radius-sm);
  opacity: 0;
}

.item:hover .more,
.more:focus-visible {
  opacity: 1;
}

.item:hover .age,
.item:focus-within .age {
  visibility: hidden;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: 0 var(--vr-space-2) var(--vr-space-2) 26px;
}

.detail {
  max-height: min(30dvh, 10em);
  padding: 6px 8px;
  overflow-x: hidden;
  overflow-y: auto;
  overflow-wrap: anywhere;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: pre-wrap;
  background: var(--vr-color-surface-sunken);
  border-radius: var(--vr-radius-md);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
}

.action {
  flex: 0 1 auto;
  min-width: 5.5em;
  min-height: var(--vr-size-touch);
  padding: 0 var(--vr-space-4);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.action.primary {
  font-weight: 600;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

:global(:root[data-vr-viewport='phone']) .item {
  margin-bottom: var(--vr-space-2);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

:global(:root[data-vr-viewport='phone']) .item[data-kind='waiting'] {
  border-color: var(--vr-status-feedback);
}

:global(:root[data-vr-viewport='phone']) .more {
  opacity: 1;
}

:global(:root[data-vr-viewport='phone']) .head {
  display: none;
}

.action:disabled {
  opacity: 0.6;
}

.error {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
  overflow-wrap: anywhere;
}
</style>
