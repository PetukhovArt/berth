<script setup lang="ts">
import { shallowRef, useTemplateRef } from 'vue';
import { Ellipsis } from '../../../shared/ui/icons';
import { VContextMenu } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { allowRequest, chooseOption, denyRequest, StatusIcon, useBerth, useSessionDock } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { settleInbox, snoozeInbox, useInbox, type InboxEntry } from '../model/inbox';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const { items, now } = useInbox();
const dock = useSessionDock();
const berth = useBerth();
const requestOf = (entry: InboxEntry) => berth?.feedback.value.get(entry.node.id);
const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const target = shallowRef<InboxEntry | null>(null);

const menuItems = [
  { key: 'snooze-hour', label: t('attention.inbox.snoozeHour'), command: () => target.value && snoozeInbox(target.value, HOUR) },
  { key: 'snooze-day', label: t('attention.inbox.snoozeDay'), command: () => target.value && snoozeInbox(target.value, untilTomorrow()) },
  { key: 'settle', label: t('attention.inbox.settle'), command: () => target.value && settleInbox(target.value) },
];

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
  return until ? new Date(until).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '';
}

function showMenu(event: MouseEvent, entry: InboxEntry) {
  target.value = entry;
  menu.value?.show(event);
}
</script>

<template>
  <section v-if="items.length" class="inbox" data-testid="inbox">
    <div class="head">
      <span class="title">{{ t('attention.inbox.title') }}</span>
      <span class="count" data-testid="inbox-count">{{ items.length }}</span>
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
          <template v-if="requestOf(entry)?.kind === 'Permission'">
            <UiButton type="button" class="action primary" data-testid="inbox-allow" @click="allowRequest(entry.node.ref)">{{ t('feedback.allow') }}</UiButton>
            <UiButton type="button" class="action" data-testid="inbox-deny" @click="denyRequest(entry.node.ref)">{{ t('feedback.deny') }}</UiButton>
          </template>
          <UiButton v-for="(option, index) in requestOf(entry)?.options" v-else :key="index" type="button" class="action" :class="{ primary: index === 0 }" data-testid="inbox-option" @click="chooseOption(entry.node.ref, index)">{{ option }}</UiButton>
        </div>
      </div>
      <UiButton class="more" :title="t('attention.inbox.actions')" :aria-label="t('attention.inbox.actions')" aria-haspopup="menu" data-testid="inbox-menu" @click="showMenu($event, entry)"><Ellipsis :size="14" /></UiButton>
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
  font-size: 13px;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.row:hover {
  background: var(--vr-color-nav-hover);
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
  font-size: 11.5px;
  color: var(--vr-color-text-muted);
}

.reason {
  font-size: 11.5px;
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
  padding: 6px 8px;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-surface-sunken);
  border-radius: var(--vr-radius-md);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
}

.action {
  flex: 1;
  min-height: var(--vr-size-touch);
  padding: 0 var(--vr-space-3);
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
</style>
