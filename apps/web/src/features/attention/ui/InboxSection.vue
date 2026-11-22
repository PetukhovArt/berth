<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, Ellipsis, MessageSquare } from '../../../shared/ui/icons';
import { useIsPhone, VContextMenu } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { allowRequest, alwaysAllowRequest, chooseOption, denyRequest, isYesNoPermission, offersAlways, openChannel, restartAgent, StatusIcon, useAction, useBerth, useChannelUnread, useSessionDock, type SessionRef } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { shortAge } from '../../../shared/lib/shortAge';
import { inboxCollapsed, openNextNeedingYou, settleInbox, snoozeInbox, useInbox, type InboxEntry } from '../model/inbox';

const { t, formatTime } = useI18n();
const phone = useIsPhone();
const open = computed(() => phone.value || !inboxCollapsed.value);
const nextShortcut = computed(() => shortcutLabel('inbox.nextWaiting'));
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

const { items, now } = useInbox();
const { unread: channels } = useChannelUnread();
const dock = useSessionDock();
const berth = useBerth();
const agentRef = (entry: InboxEntry): { Agent: string } | null => (entry.node && 'Agent' in entry.node.ref ? entry.node.ref : null);
const requestOf = (entry: InboxEntry) => {
  const ref = agentRef(entry);
  return ref ? berth?.feedback.value.get(ref.Agent) : undefined;
};
function act(entry: InboxEntry, run: (ref: SessionRef) => Promise<unknown>) {
  const ref = agentRef(entry);
  if (ref) void run(ref);
}
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
  const ref = agentRef(entry);
  if (!ref) return;
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

function age(since: number | null): string {
  return since === null ? '' : shortAge(now.value, since);
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
      <UiButton class="toggle" :aria-expanded="open" data-testid="inbox-toggle" @click="inboxCollapsed = !inboxCollapsed">
        <ChevronRight :size="14" class="chevron vr-chevron" />
        <span class="title">{{ t('attention.inbox.title') }}</span>
        <span class="count" data-testid="inbox-count">{{ items.length + channels.length }}</span>
      </UiButton>
      <UiButton v-if="nextShortcut" class="next" :title="t('attention.inbox.next')" :aria-label="t('attention.inbox.next')" data-testid="inbox-next-waiting" @click="openNextNeedingYou">
        <kbd>{{ nextShortcut }}</kbd>
      </UiButton>
    </div>
    <div v-if="open" class="items" data-testid="inbox-items">
      <div v-for="entry in items" :key="entry.key" class="item" data-testid="inbox-item" :data-kind="entry.kind" :data-session-id="entry.node?.id">
        <UiButton class="row" :title="entry.state.name" @click="dock.openKey(entry.key)" @contextmenu.prevent="showMenu($event, entry)">
          <StatusIcon :status="entry.state.status" :unseen="entry.state.unseen" />
          <span class="body">
            <span class="line">
              <span class="name">{{ entry.state.name }}</span>
              <span class="age">{{ age(entry.since) }}</span>
            </span>
            <span class="reason"><span class="kind">{{ entry.kind === 'limit' && !resetTime(entry.state.limitUntil) ? t('attention.inbox.limitNoReset') : t(`attention.inbox.${entry.kind}`, { time: resetTime(entry.state.limitUntil) }) }}</span><template v-if="entry.place"> · {{ entry.place }}</template></span>
          </span>
        </UiButton>
        <div v-if="requestOf(entry)" class="card vr-phone-only" data-testid="inbox-request">
          <code v-if="requestOf(entry)?.detail" class="detail">{{ requestOf(entry)?.detail }}</code>
          <div class="actions">
            <template v-if="yesNo(entry)">
              <UiButton type="button" class="action primary" data-testid="inbox-allow" @click="act(entry, allowRequest)">{{ t('feedback.allow') }}</UiButton>
              <UiButton v-if="always(entry)" type="button" class="action" :title="t('feedback.alwaysTitle')" data-testid="inbox-always" @click="act(entry, alwaysAllowRequest)">{{ t('feedback.always') }}</UiButton>
              <UiButton type="button" class="action" data-testid="inbox-deny" @click="act(entry, denyRequest)">{{ t('feedback.deny') }}</UiButton>
            </template>
            <UiButton v-for="(option, index) in requestOf(entry)?.options" v-else :key="index" type="button" class="action" :class="{ primary: index === 0 }" data-testid="inbox-option" @click="act(entry, (ref) => chooseOption(ref, index))">{{ option }}</UiButton>
          </div>
        </div>
        <div v-else-if="entry.kind === 'crashed' && agentRef(entry)" class="card vr-phone-only" data-testid="inbox-crashed">
          <code class="detail" data-testid="inbox-crashed-detail">{{ t('attention.inbox.crashedDetail') }}</code>
          <div class="actions">
            <UiButton type="button" class="action primary" :title="t('attention.inbox.resumeTitle')" :disabled="resuming.busy.value" data-testid="inbox-resume" @click="resume(entry)">{{ t('attention.inbox.resume') }}</UiButton>
            <UiButton type="button" class="action" data-testid="inbox-open" @click="dock.openKey(entry.key)">{{ t('attention.inbox.open') }}</UiButton>
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
  display: flex;
  flex-direction: column;
  min-height: 0;
  max-height: 40dvh;
  margin-bottom: var(--vr-space-2);
}

.inbox.pinned-inbox {
  padding-bottom: var(--vr-space-1);
  margin-bottom: 0;
  border-bottom: 1px solid var(--vr-color-border);
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 28px;
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 6px;
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.toggle:hover {
  background: var(--vr-color-nav-hover);
}

.toggle .chevron {
  flex: none;
  width: 10px;
  height: 10px;
  color: var(--vr-color-text-subtle);
}

.items {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
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

.item[data-kind='needs_feedback'] .kind {
  color: var(--vr-status-feedback);
}

.item[data-kind='limit'] .kind {
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

:root[data-vr-viewport='phone'] .item {
  margin-bottom: var(--vr-space-2);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

:root[data-vr-viewport='phone'] .item[data-kind='needs_feedback'] {
  border-color: var(--vr-status-feedback);
}

:root[data-vr-viewport='phone'] .item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
}

:root[data-vr-viewport='phone'] .item > .row {
  grid-column: 1 / -1;
  grid-row: 1;
}

:root[data-vr-viewport='phone'] .item > .card {
  grid-column: 1;
  grid-row: 2;
}

:root[data-vr-viewport='phone'] .more {
  position: static;
  grid-column: 2;
  grid-row: 2;
  margin: 0 var(--vr-space-2) var(--vr-space-2) 0;
  opacity: 1;
}

:root[data-vr-viewport='phone'] .item:not(:has(.card)) > .row {
  grid-column: 1;
}

:root[data-vr-viewport='phone'] .item:not(:has(.card)) > .more {
  grid-row: 1;
  align-self: center;
  margin-bottom: 0;
}

:root[data-vr-viewport='phone'] .item:hover .age,
:root[data-vr-viewport='phone'] .item:focus-within .age {
  visibility: visible;
}

:root[data-vr-viewport='phone'] .head {
  display: none;
}

:root[data-vr-viewport='phone'] .inbox {
  max-height: none;
}

:root[data-vr-viewport='phone'] .items {
  overflow: visible;
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
