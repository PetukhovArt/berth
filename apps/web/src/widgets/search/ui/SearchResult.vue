<script setup lang="ts">
import { computed } from 'vue';
import { Archive, ChevronRight, FileText, GitPullRequest, MessageSquare, Ticket, Zap } from '../../../shared/ui/icons';
import { markedSegments, revealFirstMatch, stripMarks, wholeWordStart, wordSegments } from '../../../shared/lib/segments';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { HarnessIcon, StatusIcon, useSessionStates } from '../../../entities/session';
import { pipelineTone } from '../../../entities/merge-request';
import type { ConversationRow, SearchRow } from '../model/rows';
import { UiButton } from '../../../shared/ui/UiButton';
import Highlighted from './Highlighted.vue';

const props = defineProps<{ row: SearchRow; query: string; current: boolean; blocked: string | null; heading: string; testid: string; folded?: ConversationRow; compact?: boolean }>();
const emit = defineEmits<{ open: []; hover: []; openConversation: [key: string] }>();

const SNIPPETS_SHOWN = 2;
const CRUMB_MIN_CH = 8;

const crumbStyle = (crumb: string) => ({ minWidth: `${Math.min(crumb.length, CRUMB_MIN_CH)}ch` });
const titleSegments = (text: string, query: string) => revealFirstMatch(wordSegments(text, query));

const { t, formatRelative, formatDate } = useI18n();
const states = useSessionStates();
const phone = useIsPhone();

const age = (value: number | string | null | undefined): { text: string; full: string } | null => {
  if (value === null || value === undefined) return null;
  const time = typeof value === 'number' ? value : Date.parse(value);
  if (!time || Number.isNaN(time)) return null;
  return { text: formatRelative(time, phone.value ? 'narrow' : 'long'), full: formatDate(time) };
};

const entity = computed(() => {
  const { row } = props;
  if (row.kind !== 'sessions' && row.kind !== 'mr' && row.kind !== 'tasks') return null;
  const { node, crumbs, agent, at } = row.entry;
  const data = node.data;
  const session = data?.session;
  const external = data?.external;
  const status = session ? (states.value.get(session.key)?.status ?? session.status) : external ? (states.value.get(node.id)?.status ?? external.status) : undefined;
  const archived = node.kind === 'archived' || node.kind === 'archived-external';
  return {
    label: node.label,
    crumbs,
    status,
    unseen: session?.unseen ?? false,
    live: !archived && (session !== undefined || external !== undefined),
    archived,
    harness: external?.harness ?? null,
    pipeline: data?.item && data.item.pipeline !== 'none' ? pipelineTone(data.item.pipeline) : null,
    agent,
    age: age(at ?? data?.item?.updated_at ?? data?.issue?.updated),
  };
});

const conversation = computed(() => {
  const { row } = props;
  const source = row.kind === 'messages' ? row : props.folded;
  if (!source) return null;
  const { conversation: found } = source;
  const own = row.kind === 'messages';
  const shown = props.compact ? (own ? 1 : 0) : own ? SNIPPETS_SHOWN : 1;
  const project = found.cwd?.split(/[\\/]/).findLast(Boolean);
  return {
    crumbs: [project, found.branch].filter((crumb): crumb is string => Boolean(crumb)),
    matches: t('search.matches', { count: found.matches }),
    age: age(found.at),
    snippets: found.snippets.slice(0, shown).map((message) => ({
      key: message.key,
      segments: markedSegments(wholeWordStart(message.snippet)),
      plain: stripMarks(message.snippet),
      role: message.role === 'user' ? t('search.role.user') : t('search.role.assistant'),
      user: message.role === 'user',
    })),
  };
});

const channel = computed(() => {
  const { row } = props;
  if (row.kind !== 'channels') return null;
  const { message: posted } = row;
  return { age: age(posted.createdAt), where: t('search.channelMessage', { channel: posted.channel, author: posted.authorName }) };
});

const channelText = computed(() => (props.row.kind === 'channels' ? props.row.message.content.replace(/\s+/g, ' ').trim() : ''));
</script>

<template>
  <li
    class="result"
    role="option"
    :data-testid="testid"
    :class="{ compact, counted: conversation !== null }"
    :data-kind="row.kind"
    :data-session-id="row.kind === 'sessions' ? row.entry.node.data?.session?.id : undefined"
    :aria-selected="current"
    :aria-disabled="blocked !== null"
    :data-current="current"
    :data-blocked="blocked !== null"
    v-tooltip="blocked ?? undefined"
    @pointermove="emit('hover')"
    @click="emit('open')"
  >
    <template v-if="entity">
      <span class="icons">
        <Archive v-if="entity.archived" :size="14" />
        <StatusIcon v-else-if="entity.live" :status="entity.status" :unseen="entity.unseen" />
        <GitPullRequest v-else-if="row.kind === 'mr'" :size="14" />
        <Ticket v-else :size="14" />
        <HarnessIcon v-if="entity.harness" :harness="entity.harness" />
      </span>
      <span class="line" :class="{ tallied: conversation !== null }">
        <span class="title" v-tooltip="entity.label"><Highlighted :segments="titleSegments(entity.label, query)" /></span>
        <span v-if="conversation" class="meta count" data-testid="search-matches">{{ conversation.matches }}</span>
      </span>
      <span v-if="entity.crumbs.length" class="sub">
        <span class="place">
          <template v-for="(crumb, at) in entity.crumbs" :key="at">
            <ChevronRight v-if="at > 0" :size="11" class="sep" />
            <span class="crumb" :style="crumbStyle(crumb)" v-tooltip="crumb"><Highlighted :segments="wordSegments(crumb, query)" /></span>
          </template>
        </span>
      </span>
      <span class="aside">
        <span v-if="entity.agent" class="meta">{{ entity.agent }}</span>
        <span v-if="entity.age" class="meta age" v-tooltip="entity.age.full">{{ entity.age.text }}</span>
        <StatusIcon v-if="entity.pipeline" :tone="entity.pipeline" />
      </span>
    </template>
    <template v-else-if="row.kind === 'files'">
      <span class="icons"><FileText :size="14" /></span>
      <span class="line" v-tooltip="row.file.name"><Highlighted :segments="wordSegments(row.file.name, query)" /></span>
      <span class="sub mono"><span class="crumb" v-tooltip="row.file.rel"><Highlighted :segments="wordSegments(row.file.rel, query)" /></span></span>
    </template>
    <template v-else-if="row.kind === 'actions'">
      <span class="icons"><Zap :size="14" /></span>
      <span class="line" v-tooltip="row.action.label"><Highlighted :segments="wordSegments(row.action.label, query)" /></span>
    </template>
    <template v-else-if="conversation && row.kind === 'messages'">
      <span class="icons"><HarnessIcon :harness="row.conversation.harness" :size="14" /></span>
      <span class="line tallied">
        <span class="title" v-tooltip="heading">{{ heading }}</span>
        <span class="meta count" data-testid="search-matches">{{ conversation.matches }}</span>
      </span>
      <span class="sub">
        <span class="place">
          <template v-for="(crumb, at) in conversation.crumbs" :key="at">
            <ChevronRight v-if="at > 0" :size="11" class="sep" />
            <span class="crumb" :style="crumbStyle(crumb)" v-tooltip="crumb">{{ crumb }}</span>
          </template>
        </span>
      </span>
      <span class="aside">
        <span v-if="conversation.age" class="meta age" v-tooltip="conversation.age.full">{{ conversation.age.text }}</span>
      </span>
    </template>
    <template v-else-if="channel && row.kind === 'channels'">
      <span class="icons"><MessageSquare :size="14" /></span>
      <span class="line" v-tooltip="channelText"><Highlighted :segments="titleSegments(channelText, query)" /></span>
      <span class="sub"><span class="crumb" v-tooltip="channel.where">{{ channel.where }}</span></span>
      <span class="aside">
        <span v-if="channel.age" class="meta age" v-tooltip="channel.age.full">{{ channel.age.text }}</span>
      </span>
    </template>
    <span v-if="conversation?.snippets.length" class="snippets">
      <UiButton v-for="snippet in conversation.snippets" :key="snippet.key" type="button" class="snippet" data-testid="search-snippet" @click.stop="emit('openConversation', snippet.key)">
        <span class="role" :data-user="snippet.user">{{ snippet.role }}</span>
        <span class="snippet-text" v-tooltip="snippet.plain"><Highlighted :segments="snippet.segments" /></span>
      </UiButton>
    </span>
  </li>
</template>

<style scoped>
.result {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-content: center;
  column-gap: var(--vr-space-3);
  min-height: var(--vr-size-row);
  box-sizing: border-box;
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  border-radius: var(--vr-radius-md);
}

.result[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.result[data-blocked='true'] {
  cursor: not-allowed;
  opacity: 0.55;
}

.icons {
  display: inline-flex;
  grid-row: 1;
  grid-column: 1;
  align-items: center;
  align-self: center;
  gap: var(--vr-space-1);
  color: var(--vr-color-text-subtle);
}

.line {
  grid-row: 1;
  grid-column: 2;
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tallied {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
}

.title {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sub {
  display: flex;
  grid-row: 2;
  grid-column: 2 / -1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.place {
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow: hidden;
}

.crumb {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.crumb:last-child {
  flex-shrink: 0;
  max-width: 100%;
}

.sep {
  flex: none;
}

.aside {
  display: flex;
  grid-row: 1;
  grid-column: 3;
  align-items: center;
  gap: var(--vr-space-2);
  justify-self: end;
  white-space: nowrap;
}

.meta {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.age {
  font-variant-numeric: tabular-nums;
}

.count {
  flex: none;
  margin-inline-start: auto;
  white-space: nowrap;
}

.snippets {
  display: flex;
  grid-row: 3;
  grid-column: 2 / -1;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
  padding-inline-start: var(--vr-space-3);
  margin-top: var(--vr-space-1);
}

.snippet {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  min-width: 0;
  padding: var(--vr-space-1) var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: start;
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.snippet:hover {
  background: var(--vr-color-nav-hover);
}

.snippet:focus-visible {
  outline: 2px solid var(--vr-color-accent);
  outline-offset: -2px;
}

.role {
  flex: none;
  padding: 0 var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.snippet-text {
  display: -webkit-box;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.compact .snippet-text {
  -webkit-line-clamp: 1;
}

:root[data-vr-viewport='phone'] .line {
  grid-column: 2 / -1;
}

:root[data-vr-viewport='phone'] .sub {
  grid-column: 2;
}

:root[data-vr-viewport='phone'] .aside {
  grid-row: 2;
}

:root[data-vr-viewport='phone'] .counted .line {
  grid-column: 2;
}

:root[data-vr-viewport='phone'] .counted .sub {
  grid-column: 2 / -1;
}

:root[data-vr-viewport='phone'] .counted .aside {
  grid-row: 1;
}
</style>
