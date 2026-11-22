<script setup lang="ts">
import { computed } from 'vue';
import { Archive, ChevronRight, FileText, GitPullRequest, MessageSquare, Ticket, Zap } from '../../../shared/ui/icons';
import { markedSegments, stripMarks, wordSegments } from '../../../shared/lib/segments';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon, StatusIcon, useSessionStates } from '../../../entities/session';
import { pipelineTone } from '../../../entities/merge-request';
import type { SearchRow } from '../model/rows';
import Highlighted from './Highlighted.vue';

const props = defineProps<{ row: SearchRow; query: string; current: boolean; blocked: string | null; heading: string; testid: string }>();
const emit = defineEmits<{ open: []; hover: [] }>();

const WEEK_MS = 7 * 24 * 3_600_000;

const { t, formatRelative, formatDate } = useI18n();
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

const message = computed(() => {
  const { row } = props;
  if (row.kind !== 'messages') return null;
  const { hit } = row;
  const project = hit.cwd?.split(/[\\/]/).findLast(Boolean);
  const crumbs = [project, hit.branch].filter((crumb): crumb is string => Boolean(crumb));
  return { crumbs, snippet: markedSegments(hit.snippet), plain: stripMarks(hit.snippet), role: hit.role === 'user' ? t('search.role.user') : t('search.role.assistant'), age: age(hit.at) };
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
      <span class="text">
        <span class="line" :title="entity.label"><Highlighted :segments="wordSegments(entity.label, query)" /></span>
        <span v-if="entity.crumbs.length" class="sub">
          <template v-for="(crumb, at) in entity.crumbs" :key="at">
            <ChevronRight v-if="at > 0" :size="11" class="sep" />
            <span class="crumb" :title="crumb"><Highlighted :segments="wordSegments(crumb, query)" /></span>
          </template>
        </span>
      </span>
      <span v-if="entity.agent" class="meta">{{ entity.agent }}</span>
      <span v-if="entity.age" class="meta age" :title="entity.age.full">{{ entity.age.text }}</span>
      <StatusIcon v-if="entity.pipeline" :tone="entity.pipeline" />
    </template>
    <template v-else-if="row.kind === 'files'">
      <span class="icons"><FileText :size="14" /></span>
      <span class="text">
        <span class="line" :title="row.file.name"><Highlighted :segments="wordSegments(row.file.name, query)" /></span>
        <span class="sub mono"><span class="crumb" :title="row.file.rel"><Highlighted :segments="wordSegments(row.file.rel, query)" /></span></span>
      </span>
    </template>
    <template v-else-if="row.kind === 'actions'">
      <span class="icons"><Zap :size="14" /></span>
      <span class="text">
        <span class="line" :title="row.action.label"><Highlighted :segments="wordSegments(row.action.label, query)" /></span>
      </span>
    </template>
    <template v-else-if="message && row.kind === 'messages'">
      <span class="icons"><HarnessIcon :harness="row.hit.harness" :size="14" /></span>
      <span class="text">
        <span class="line" :title="heading">
          <span class="name">{{ heading }}</span>
          <span class="role" :data-role="row.hit.role">{{ message.role }}</span>
        </span>
        <span v-if="message.crumbs.length" class="sub">
          <template v-for="(crumb, at) in message.crumbs" :key="at">
            <ChevronRight v-if="at > 0" :size="11" class="sep" />
            <span class="crumb" :title="crumb">{{ crumb }}</span>
          </template>
        </span>
        <span class="snippet" :title="message.plain"><Highlighted :segments="message.snippet" /></span>
      </span>
      <span v-if="message.age" class="meta age" :title="message.age.full">{{ message.age.text }}</span>
    </template>
    <template v-else-if="channel && row.kind === 'channels'">
      <span class="icons"><MessageSquare :size="14" /></span>
      <span class="text">
        <span class="line" :title="channelText"><Highlighted :segments="wordSegments(channelText, query)" /></span>
        <span class="sub"><span class="crumb" :title="channel.where">{{ channel.where }}</span></span>
      </span>
      <span v-if="channel.age" class="meta age" :title="channel.age.full">{{ channel.age.text }}</span>
    </template>
  </li>
</template>

<style scoped>
.result {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
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
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  color: var(--vr-color-text-subtle);
}

.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.line {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.role {
  flex: none;
  padding: 0 var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.sub {
  display: flex;
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
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sep {
  flex: none;
}

.snippet {
  display: -webkit-box;
  overflow: hidden;
  overflow-wrap: anywhere;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.meta {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
}

.age {
  font-variant-numeric: tabular-nums;
}
</style>
