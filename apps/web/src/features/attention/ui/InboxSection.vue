<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, Ellipsis, MessageSquare } from '../../../shared/ui/icons';
import { useIsPhone, VBadge, VButton, VContextMenu, VMessage, VRowButton, vTooltip } from '../../../shared/ui/kit';
import { allowRequest, alwaysAllowRequest, chooseOption, denyRequest, isYesNoPermission, offersAlways, openChannel, restartAgent, StatusIcon, useAction, useHollow, useBerth, useChannelUnread, useSessionDock, type SessionRef } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { shortAge } from '../../../shared/lib/shortAge';
import { matchesWords } from '../../../shared/lib/fuzzy';
import { inboxCollapsed, openNextNeedingYou, settleInbox, snoozeInbox, useInbox, type InboxEntry } from '../model/inbox';

const { t, formatTime } = useI18n();
const phone = useIsPhone();
const props = withDefaults(defineProps<{ query?: string }>(), { query: '' });
const filtering = computed(() => props.query.trim().length > 0);
const open = computed(() => phone.value || !inboxCollapsed.value || filtering.value);

function toggle() {
  if (!filtering.value) inboxCollapsed.value = !inboxCollapsed.value;
}
const nextShortcut = computed(() => shortcutLabel('inbox.nextWaiting'));
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

const { items, now } = useInbox();
const { unread: channels } = useChannelUnread();
const dock = useSessionDock();
const berth = useBerth();
const hollow = useHollow();
const reason = (entry: InboxEntry) => {
  if (hollow.value) return t(`agents.status.${hollow.value}`);
  return entry.kind === 'limit' && !resetTime(entry.state.limitUntil) ? t('attention.inbox.limitNoReset') : t(`attention.inbox.${entry.kind}`, { time: resetTime(entry.state.limitUntil) });
};
const shown = computed(() => items.value.filter((entry) => matchesWords(props.query, [entry.state.name, entry.place, reason(entry)].join(' '))));
const shownChannels = computed(() => channels.value.filter((channel) => matchesWords(props.query, channel.key)));
const total = computed(() => shown.value.length + shownChannels.value.length);
const hasProjects = computed(() => berth?.tree.value.some((workspace) => workspace.projects.length > 0) ?? false);
const calm = computed(() => total.value === 0 && !filtering.value && !phone.value && hasProjects.value);
const asOf = computed(() => {
  const at = berth?.lastConnectedAt.value;
  return hollow.value && at ? t('attention.inbox.asOf', { time: formatTime(at) }) : '';
});
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
  <section v-if="total || calm" class="inbox" data-testid="inbox">
    <div class="head">
      <VButton class="toggle" text severity="secondary" size="small" :aria-expanded="open" :aria-disabled="filtering" data-testid="inbox-toggle" @click="toggle">
        <ChevronRight :size="14" class="chevron vr-chevron" />
        <span class="title">{{ t('attention.inbox.title') }}</span>
        <VBadge v-if="total" class="count" severity="secondary" size="small" :value="open ? total : t('attention.inbox.needYou', { count: total })" :max="999" data-testid="inbox-count" />
      </VButton>
      <span v-if="asOf" class="as-of" data-testid="inbox-as-of">{{ asOf }}</span>
      <VButton v-if="nextShortcut" v-tooltip="t('attention.inbox.next')" class="next" outlined severity="secondary" size="small" :aria-label="t('attention.inbox.next')" data-testid="inbox-next-waiting" @click="openNextNeedingYou">
        <kbd>{{ nextShortcut }}</kbd>
      </VButton>
    </div>
    <p v-if="open && calm" class="calm" data-testid="inbox-empty">{{ t('attention.inbox.nothing') }}</p>
    <div v-if="open && total" class="items" data-testid="inbox-items">
      <div v-for="entry in shown" :key="entry.key" class="item" data-testid="inbox-item" :data-kind="entry.kind" :data-hollow="!!hollow" :data-session-id="entry.node?.id">
        <VRowButton v-tooltip="entry.state.name" class="row" align="start" @click="dock.openKey(entry.key)" @contextmenu.prevent="showMenu($event, entry)">
          <template #leading><StatusIcon class="status-lead" :status="entry.state.status" :unseen="entry.state.unseen" /></template>
          <span class="body">
            <span class="line">
              <span class="name">{{ entry.state.name }}</span>
              <span class="age">{{ age(entry.since) }}</span>
            </span>
            <span class="reason"><span class="kind">{{ reason(entry) }}</span><template v-if="entry.place"> · {{ entry.place }}</template></span>
          </span>
        </VRowButton>
        <div v-if="requestOf(entry)" class="card vr-phone-only" data-testid="inbox-request">
          <code v-if="requestOf(entry)?.detail" class="detail">{{ requestOf(entry)?.detail }}</code>
          <div class="actions">
            <template v-if="yesNo(entry)">
              <VButton class="action" :label="t('feedback.allow')" data-testid="inbox-allow" @click="act(entry, allowRequest)" />
              <VButton v-if="always(entry)" v-tooltip="t('feedback.alwaysTitle')" class="action" severity="secondary" :label="t('feedback.always')" data-testid="inbox-always" @click="act(entry, alwaysAllowRequest)" />
              <VButton class="action" severity="secondary" :label="t('feedback.deny')" data-testid="inbox-deny" @click="act(entry, denyRequest)" />
            </template>
            <VButton v-for="(option, index) in requestOf(entry)?.options" v-else :key="index" class="action" :severity="index === 0 ? undefined : 'secondary'" :label="option" data-testid="inbox-option" @click="act(entry, (ref) => chooseOption(ref, index))" />
          </div>
        </div>
        <div v-else-if="entry.kind === 'crashed' && agentRef(entry)" class="card vr-phone-only" data-testid="inbox-crashed">
          <code class="detail" data-testid="inbox-crashed-detail">{{ t('attention.inbox.crashedDetail') }}</code>
          <div class="actions">
            <VButton v-tooltip="t('attention.inbox.resumeTitle')" class="action" :label="t('attention.inbox.resume')" :disabled="resuming.busy.value" data-testid="inbox-resume" @click="resume(entry)" />
            <VButton class="action" severity="secondary" :label="t('attention.inbox.open')" data-testid="inbox-open" @click="dock.openKey(entry.key)" />
          </div>
          <VMessage v-if="resumeFailed === entry.key && resuming.error.value" severity="error" size="small" data-testid="inbox-resume-error">{{ resuming.error.value }}</VMessage>
        </div>
        <VButton v-tooltip="t('attention.inbox.actions')" class="more" text severity="secondary" size="small" :aria-label="t('attention.inbox.actions')" aria-haspopup="menu" data-testid="inbox-menu" @click="showMenu($event, entry)">
          <template #icon><Ellipsis :size="14" /></template>
        </VButton>
      </div>
      <div v-for="channel in shownChannels" :key="channel.key" class="item" data-testid="inbox-channel" data-kind="channel">
        <VRowButton v-tooltip="channel.key" class="row" align="start" @click="openChannel(channel.key)">
          <template #leading><MessageSquare :size="14" class="channel-icon" /></template>
          <span class="body">
            <span class="line">
              <span class="name"># {{ channel.key }}</span>
            </span>
            <span class="reason">{{ t('attention.inbox.channelUnread', { count: channel.unread }) }}</span>
          </span>
        </VRowButton>
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
  flex: 1;
  justify-content: flex-start;
  gap: var(--vr-space-2);
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 6px;
  text-align: left;
}

.toggle[aria-disabled='true'] {
  cursor: default;
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
  height: 18px;
  padding: 0 var(--vr-space-1);
  margin-right: var(--vr-space-1);
}

.next kbd {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.item {
  position: relative;
}

.channel-icon {
  flex: none;
  margin-top: 3px;
  color: var(--vr-color-text-muted);
}

.status-lead {
  margin-top: 2px;
}

.body {
  display: flex;
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

.item[data-hollow='true'] .kind {
  color: var(--vr-status-idle);
}

.as-of {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  font-weight: 400;
  color: var(--vr-color-text-subtle);
}

.calm {
  margin: 0;
  padding: 2px var(--vr-space-4) var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  font-weight: 400;
  color: var(--vr-color-text-subtle);
}

.more {
  position: absolute;
  top: 4px;
  right: var(--vr-space-1);
  width: 22px;
  height: 20px;
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
}

:root[data-vr-viewport='phone'] .item {
  margin-bottom: var(--vr-space-2);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

:root[data-vr-viewport='phone'] .item .name {
  font-size: var(--vr-font-size-title);
  font-weight: 600;
}

:root[data-vr-viewport='phone'] .item .reason {
  font-size: var(--vr-font-size-label);
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

</style>
