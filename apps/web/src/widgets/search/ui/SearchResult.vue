<script setup lang="ts">
import { computed } from 'vue';
import { Archive, ChevronRight, FileText, GitPullRequest, MessageSquare, Ticket, Zap } from '../../../shared/ui/icons';
import { markedSegments, stripMarks, wholeWordStart, wordSegments } from '../../../shared/lib/segments';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon, StatusIcon, useSessionStates } from '../../../entities/session';
import { pipelineTone } from '../../../entities/merge-request';
import type { SearchRow } from '../model/rows';
import Highlighted from './Highlighted.vue';

const props = defineProps<{ row: SearchRow; query: string; current: boolean; blocked: string | null; heading: string; testid: string }>();
const emit = defineEmits<{ open: []; hover: [] }>();

const WEEK_MS = 7 * 24 * 3_600_000;
const SNIPPETS_SHOWN = 2;
const PROJECT_MIN_CH = 12;
const PROJECT_WHOLE_CHARS = 20;

const crumbStyle = (crumb: string, at: number) => {
  if (at !== 0) return undefined;
  return crumb.length > PROJECT_WHOLE_CHARS ? { minWidth: `${PROJECT_MIN_CH}ch` } : { flexShrink: 0 };
};

const { t, formatRelative, formatDate, formatNumber } = useI18n();
const states = useSessionStates();

const age = (value: number | string | null | undefined): { text: string; full: string } | null => {
  if (value === null || value === undefined) return null;
  const time = typeof value === 'number' ? value : Date.parse(value);
  if (!time || Number.isNaN(time)) return null;
  return { text: Date.now() - time > WEEK_MS ? formatDate(time, { dateStyle: 'medium' }) : formatRelative(time), full: formatDate(time) };
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
  if (row.kind !== 'messages') return null;
  const { head, hits } = row;
  const project = head.cwd?.split(/[\\/]/).findLast(Boolean);
  return {
    crumbs: [project, head.branch].filter((crumb): crumb is string => Boolean(crumb)),
    matches: t('search.matches', { count: hits.length }),
    age: age(head.at),
    snippets: hits.slice(0, SNIPPETS_SHOWN).map((hit) => ({
      key: hit.key,
      segments: markedSegments(wholeWordStart(hit.snippet)),
      plain: stripMarks(hit.snippet),
      role: hit.role === 'user' ? t('search.role.user') : t('search.role.assistant'),
      user: hit.role === 'user',
    })),
    more: Math.max(0, hits.length - SNIPPETS_SHOWN),
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
    :data-kind="row.kind"
    :data-session-id="row.kind === 'sessions' ? row.entry.node.data?.session?.id : undefined"
    :aria-selected="current"
    :aria-disabled="blocked !== null"
    :data-current="current"
    :data-blocked="blocked !== null"
    :title="blocked ?? undefined"
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
      <span class="line" :title="entity.label"><Highlighted :segments="wordSegments(entity.label, query)" /></span>
      <span v-if="entity.crumbs.length" class="sub">
        <template v-for="(crumb, at) in entity.crumbs" :key="at">
          <ChevronRight v-if="at > 0" :size="11" class="sep" />
          <span class="crumb" :class="{ lead: at === 0 }" :style="crumbStyle(crumb, at)" :title="crumb"><Highlighted :segments="wordSegments(crumb, query)" /></span>
        </template>
      </span>
      <span class="aside">
        <span v-if="entity.agent" class="meta">{{ entity.agent }}</span>
        <span v-if="entity.age" class="meta age" :title="entity.age.full">{{ entity.age.text }}</span>
        <StatusIcon v-if="entity.pipeline" :tone="entity.pipeline" />
      </span>
    </template>
    <template v-else-if="row.kind === 'files'">
      <span class="icons"><FileText :size="14" /></span>
      <span class="line" :title="row.file.name"><Highlighted :segments="wordSegments(row.file.name, query)" /></span>
      <span class="sub mono"><span class="crumb lead" :title="row.file.rel"><Highlighted :segments="wordSegments(row.file.rel, query)" /></span></span>
    </template>
    <template v-else-if="row.kind === 'actions'">
      <span class="icons"><Zap :size="14" /></span>
      <span class="line" :title="row.action.label"><Highlighted :segments="wordSegments(row.action.label, query)" /></span>
    </template>
    <template v-else-if="conversation && row.kind === 'messages'">
      <span class="icons"><HarnessIcon :harness="row.head.harness" :size="14" /></span>
      <span class="line" :title="heading">{{ heading }}</span>
      <span class="sub">
        <template v-for="(crumb, at) in conversation.crumbs" :key="at">
          <ChevronRight v-if="at > 0" :size="11" class="sep" />
          <span class="crumb" :class="{ lead: at === 0 }" :style="crumbStyle(crumb, at)" :title="crumb">{{ crumb }}</span>
        </template>
        <span class="count" data-testid="search-matches">{{ conversation.matches }}</span>
      </span>
      <span class="aside">
        <span v-if="conversation.age" class="meta age" :title="conversation.age.full">{{ conversation.age.text }}</span>
      </span>
      <span class="snippets">
        <span v-for="snippet in conversation.snippets" :key="snippet.key" class="snippet" data-testid="search-snippet">
          <span class="role" :data-user="snippet.user">{{ snippet.role }}</span>
          <span class="snippet-text" :title="snippet.plain"><Highlighted :segments="snippet.segments" /></span>
        </span>
        <span v-if="conversation.more > 0" class="more" data-testid="search-more-matches">{{ t('search.moreInConversation', { count: formatNumber(conversation.more) }) }}</span>
      </span>
    </template>
    <template v-else-if="channel && row.kind === 'channels'">
      <span class="icons"><MessageSquare :size="14" /></span>
      <span class="line" :title="channelText"><Highlighted :segments="wordSegments(channelText, query)" /></span>
      <span class="sub"><span class="crumb lead" :title="channel.where">{{ channel.where }}</span></span>
      <span class="aside">
        <span v-if="channel.age" class="meta age" :title="channel.age.full">{{ channel.age.text }}</span>
      </span>
    </template>
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

.crumb {
  flex: 0 100 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.crumb.lead {
  flex-shrink: 1;
}

.sep {
  flex: none;
}

.count {
  flex: none;
  margin-inline-start: var(--vr-space-2);
  color: var(--vr-color-text-subtle);
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
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
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

.more {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
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
</style>
